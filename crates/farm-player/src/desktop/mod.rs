//! The desktop game (feature `desktop`): a winit window presenting the [`Player`]'s frames
//! through the GPU renderer (feature `gpu`) or softbuffer ([`present`]), keyboard, mouse and gilrs
//! gamepads, cpal audio, user folders and crash logs.
//!
//! - The window takes its title and size from the cartridge's game info; fullscreen is
//!   borderless and toggles with Alt+Enter or F11 (the player asks through
//!   [`PlayerRequest::SetFullscreen`]).
//! - Frames are paced at 60 per second: the loop sleeps until the next frame is due.
//! - Pixels are physical: HiDPI screens get a sharp frame, and the UI scales with the window.
//! - Closing the window while a game runs asks first, like Quit in the pause menu
//!   ([`Player::request_close`]).
//! - F12 saves the frame on screen as a PNG in the game's screenshots folder ([`screenshot`]).
//! - When the game cannot start or stops with an error, the window shows what happened and where
//!   the crash log is (exported Windows games have no console), until it is closed or Esc /
//!   Enter is pressed.
// Wall-clock time paces frames and stamps saves and crash logs here; it never reaches the
// simulation (docs/LANGUAGES.md "Determinism rules").
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

pub mod crash;
pub mod folders;
mod gamepad;
mod keys;
pub mod present;
pub mod screenshot;

use crate::input::{InputEvent, PointerButton};
use crate::player::{Player, PlayerOptions, PlayerRequest};
use crate::render::error_screen;
use crate::saves::{FsSaveStore, FsSettingsStore, MemorySaveStore, MemorySettingsStore};
use crate::speaker::Audio;
use farm_ui::game::ToastKind;
use farm_ui::RendererChoice;
use gamepad::Gamepads;
use present::Presenter;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowId};

/// Frames per second the loop aims for.
pub const TARGET_FPS: f64 = 60.0;

/// How the desktop game runs.
#[derive(Debug, Clone, Default)]
pub struct DesktopOptions {
    /// Close after this many frames (smoke tests).
    pub exit_after_frames: Option<u64>,
    /// Force a window or fullscreen for this run.
    pub fullscreen: Option<bool>,
    /// The renderer for this run (`--renderer`); else `FARM_RENDERER`, else the setting
    /// ([`present`]).
    pub renderer: Option<RendererChoice>,
}

struct App {
    /// `None` when the game could not even start (only the error screen shows).
    player: Option<Player>,
    options: DesktopOptions,
    window: Option<Arc<Window>>,
    presenter: Option<Presenter>,
    /// The renderer asked for (resolved when the window opens).
    renderer: RendererChoice,
    events: Vec<InputEvent>,
    pointer: PhysicalPosition<f64>,
    last_frame: Option<Instant>,
    next_frame: Instant,
    frames: u64,
    audio: Option<Audio>,
    gamepads: Option<Gamepads>,
    /// The window has keyboard focus (gamepad input is ignored without it).
    focused: bool,
    /// Where F12 screenshots go (`None` without a user folder).
    screenshots: Option<PathBuf>,
    /// F12 was pressed: save the next frame shown.
    screenshot_requested: bool,
    error: Option<String>,
    /// The error screen's paragraphs once the game stopped.
    failure: Option<Vec<String>>,
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |time| time.as_secs() as i64)
}

/// The error screen's paragraphs for `message` and the crash log (if one was written).
fn failure_lines(message: &str, log: Option<&std::path::Path>) -> Vec<String> {
    let mut lines = vec![message.to_owned()];
    match log {
        Some(path) => lines.push(format!("A crash log was saved to {}", path.display())),
        None => lines.push("No crash log could be written.".to_owned()),
    }
    lines.push("Press Esc or close this window to exit.".to_owned());
    lines
}

/// Shows `message` in a window until it is closed (the game could not start). Without a display
/// it does nothing; the caller still reports the error on the console.
pub fn show_error(message: &str, options: DesktopOptions) {
    let Ok(event_loop) = EventLoop::new() else { return };
    let mut app = App::new(None, options);
    app.failure = Some(vec![
        message.to_owned(),
        "Check that the game's files are complete, or reinstall it.".to_owned(),
        "Press Esc or close this window to exit.".to_owned(),
    ]);
    let _ = event_loop.run_app(&mut app);
}

