//! Getting frames onto the window: the CPU renderer through softbuffer, or (feature `gpu`) the GPU
//! renderer of farm-gpu through a wgpu surface.
//!
//! The renderer is chosen when the window opens: `--renderer`, else `FARM_RENDERER`, else the
//! `display.renderer` setting, else `auto` ([`RendererChoice`]). `auto` takes the GPU when a
//! hardware adapter works and the CPU otherwise; `gpu` accepts software adapters too. Whatever
//! goes wrong with the GPU (no adapter, no surface, a failed or lost device, a panic in the
//! driver layer), the window switches to the CPU renderer and shows the same frame there, so the
//! game never stops or shows a black window over it. The renderer in use is logged on stderr.

use crate::input::InputEvent;
use crate::player::{Player, PlayerError, StepOutput};
use farm_ui::RendererChoice;
use std::num::NonZeroU32;
use std::sync::Arc;
use winit::window::Window;

type Surface = softbuffer::Surface<Arc<Window>, Arc<Window>>;

/// The environment variable that picks the renderer (`auto`, `cpu`, `gpu`) when no flag does.
pub const RENDERER_ENV: &str = "FARM_RENDERER";

/// The renderer for this run: the flag, else the environment, else the setting.
pub fn renderer_choice(flag: Option<RendererChoice>, setting: RendererChoice) -> RendererChoice {
    flag.or_else(|| std::env::var(RENDERER_ENV).ok().and_then(|value| RendererChoice::parse(&value))).unwrap_or(setting)
}

/// Draws frames into the window.
pub enum Presenter {
    Cpu(Box<Surface>),
    #[cfg(feature = "gpu")]
    Gpu(Box<farm_gpu::WindowRenderer>),
}

impl std::fmt::Debug for Presenter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Presenter::Cpu(_) => f.write_str("Presenter::Cpu"),
            #[cfg(feature = "gpu")]
            Presenter::Gpu(gpu) => write!(f, "Presenter::Gpu({})", gpu.summary()),
        }
    }
}

#[cfg(feature = "gpu")]
fn panic_text(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic".to_owned())
}

