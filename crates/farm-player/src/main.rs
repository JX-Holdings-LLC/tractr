use farm_player::script::{run_frames, InputScript};
use farm_player::{run_cartridge, DebugAction, Player, PlayerOptions, ReplayFile};
use farm_sim::schema::TestState;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Default)]
struct Options {
    cart: Option<PathBuf>,
    replay: Option<PathBuf>,
    load: Option<PathBuf>,
    save: Option<PathBuf>,
    headless: bool,
    screenshot: Option<PathBuf>,
    size: Option<(u32, u32)>,
    frames: Option<u32>,
    script: Option<PathBuf>,
    exit_after_frames: Option<u64>,
    fullscreen: Option<bool>,
    renderer: Option<farm_ui::RendererChoice>,
    /// `--test-state` and the `--start-*` options, in that order.
    test_state: Option<PathBuf>,
    start: TestState,
}

impl Options {
    fn starts_somewhere(&self) -> bool {
        self.test_state.is_some() || self.start != TestState::default()
    }

    /// The test state to apply after New Game: `base` (a replay's own), then `--test-state`, then
    /// the `--start-*` options, each over the one before.
    fn test_state(&self, base: Option<TestState>) -> Result<Option<TestState>, String> {
        if !self.starts_somewhere() {
            return Ok(base);
        }
        let mut layers = Vec::new();
        layers.extend(base);
        if let Some(path) = &self.test_state {
            let text = fs::read_to_string(path).map_err(|error| format!("Read {}: {error}", path.display()))?;
            layers
                .push(serde_json::from_str(&text).map_err(|error| format!("Test state {}: {error}", path.display()))?);
        }
        layers.push(self.start.clone());
        Ok(Some(layers.into_iter().reduce(overlay).unwrap_or_default()))
    }
}

/// `top` over `base`: every field `top` sets wins, maps merge.
fn overlay(base: TestState, top: TestState) -> TestState {
    let mut flags = base.flags;
    flags.extend(top.flags);
    let mut variables = base.variables;
    variables.extend(top.variables);
    let mut quests = base.quests;
    quests.extend(top.quests);
    let mut friendship = base.friendship;
    friendship.extend(top.friendship);
    TestState {
        id: if top.id.is_empty() { base.id } else { top.id },
        name: if top.name.is_empty() { base.name } else { top.name },
        description: top.description.or(base.description),
        day: top.day.or(base.day),
        season: top.season.or(base.season),
        day_of_season: top.day_of_season.or(base.day_of_season),
        year: top.year.or(base.year),
        time: top.time.or(base.time),
        weather: top.weather.or(base.weather),
        // A scene of its own starts on its own tile.
        x: if top.scene_id.is_some() { top.x } else { top.x.or(base.x) },
        y: if top.scene_id.is_some() { top.y } else { top.y.or(base.y) },
        scene_id: top.scene_id.or(base.scene_id),
        facing: top.facing.or(base.facing),
        flags,
        variables,
        quests,
        friendship,
        inventory: top.inventory.or(base.inventory),
        money: top.money.or(base.money),
    }
}

fn parse_tile(text: &str) -> Result<(i32, i32), String> {
    let (x, y) = text.split_once(',').ok_or_else(|| format!("--start-at wants X,Y (a tile), not {text}"))?;
    match (x.trim().parse::<i32>(), y.trim().parse::<i32>()) {
        (Ok(x), Ok(y)) if x >= 0 && y >= 0 => Ok((x, y)),
        _ => Err(format!("--start-at wants X,Y (whole tiles from 0), not {text}")),
    }
}

/// `--time 8:30` or `--time 510` (minutes after midnight).
fn parse_time(text: &str) -> Result<u32, String> {
    let error = || format!("--time wants a minute of the day (510) or HH:MM (8:30), not {text}");
    match text.split_once(':') {
        Some((hours, minutes)) => {
            let (hours, minutes) =
                (hours.parse::<u32>().map_err(|_| error())?, minutes.parse::<u32>().map_err(|_| error())?);
            if minutes >= 60 {
                return Err(error());
            }
            hours.checked_mul(60).and_then(|h| h.checked_add(minutes)).ok_or_else(error)
        }
        None => text.parse().map_err(|_| error()),
    }
}