/// Runs the desktop game for `cart` bytes until the player quits. Saves and settings live in
/// the user folders of the game. A cartridge that does not load is shown in a window too.
pub fn run(cart: &[u8], options: DesktopOptions) -> Result<(), String> {
    let loaded = match farm_cart::load_cartridge(cart) {
        Ok(loaded) => loaded,
        Err(error) => {
            let message = format!("The game could not be loaded: {error}");
            if options.exit_after_frames.is_none() {
                show_error(&message, options);
            }
            return Err(message);
        }
    };
    let folders = folders::UserFolders::for_game(&loaded.info.game_id, loaded.info.company.as_deref());
    let mut player_options = PlayerOptions::standalone();
    player_options.clock = Box::new(unix_now);
    // The interface follows the system's language until the player picks one.
    player_options.system_locale = sys_locale::get_locale();
    match &folders {
        Some(folders) => {
            player_options.saves = Box::new(FsSaveStore::new(&folders.saves));
            player_options.settings = Box::new(FsSettingsStore::new(&folders.settings));
        }
        None => {
            eprintln!("No user folder found; saves and settings last until the game closes.");
            player_options.saves = Box::new(MemorySaveStore::new());
            player_options.settings = Box::new(MemorySettingsStore::new());
        }
    }
    crash::install(folders.as_ref().map(|folders| folders.saves.clone()));
    let mut player = match Player::from_cartridge(loaded, player_options) {
        Ok(player) => player,
        Err(error) => {
            let message = error.to_string();
            if options.exit_after_frames.is_none() {
                show_error(&message, options);
            }
            return Err(message);
        }
    };
    if let Some(fullscreen) = options.fullscreen {
        let mut settings = player.settings().clone();
        settings.display.fullscreen = fullscreen;
        player.set_settings(settings);
    }
    crash::publish(player.crash_report());

    let event_loop = EventLoop::new().map_err(|error| format!("Cannot open a window: {error}"))?;
    let mut app = App::new(Some(player), options);
    app.screenshots = folders.map(|folders| folders.screenshots);
    app.audio = Audio::start();
    app.gamepads = Gamepads::start();
    event_loop.run_app(&mut app).map_err(|error| format!("Window loop: {error}"))?;
    match app.error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

impl App {
    fn new(player: Option<Player>, options: DesktopOptions) -> Self {
        let setting = player.as_ref().map_or(RendererChoice::Auto, |player| player.settings().display.renderer);
        let renderer = present::renderer_choice(options.renderer, setting);
        Self {
            player,
            options,
            window: None,
            presenter: None,
            renderer,
            events: Vec::new(),
            pointer: PhysicalPosition::new(0.0, 0.0),
            last_frame: None,
            next_frame: Instant::now(),
            frames: 0,
            audio: None,
            gamepads: None,
            focused: true,
            screenshots: None,
            screenshot_requested: false,
            error: None,
            failure: None,
        }
    }

    fn set_fullscreen(&self, fullscreen: bool) {
        if let Some(window) = &self.window {
            window.set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
        }
    }

    /// The game stopped: write the crash log, then show the error screen until the player
    /// closes it (smoke runs exit at once).
    fn fail(&mut self, event_loop: &ActiveEventLoop, message: String) {
        let report = self.player.as_ref().map(Player::crash_report).unwrap_or_default();
        let log = crash::write(&format!("{report}\n{message}\n"));
        if let Some(path) = &log {
            eprintln!("Crash log: {}", path.display());
        }
        self.failure = Some(failure_lines(&message, log.as_deref()));
        self.error = Some(message);
        // The game is gone; so are its sounds and its fullscreen.
        self.audio = None;
        self.set_fullscreen(false);
        if self.options.exit_after_frames.is_some() {
            event_loop.exit();
        } else if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Saves the frame just shown as a PNG and says where (or why not) in a toast. The frame is
    /// rasterized on the CPU, whichever renderer showed it, so the file matches `--screenshot`.
    fn save_screenshot(&mut self) {
        let Some(player) = self.player.as_mut() else { return };
        let lang = player.lang();
        let result = match (&self.screenshots, player.compose_pixels()) {
            (Some(folder), Ok(pixmap)) => screenshot::write(folder, unix_now(), &farm_render::encode_png(pixmap))
                .map_err(|error| error.to_string()),
            (None, _) => Err("there is no folder to save it in".to_owned()),
            (_, Err(error)) => Err(error.to_string()),
        };
        match result {
            Ok(path) => {
                let path = path.display().to_string();
                eprintln!("Screenshot: {path}");
                player.notify(lang.format("toast.screenshotSaved", &[&path]), ToastKind::Success);
            }
            Err(error) => player.notify(lang.format("toast.screenshotFailed", &[&error]), ToastKind::Error),
        }
    }

    /// The window cannot be drawn to at all: stop.
    fn cannot_draw(&mut self, event_loop: &ActiveEventLoop, message: String) {
        self.presenter = None;
        self.error.get_or_insert(message);
        event_loop.exit();
    }

    fn render(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(window), Some(presenter)) = (self.window.clone(), self.presenter.as_mut()) else { return };
        let size = window.inner_size();
        let (Some(width), Some(height)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        if let Some(lines) = &self.failure {
            let pixmap = error_screen(width.get(), height.get(), "The game stopped", lines);
            if let Err(message) = presenter.show_pixels(&window, width, height, pixmap.data()) {
                self.cannot_draw(event_loop, message);
            }
            return;
        }
        let Some(player) = self.player.as_mut() else { return };
        let now = Instant::now();
        let dt = self.last_frame.map_or(0.0, |last| (now - last).as_secs_f64().min(0.1));
        self.last_frame = Some(now);
        if let Some(gamepads) = self.gamepads.as_mut() {
            gamepads.poll(&mut self.events, self.focused);
        }
        let events = std::mem::take(&mut self.events);
        crash::set_frame(self.frames);
        let output = match presenter.frame(&window, player, dt, &events, width, height) {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                let message = error.to_string();
                self.fail(event_loop, message);
                return;
            }
            Err(message) => {
                self.cannot_draw(event_loop, message);
                return;
            }
        };
        if let Some(audio) = self.audio.as_mut() {
            audio.maintain();
            audio.set_music(output.music);
            for sound in output.sounds {
                audio.play(sound);
            }
        }
        for request in output.requests {
            match request {
                PlayerRequest::Quit => event_loop.exit(),
                PlayerRequest::SetFullscreen(fullscreen) => self.set_fullscreen(fullscreen),
                PlayerRequest::SetTitle(title) => window.set_title(&title),
            }
        }
        if std::mem::take(&mut self.screenshot_requested) {
            self.save_screenshot();
        }
        self.frames += 1;
        if self.frames.is_multiple_of(60) {
            if let Some(player) = &self.player {
                crash::publish(player.crash_report());
            }
        }
        if self.options.exit_after_frames.is_some_and(|limit| self.frames >= limit) {
            event_loop.exit();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let (title, width, height) = match &self.player {
            Some(player) => {
                let info = player.info();
                (info.title.clone(), info.window_width.max(320), info.window_height.max(200))
            }
            None => ("Farming RPG Maker".to_owned(), 800, 500),
        };
        let attributes = Window::default_attributes()
            .with_title(title)
            .with_inner_size(LogicalSize::new(f64::from(width), f64::from(height)))
            .with_min_inner_size(LogicalSize::new(320.0, 200.0));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.error = Some(format!("Cannot open a window: {error}"));
                event_loop.exit();
                return;
            }
        };
        match Presenter::open(&window, self.renderer) {
            Ok(presenter) => self.presenter = Some(presenter),
            Err(message) => {
                self.error = Some(message);
                event_loop.exit();
                return;
            }
        }
        if self.player.as_ref().is_some_and(|player| player.settings().display.fullscreen) {
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        window.request_redraw();
        self.window = Some(window);
        self.next_frame = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                // While a game runs the player asks first (progress since the last save).
                let close = self.failure.is_some() || self.player.as_mut().is_none_or(Player::request_close);
                if close {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => self.render(event_loop),
            WindowEvent::KeyboardInput { event, .. } if self.failure.is_some() => {
                let key = keys::key_name(&event.logical_key);
                if event.state == ElementState::Pressed && matches!(key.as_deref(), Some("escape" | "enter")) {
                    event_loop.exit();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(key) = keys::key_name(&event.logical_key) {
                    // F12 belongs to the window (like F11): the game never sees it.
                    if key == screenshot::KEY {
                        if event.state == ElementState::Pressed && !event.repeat {
                            self.screenshot_requested = true;
                        }
                        return;
                    }
                    self.events.push(match event.state {
                        ElementState::Pressed => InputEvent::KeyDown { key, repeat: event.repeat },
                        ElementState::Released => InputEvent::KeyUp { key },
                    });
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = position;
                self.events.push(InputEvent::PointerMove { x: position.x as f32, y: position.y as f32 });
            }
            WindowEvent::CursorLeft { .. } => self.events.push(InputEvent::PointerLeft),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => PointerButton::Primary,
                    MouseButton::Right => PointerButton::Secondary,
                    MouseButton::Middle => PointerButton::Middle,
                    _ => return,
                };
                let (x, y) = (self.pointer.x as f32, self.pointer.y as f32);
                self.events.push(match state {
                    ElementState::Pressed => InputEvent::PointerDown { x, y, button },
                    ElementState::Released => InputEvent::PointerUp { x, y, button },
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x, y),
                    MouseScrollDelta::PixelDelta(position) => (position.x as f32 / 40.0, position.y as f32 / 40.0),
                };
                self.events.push(InputEvent::Wheel { dx, dy });
            }
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                if !focused {
                    self.events.push(InputEvent::FocusLost);
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.failure.is_some() {
            // The error screen only redraws when the window asks (resizes, exposure).
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        let now = Instant::now();
        if now >= self.next_frame {
            let period = Duration::from_secs_f64(1.0 / TARGET_FPS);
            // Keep the cadence, but never try to catch up on missed frames.
            self.next_frame = (self.next_frame + period).max(now);
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}