impl Presenter {
    /// The presenter `choice` asks for, falling back to the CPU.
    pub fn open(window: &Arc<Window>, choice: RendererChoice) -> Result<Self, String> {
        match choice {
            RendererChoice::Cpu => {
                eprintln!("Renderer: CPU (chosen)");
                Self::cpu(window)
            }
            #[cfg(feature = "gpu")]
            RendererChoice::Auto | RendererChoice::Gpu => {
                let size = window.inner_size();
                let allow_software = choice == RendererChoice::Gpu;
                let target = Arc::clone(window);
                let opened = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    farm_gpu::WindowRenderer::new(target, size.width, size.height, allow_software)
                }));
                match opened {
                    Ok(Ok(gpu)) => {
                        eprintln!("Renderer: GPU, {}", gpu.summary());
                        return Ok(Presenter::Gpu(Box::new(gpu)));
                    }
                    Ok(Err(error)) => eprintln!("Renderer: CPU (the GPU renderer is not available: {error})"),
                    Err(panic) => eprintln!("Renderer: CPU (the GPU renderer failed: {})", panic_text(&*panic)),
                }
                Self::cpu(window)
            }
            #[cfg(not(feature = "gpu"))]
            RendererChoice::Auto | RendererChoice::Gpu => {
                if choice == RendererChoice::Gpu {
                    eprintln!("Renderer: CPU (this build has no GPU renderer)");
                } else {
                    eprintln!("Renderer: CPU");
                }
                Self::cpu(window)
            }
        }
    }

    /// The CPU renderer's softbuffer surface.
    pub fn cpu(window: &Arc<Window>) -> Result<Self, String> {
        let context = softbuffer::Context::new(Arc::clone(window))
            .map_err(|error| format!("Cannot draw to the window: {error}"))?;
        let surface = softbuffer::Surface::new(&context, Arc::clone(window))
            .map_err(|error| format!("Cannot draw to the window: {error}"))?;
        Ok(Presenter::Cpu(Box::new(surface)))
    }

    /// Copies premultiplied RGBA (opaque) pixels into the window as 0RGB.
    fn blit(surface: &mut Surface, width: NonZeroU32, height: NonZeroU32, pixels: &[u8]) {
        if surface.resize(width, height).is_err() {
            return;
        }
        if let Ok(mut buffer) = surface.buffer_mut() {
            for (out, pixel) in buffer.iter_mut().zip(pixels.chunks_exact(4)) {
                *out = (u32::from(pixel[0]) << 16) | (u32::from(pixel[1]) << 8) | u32::from(pixel[2]);
            }
            let _ = buffer.present();
        }
    }

    /// Gives up the GPU renderer for the CPU one (logged).
    #[cfg(feature = "gpu")]
    fn fall_back(&mut self, window: &Arc<Window>, why: &str) -> Result<(), String> {
        eprintln!("Renderer: CPU (the GPU renderer stopped: {why})");
        // The assignment drops the wgpu surface before softbuffer presents anything.
        *self = Self::cpu(window)?;
        Ok(())
    }

    /// Shows CPU pixels (the error screen).
    #[cfg_attr(not(feature = "gpu"), allow(unused_variables))]
    pub fn show_pixels(
        &mut self,
        window: &Arc<Window>,
        width: NonZeroU32,
        height: NonZeroU32,
        pixels: &[u8],
    ) -> Result<(), String> {
        match self {
            Presenter::Cpu(surface) => {
                Self::blit(surface, width, height, pixels);
                Ok(())
            }
            #[cfg(feature = "gpu")]
            Presenter::Gpu(gpu) => {
                let shown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    gpu.present_pixels(width.get(), height.get(), pixels)
                }));
                let why = match shown {
                    Ok(Ok(_)) => return Ok(()),
                    Ok(Err(error)) => error.to_string(),
                    Err(panic) => panic_text(&*panic),
                };
                self.fall_back(window, &why)?;
                self.show_pixels(window, width, height, pixels)
            }
        }
    }

    /// Runs one frame of `player` and shows it. A player error comes back as `Ok(Err(…))`; a
    /// window that cannot be drawn to at all as `Err`.
    #[cfg_attr(not(feature = "gpu"), allow(unused_variables))]
    pub fn frame(
        &mut self,
        window: &Arc<Window>,
        player: &mut Player,
        dt: f64,
        events: &[InputEvent],
        width: NonZeroU32,
        height: NonZeroU32,
    ) -> Result<Result<StepOutput, PlayerError>, String> {
        match self {
            Presenter::Cpu(surface) => {
                let output = match player.frame(dt, events, width.get(), height.get()) {
                    Ok(output) => output,
                    Err(error) => return Ok(Err(error)),
                };
                Self::blit(surface, width, height, output.pixels.data());
                Ok(Ok(StepOutput { sounds: output.sounds, requests: output.requests, music: output.music }))
            }
            #[cfg(feature = "gpu")]
            Presenter::Gpu(gpu) => {
                let (step, shown) = {
                    let (step, composition) = match player.frame_composition(dt, events, width.get(), height.get()) {
                        Ok(frame) => frame,
                        Err(error) => return Ok(Err(error)),
                    };
                    let shown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| gpu.present(&composition)));
                    (step, shown)
                };
                let why = match shown {
                    Ok(Ok(_)) => return Ok(Ok(step)),
                    // A frame the GPU renderer cannot hold: the CPU draws this one.
                    Ok(Err(farm_gpu::GpuError::TooLarge)) => None,
                    Ok(Err(error)) => Some(error.to_string()),
                    Err(panic) => Some(panic_text(&*panic)),
                };
                if let Some(why) = why {
                    self.fall_back(window, &why)?;
                }
                let pixels = match player.compose_pixels() {
                    Ok(pixels) => pixels,
                    Err(error) => return Ok(Err(error)),
                };
                self.show_pixels(window, width, height, pixels.data())?;
                Ok(Ok(step))
            }
        }
    }
}