fn usage() -> &'static str {
    "Usage:\n\
     \x20 farm-player [--cart game.cart] [--fullscreen | --windowed] [--renderer auto|gpu|cpu] [--exit-after-frames N]\n\
     \x20     Play the game in a window (game.cart beside the executable by default). --renderer (or\n\
     \x20     FARM_RENDERER) picks the GPU or CPU renderer; auto takes the GPU when a hardware one works.\n\
     \x20 farm-player --headless [--cart game.cart] [--replay replay.json] [--load save] [--save save]\n\
     \x20     Replay without a window and print a JSON report (quests, flags, variables, money). A save\n\
     \x20     path ending in .json writes a JSON save; any other name writes a binary save. --load\n\
     \x20     reads either. Replay `expect` entries that do not hold fail the run.\n\
     \x20 farm-player --screenshot out.png [--cart game.cart] [--size 1280x800] [--frames 60] [--script inputs.json]\n\
     \x20     Render frames without a window (60 per second of game time) and write the last one.\n\
     \x20 Start options for --headless and --screenshot, applied after New Game:\n\
     \x20     [--test-state state.json] [--start-scene ID] [--start-at X,Y] [--facing up|down|left|right]\n\
     \x20     [--time MINUTE|HH:MM] [--day N]"
}

fn parse_size(text: &str) -> Result<(u32, u32), String> {
    let (width, height) =
        text.split_once(['x', 'X']).ok_or_else(|| format!("--size wants WIDTHxHEIGHT, not {text}"))?;
    let parse = |value: &str| value.trim().parse::<u32>().ok().filter(|v| (16..=8192).contains(v));
    match (parse(width), parse(height)) {
        (Some(width), Some(height)) => Ok((width, height)),
        _ => Err(format!("--size wants WIDTHxHEIGHT between 16 and 8192, not {text}")),
    }
}

fn parse_args() -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value.\n{}", usage()));
        match arg.as_str() {
            "--headless" => options.headless = true,
            "--cart" => options.cart = Some(PathBuf::from(value("--cart")?)),
            "--replay" => options.replay = Some(PathBuf::from(value("--replay")?)),
            "--load" => options.load = Some(PathBuf::from(value("--load")?)),
            "--save" => options.save = Some(PathBuf::from(value("--save")?)),
            "--screenshot" => options.screenshot = Some(PathBuf::from(value("--screenshot")?)),
            "--script" => options.script = Some(PathBuf::from(value("--script")?)),
            "--size" => options.size = Some(parse_size(&value("--size")?)?),
            "--frames" => {
                let text = value("--frames")?;
                options.frames = Some(text.parse().map_err(|_| format!("--frames wants a number, not {text}"))?);
            }
            "--exit-after-frames" => {
                let text = value("--exit-after-frames")?;
                options.exit_after_frames =
                    Some(text.parse().map_err(|_| format!("--exit-after-frames wants a number, not {text}"))?);
            }
            "--test-state" => options.test_state = Some(PathBuf::from(value("--test-state")?)),
            "--start-scene" => options.start.scene_id = Some(value("--start-scene")?),
            "--start-at" => {
                let (x, y) = parse_tile(&value("--start-at")?)?;
                (options.start.x, options.start.y) = (Some(x), Some(y));
            }
            "--facing" => {
                let text = value("--facing")?;
                if !farm_sim::schema::directions::ALL.contains(&text.as_str()) {
                    return Err(format!("--facing wants up, down, left or right, not {text}"));
                }
                options.start.facing = Some(text);
            }
            "--time" => options.start.time = Some(parse_time(&value("--time")?)?),
            "--day" => {
                let text = value("--day")?;
                options.start.day = Some(
                    text.parse()
                        .ok()
                        .filter(|day| *day >= 1)
                        .ok_or_else(|| format!("--day wants a day from 1, not {text}"))?,
                );
            }
            "--fullscreen" => options.fullscreen = Some(true),
            "--windowed" => options.fullscreen = Some(false),
            "--renderer" => {
                let text = value("--renderer")?;
                options.renderer = Some(
                    farm_ui::RendererChoice::parse(&text)
                        .ok_or_else(|| format!("--renderer wants cpu, gpu or auto, not {text}"))?,
                );
            }
            "--help" | "-h" => return Err(usage().to_owned()),
            _ => return Err(format!("Unknown option {arg}.\n{}", usage())),
        }
    }
    Ok(options)
}

