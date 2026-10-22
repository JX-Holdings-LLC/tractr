//! The `farm-player` executable: `--screenshot` renders without a window, and the windowed game
//! runs a few frames under a virtual X server (`xvfb-run`) with each renderer and exits cleanly.

mod common;

use std::path::PathBuf;
use std::process::Command;

fn scratch(name: &str) -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("farm-player-binary");
    std::fs::create_dir_all(&folder).unwrap();
    folder.join(name)
}

fn cart() -> PathBuf {
    common::root().join("fixtures").join("golden").join("cartridges").join("fixture-project.cart")
}

#[test]
fn screenshot_renders_frames_without_a_window() {
    let out = scratch("title.png");
    let script = scratch("script.json");
    // New Game on frame 2, then a few frames of play.
    std::fs::write(&script, r#"{"steps":[{"at":2,"press":"enter"}]}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_farm-player"))
        .args(["--screenshot", out.to_str().unwrap(), "--cart", cart().to_str().unwrap()])
        .args(["--size", "320x200", "--frames", "8", "--script", script.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let png = std::fs::read(&out).unwrap();
    let image = farm_render::images::decode_image(&png).unwrap();
    assert_eq!((image.width(), image.height()), (320, 200));

    let bad = Command::new(env!("CARGO_BIN_EXE_farm-player")).args(["--size", "12"]).output().unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("--size"));
}

/// Runs the real window under Xvfb for a few frames with `renderer` and returns its stderr, or
/// `None` (with a note) where no virtual display can be started, unless `FARM_REQUIRE_DISPLAY=1`
/// (CI, which installs Xvfb) makes that a failure: a missing display must not turn the test
/// green there.
#[cfg(all(feature = "desktop", target_os = "linux"))]
fn run_window(renderer: &str, name: &str) -> Option<String> {
    let required = std::env::var("FARM_REQUIRE_DISPLAY").is_ok_and(|value| value == "1");
    let available = Command::new("xvfb-run").arg("--help").output().is_ok_and(|output| output.status.success());
    if !available {
        assert!(!required, "FARM_REQUIRE_DISPLAY=1 but xvfb-run is not installed");
        eprintln!("skipped: xvfb-run is not installed");
        return None;
    }
    let user = scratch(name);
    let output = Command::new("xvfb-run")
        .args(["-a", "-s", "-screen 0 1280x800x24", env!("CARGO_BIN_EXE_farm-player")])
        .args(["--cart", cart().to_str().unwrap(), "--exit-after-frames", "5", "--windowed"])
        .args(["--renderer", renderer])
        .env("FARM_PLAYER_USER_DIR", &user)
        .env_remove("FARM_RENDERER")
        .output()
        .unwrap();
    // xvfb-run sends the program's stderr to stdout.
    let stderr = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    if !output.status.success() && (stderr.contains("Cannot open a window") || stderr.contains("xvfb-run: error")) {
        assert!(!required, "FARM_REQUIRE_DISPLAY=1 but no display server could be started: {stderr}");
        eprintln!("skipped: no display server could be started: {stderr}");
        return None;
    }
    assert!(output.status.success(), "{stderr}");
    assert!(!stderr.contains("Crash log"), "{stderr}");
    Some(stderr)
}

#[cfg(all(feature = "desktop", target_os = "linux"))]
#[test]
fn the_window_opens_renders_and_exits() {
    let Some(stderr) = run_window("cpu", "user-cpu") else { return };
    assert!(stderr.contains("Renderer: CPU (chosen)"), "{stderr}");
}

/// The window through the GPU renderer: `gpu` accepts a software adapter (lavapipe in CI), and
/// without any adapter the game must fall back to the CPU, never fail. `FARM_REQUIRE_GPU=1` makes
/// that fallback a failure. `auto` takes hardware adapters only, else the CPU.
#[cfg(all(feature = "gpu", target_os = "linux"))]
#[test]
fn the_window_renders_through_the_gpu_or_falls_back() {
    let Some(stderr) = run_window("gpu", "user-gpu") else { return };
    let required = std::env::var("FARM_REQUIRE_GPU").is_ok_and(|value| value == "1");
    if stderr.contains("Renderer: GPU, ") {
        assert!(!stderr.contains("the GPU renderer stopped"), "{stderr}");
    } else {
        assert!(!required, "FARM_REQUIRE_GPU=1 but the window fell back to the CPU: {stderr}");
        assert!(stderr.contains("Renderer: CPU (the GPU renderer is not available"), "{stderr}");
        eprintln!("the GPU renderer was not available; the window fell back to the CPU");
    }
    let Some(stderr) = run_window("auto", "user-auto") else { return };
    assert!(stderr.contains("Renderer: GPU, ") || stderr.contains("Renderer: CPU ("), "{stderr}");
    assert!(!stderr.contains("the GPU renderer stopped"), "{stderr}");
}
