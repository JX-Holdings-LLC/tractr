//! The desktop GPU renderer against the CPU renderer on the screenshot goldens' screens (title,
//! settings, gameplay HUD, dialogue, shop at 1280×800 and 1920×1080; a pack minigame, the samples,
//! a walk with the camera between pixels, Spanish with the readable font): each frame is composed
//! once ([`Player::frame_composition`]), drawn offscreen by farm-gpu, then rasterized on the CPU
//! ([`Player::compose_pixels`]), and the two must agree within a small tolerance.
//!
//! Without a GPU adapter (on Linux, `mesa-vulkan-drivers` provides lavapipe) the test prints why
//! and passes, unless `FARM_REQUIRE_GPU=1` makes that a failure. `FARM_GPU_SCREENSHOTS=<dir>`
//! writes both frames of every screen.

mod common;

use common::{scene, FRAME};
use farm_gpu::GpuRenderer;
use farm_player::Player;
use farm_render::tiny_skia::{IntSize, Pixmap};

/// A channel off by more than this counts as a different pixel.
const CHANNEL_TOLERANCE: i32 = 6;
/// At most this fraction of a frame's pixels may differ.
const MAX_DIFFERENT_FRACTION: f64 = 0.002;

fn gpu() -> Option<GpuRenderer> {
    match GpuRenderer::headless(true) {
        Ok(renderer) => {
            eprintln!("GPU adapter: {}", renderer.summary());
            Some(renderer)
        }
        Err(error) => {
            let required = std::env::var("FARM_REQUIRE_GPU").is_ok_and(|value| value == "1");
            assert!(!required, "FARM_REQUIRE_GPU=1 but there is no GPU adapter: {error}");
            eprintln!("skipped: {error}");
            None
        }
    }
}

/// Composes the next frame and draws it both ways; returns a failure message when they differ.
fn compare(gpu: &mut GpuRenderer, name: &str, player: &mut Player, width: u32, height: u32) -> Option<String> {
    let (_, composition) = player.frame_composition(FRAME, &[], width, height).unwrap();
    gpu.render(&composition).unwrap();
    let (w, h, gpu_pixels) = gpu.read_frame().unwrap();
    assert_eq!((w, h), (width, height));
    let cpu = player.compose_pixels().unwrap().clone();
    let mut different = 0;
    let mut largest = 0;
    for (a, b) in cpu.data().chunks_exact(4).zip(gpu_pixels.chunks_exact(4)) {
        let diff = a.iter().zip(b).map(|(x, y)| (i32::from(*x) - i32::from(*y)).abs()).max().unwrap_or(0);
        largest = largest.max(diff);
        if diff > CHANNEL_TOLERANCE {
            different += 1;
        }
    }
    eprintln!("{name} {width}×{height}: {different} pixels differ (largest channel difference {largest})");
    if let Some(folder) = std::env::var_os("FARM_GPU_SCREENSHOTS") {
        let folder = std::path::PathBuf::from(folder);
        std::fs::create_dir_all(&folder).unwrap();
        let file = |kind: &str| folder.join(format!("{name}-{width}x{height}-{kind}.png"));
        std::fs::write(file("cpu"), farm_render::encode_png(&cpu)).unwrap();
        let gpu_pixmap = Pixmap::from_vec(gpu_pixels, IntSize::from_wh(width, height).unwrap()).unwrap();
        std::fs::write(file("gpu"), farm_render::encode_png(&gpu_pixmap)).unwrap();
    }
    let allowed = (f64::from(width * height) * MAX_DIFFERENT_FRACTION) as usize;
    (different > allowed).then(|| format!("{name} {width}×{height}: {different} pixels differ (at most {allowed})"))
}

/// Steps a screen past the toasts' fade-in, like the screenshot goldens, then compares a frame.
fn compare_scene(gpu: &mut GpuRenderer, name: &str, width: u32, height: u32) -> Option<String> {
    let mut player = scene(name);
    for _ in 0..20 {
        player.step(FRAME, &[], width, height).unwrap();
    }
    compare(gpu, name, &mut player, width, height)
}

#[test]
fn the_gpu_renderer_draws_the_screenshot_screens_like_the_cpu() {
    let Some(mut gpu) = gpu() else { return };
    let mut failures = Vec::new();
    for name in ["title", "settings", "hud", "dialogue", "shop"] {
        for (width, height) in [(1280, 800), (1920, 1080)] {
            failures.extend(compare_scene(&mut gpu, name, width, height));
        }
    }
    for name in ["pack-minigame", "sample-cozy", "sample-quest"] {
        failures.extend(compare_scene(&mut gpu, name, 1280, 800));
    }
    // Odd sizes: fitted (non-integer) scaling and a small letterboxed window.
    let mut fitted = scene("hud");
    let mut settings = fitted.settings().clone();
    settings.display.integer_scaling = false;
    fitted.set_settings(settings);
    failures.extend(compare(&mut gpu, "hud-fitted", &mut fitted, 1366, 768));
    failures.extend(compare_scene(&mut gpu, "title", 800, 500));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_gpu_renderer_follows_a_moving_camera_and_other_settings() {
    let Some(mut gpu) = gpu() else { return };
    let mut failures = Vec::new();
    // Walking: the camera sits between pixels and the world's sprites move.
    let mut player = scene("hud");
    player.step(FRAME, &[common::key_down("d")], 1280, 800).unwrap();
    for frame in 0..24 {
        if frame % 6 == 5 {
            failures.extend(compare(&mut gpu, &format!("walk-{frame}"), &mut player, 1280, 800));
        } else {
            player.step(FRAME, &[], 1280, 800).unwrap();
        }
    }
    // Spanish with the readable font, at a large text size.
    let mut localized = scene("shop");
    let mut settings = localized.settings().clone();
    settings.language = "es".into();
    settings.accessibility.readable_font = true;
    settings.accessibility.text_size = 1.4;
    localized.set_settings(settings);
    for _ in 0..20 {
        localized.step(FRAME, &[], 1920, 1080).unwrap();
    }
    failures.extend(compare(&mut gpu, "shop-es-readable", &mut localized, 1920, 1080));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