fn cart_path(options: &Options) -> Result<PathBuf, String> {
    match &options.cart {
        Some(path) => Ok(path.clone()),
        None => Ok(env::current_exe()
            .map_err(|error| format!("Find player executable: {error}"))?
            .parent()
            .ok_or("Player executable has no folder.")?
            .join("game.cart")),
    }
}

fn read(path: &PathBuf) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("Read {}: {error}", path.display()))
}

fn headless(options: Options) -> Result<(), String> {
    let cart = read(&cart_path(&options)?)?;
    let mut replay: ReplayFile = match &options.replay {
        Some(path) => {
            let text = fs::read_to_string(path).map_err(|error| format!("Read {}: {error}", path.display()))?;
            ReplayFile::parse(&text).map_err(|error| format!("Replay {}: {error}", path.display()))?
        }
        None => ReplayFile::default(),
    };
    replay.test_state = options.test_state(replay.test_state.take())?;
    let incoming_save = match &options.load {
        Some(path) => Some(read(path)?),
        None => None,
    };
    let result = run_cartridge(&cart, &replay, incoming_save.as_deref())?;
    if let Some(path) = options.save {
        let json = path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
        let bytes = if json { result.save_json.into_bytes() } else { result.save_binary };
        fs::write(&path, bytes).map_err(|error| format!("Write {}: {error}", path.display()))?;
    }
    println!("{}", serde_json::to_string(&result.report).map_err(|error| error.to_string())?);
    Ok(())
}

/// Renders frames without a window (in-memory saves and settings, a fixed clock).
fn screenshot(options: Options, out: PathBuf) -> Result<(), String> {
    let cart = read(&cart_path(&options)?)?;
    let script = match &options.script {
        Some(path) => {
            InputScript::parse(&fs::read_to_string(path).map_err(|error| format!("Read {}: {error}", path.display()))?)?
        }
        None => InputScript::default(),
    };
    let (width, height) = options.size.unwrap_or((1280, 800));
    let mut player =
        Player::from_cartridge_bytes(&cart, PlayerOptions::standalone()).map_err(|error| error.to_string())?;
    // A run that starts somewhere skips the title screen: New Game, then the test state (the
    // way Play Mode's debug teleport moves the player).
    if let Some(test) = options.test_state(None)? {
        player.start_new_game().map_err(|error| error.to_string())?;
        player.debug(&DebugAction::ApplyTestState { state: Box::new(test) }).map_err(|error| error.to_string())?;
    }
    let pixmap = run_frames(&mut player, options.frames.unwrap_or(60), width, height, &script)
        .map_err(|error| error.to_string())?;
    let png = farm_render::encode_png(&pixmap);
    fs::write(&out, png).map_err(|error| format!("Write {}: {error}", out.display()))?;
    println!("Wrote {} ({width}×{height})", out.display());
    Ok(())
}

#[cfg(feature = "desktop")]
fn windowed(options: Options) -> Result<(), String> {
    let desktop = farm_player::desktop::DesktopOptions {
        exit_after_frames: options.exit_after_frames,
        fullscreen: options.fullscreen,
        renderer: options.renderer,
    };
    // An exported game has no console: a missing cartridge is shown in a window too.
    let cart = match cart_path(&options).and_then(|path| read(&path)) {
        Ok(cart) => cart,
        Err(error) => {
            if desktop.exit_after_frames.is_none() {
                farm_player::desktop::show_error(&format!("The game could not be loaded: {error}"), desktop);
            }
            return Err(error);
        }
    };
    farm_player::desktop::run(&cart, desktop)
}

#[cfg(not(feature = "desktop"))]
fn windowed(_: Options) -> Result<(), String> {
    Err(format!("This player was built without the desktop feature; use --headless or --screenshot.\n{}", usage()))
}

fn run(options: Options) -> Result<(), String> {
    if options.headless {
        return headless(options);
    }
    if options.screenshot.is_none() && options.starts_somewhere() {
        return Err(format!("The start options are for --headless and --screenshot runs.\n{}", usage()));
    }
    if let Some(out) = options.screenshot.clone() {
        return screenshot(options, out);
    }
    windowed(options)
}

fn main() {
    if matches!(env::args().nth(1).as_deref(), Some("--help" | "-h")) {
        println!("{}", usage());
        return;
    }
    let outcome = parse_args().and_then(run);
    if let Err(error) = outcome {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
