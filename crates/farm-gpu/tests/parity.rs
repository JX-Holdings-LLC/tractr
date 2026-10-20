//! The GPU renderer against the CPU rasterizer: draw lists with every kind of command, rendered
//! offscreen through wgpu and compared with `farm_render::Rasterizer` within a small tolerance.
//!
//! Without a GPU adapter (a machine with no Vulkan, DirectX 12 or Metal device; on Linux CI
//! `mesa-vulkan-drivers` provides lavapipe) the tests print why and pass, unless
//! `FARM_REQUIRE_GPU=1` makes a missing adapter a failure.

use farm_gpu::GpuRenderer;
use farm_render::tiny_skia::{Pixmap, Transform};
use farm_render::{
    Color, Composition, DrawCmd, DrawList, FontId, Image, ImageStore, Rasterizer, Rect, Sampling, TextAlign, TextStroke,
};

/// A channel off by more than this counts as a different pixel.
const CHANNEL_TOLERANCE: i32 = 6;
/// At most this fraction of the pixels may differ.
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

/// The CPU frame of a UI-only composition.
fn cpu_frame(width: u32, height: u32, background: Color, list: &DrawList, images: &ImageStore) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).unwrap();
    pixmap.fill(farm_render::tiny_skia::Color::from_rgba8(background.r, background.g, background.b, background.a));
    Rasterizer::new().render(list, images, &mut pixmap.as_mut(), Transform::identity());
    pixmap
}

fn gpu_frame(
    renderer: &mut GpuRenderer,
    width: u32,
    height: u32,
    background: Color,
    list: &DrawList,
    images: &ImageStore,
) -> Vec<u8> {
    let composition = Composition { width, height, letterbox: background, world: None, ui: list, ui_images: images };
    renderer.render(&composition).unwrap();
    let (w, h, pixels) = renderer.read_frame().unwrap();
    assert_eq!((w, h), (width, height));
    pixels
}

/// Pixels whose largest channel difference is over the tolerance, and the largest difference.
fn differences(cpu: &[u8], gpu: &[u8]) -> (usize, i32) {
    assert_eq!(cpu.len(), gpu.len());
    let mut count = 0;
    let mut largest = 0;
    for (a, b) in cpu.chunks_exact(4).zip(gpu.chunks_exact(4)) {
        let diff = a.iter().zip(b).map(|(x, y)| (i32::from(*x) - i32::from(*y)).abs()).max().unwrap_or(0);
        largest = largest.max(diff);
        if diff > CHANNEL_TOLERANCE {
            count += 1;
        }
    }
    (count, largest)
}

fn check(name: &str, renderer: &mut GpuRenderer, width: u32, height: u32, list: &DrawList, images: &ImageStore) {
    let background = Color::rgb(0x2a, 0x22, 0x1c);
    let cpu = cpu_frame(width, height, background, list, images);
    let gpu = gpu_frame(renderer, width, height, background, list, images);
    let (count, largest) = differences(cpu.data(), &gpu);
    let fraction = count as f64 / f64::from(width * height);
    eprintln!("{name}: {count} of {} pixels differ (largest channel difference {largest})", width * height);
    if let Some(folder) = std::env::var_os("FARM_GPU_SCREENSHOTS") {
        let folder = std::path::PathBuf::from(folder);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(format!("{name}-cpu.png")), farm_render::encode_png(&cpu)).unwrap();
        let gpu = Pixmap::from_vec(gpu, farm_render::tiny_skia::IntSize::from_wh(width, height).unwrap()).unwrap();
        std::fs::write(folder.join(format!("{name}-gpu.png")), farm_render::encode_png(&gpu)).unwrap();
    }
    assert!(fraction <= MAX_DIFFERENT_FRACTION, "{name}: {count} pixels differ ({:.3} %)", fraction * 100.0);
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, w, h)
}

fn shapes() -> DrawList {
    let mut list = DrawList::new();
    list.push(DrawCmd::FillRect {
        rect: rect(10.0, 10.0, 100.0, 60.0),
        color: Color::rgb(200, 60, 40),
        anti_alias: false,
    });
    list.push(DrawCmd::FillRect {
        rect: rect(20.3, 80.6, 90.4, 30.2),
        color: Color::rgba(40, 200, 90, 180),
        anti_alias: false,
    });
    list.fill_rect(rect(130.0, 10.0, 80.0, 50.0), Color::rgba(250, 230, 120, 220));
    list.fill_rect(rect(130.25, 70.5, 80.5, 40.75), Color::rgba(90, 120, 250, 160));
    list.fill_round_rect(rect(230.0, 10.0, 120.0, 70.0), 14.0, Color::rgba(240, 240, 240, 230));
    list.fill_round_rect(rect(230.5, 90.25, 60.0, 30.0), 40.0, Color::rgba(20, 20, 20, 140));
    list.stroke_rect(rect(10.5, 130.5, 100.0, 40.0), Color::rgb(255, 255, 255), 2.0);
    list.stroke_round_rect(rect(130.0, 130.0, 100.0, 40.0), 8.0, Color::rgba(255, 200, 0, 255), 3.0);
    list.fill_circle(290.0, 150.0, 20.0, Color::rgba(0, 180, 255, 200));
    list.push(DrawCmd::StrokeCircle { cx: 340.0, cy: 150.0, radius: 15.5, color: Color::rgb(255, 80, 80), width: 2.5 });
    list.push(DrawCmd::FillOval { rect: rect(10.0, 190.0, 80.0, 30.0), color: Color::rgba(160, 90, 40, 255) });
    list.push(DrawCmd::BlurOval { rect: rect(110.0, 190.0, 60.0, 20.0), color: Color::rgba(0, 0, 0, 120), sigma: 3.0 });
    list.push(DrawCmd::BlurRect {
        rect: rect(190.0, 185.0, 70.0, 40.0),
        color: Color::rgba(255, 220, 120, 160),
        sigma: 6.0,
    });
    list.line((280.0, 190.0), (380.0, 230.0), Color::rgb(30, 220, 30), 3.0);
    // Nested clips under a translation and a scale.
    list.save();
    list.translate(20.0, 240.0);
    list.clip_rect(rect(0.0, 0.0, 150.0, 60.0));
    list.scale(1.5, 1.5);
    list.fill_round_rect(rect(-10.0, 5.0, 200.0, 30.0), 10.0, Color::rgba(120, 60, 200, 255));
    list.clip_rect(rect(10.0, 0.0, 40.0, 100.0));
    list.fill_circle(30.0, 20.0, 25.0, Color::rgba(255, 255, 0, 200));
    list.restore();
    // A mirrored (negative scale) transform goes through the CPU sprite path.
    list.save();
    list.translate(380.0, 250.0);
    list.scale(-1.0, 1.0);
    list.fill_rect(rect(0.0, 0.0, 100.0, 40.0), Color::rgb(80, 160, 80));
    list.restore();
    // The day/night modulate over part of the scene.
    list.push(DrawCmd::ModulateRect { rect: rect(0.0, 180.0, 200.0, 120.0), color: Color::rgb(120, 140, 220) });
    list
}

fn texts() -> DrawList {
    let mut list = DrawList::new();
    let text = |list: &mut DrawList, content: &str, x: f32, y: f32, size: f32, color: Color, align: TextAlign| {
        list.push(DrawCmd::Text {
            text: content.into(),
            x,
            y,
            font: FontId::Regular,
            size,
            color,
            stroke: None,
            align,
        });
    };
    text(&mut list, "Harvest festival in 3 days", 10.3, 30.0, 16.0, Color::rgb(250, 240, 220), TextAlign::Left);
    text(&mut list, "Fading toast", 10.0, 60.7, 14.0, Color::rgba(250, 240, 220, 90), TextAlign::Left);
    text(&mut list, "Centred", 200.0, 100.0, 24.5, Color::rgb(255, 200, 80), TextAlign::Center);
    text(&mut list, "Right $1,250", 390.0, 130.0, 13.0, Color::rgb(180, 220, 255), TextAlign::Right);
    list.push(DrawCmd::Text {
        text: "Bold +12".into(),
        x: 20.0,
        y: 170.0,
        font: FontId::Bold,
        size: 18.0,
        color: Color::rgb(255, 217, 74),
        stroke: Some(TextStroke { color: Color::rgba(0, 0, 0, 0x90), width: 3.0 }),
        align: TextAlign::Left,
    });
    list.push(DrawCmd::Text {
        text: "Readable".into(),
        x: 200.0,
        y: 170.0,
        font: FontId::ReadableBold,
        size: 20.0,
        color: Color::rgb(220, 255, 220),
        stroke: None,
        align: TextAlign::Left,
    });
    // Larger than the glyph sprites: filled as paths.
    text(&mut list, "Big", 10.0, 360.0, 180.0, Color::rgb(140, 200, 120), TextAlign::Left);
    // Scaled text.
    list.save();
    list.scale(1.35, 1.35);
    text(&mut list, "Scaled interface", 200.0, 150.0, 15.0, Color::WHITE, TextAlign::Left);
    list.restore();
    list
}

/// A 16×8 sheet of two sprites with transparent and translucent pixels.
fn sheet(store: &mut ImageStore) -> farm_render::ImageId {
    let (width, height) = (16u32, 8u32);
    let mut pixels = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let alpha = if (x + y) % 5 == 0 {
                0
            } else if x % 3 == 0 {
                128
            } else {
                255
            };
            pixels.extend_from_slice(&[(x * 16) as u8, (y * 32) as u8, if x < 8 { 200 } else { 40 }, alpha]);
        }
    }
    store.insert("test://sheet", Image::from_rgba(width, height, pixels).unwrap())
}

fn images(store: &mut ImageStore) -> DrawList {
    let id = sheet(store);
    let mut list = DrawList::new();
    let image = |list: &mut DrawList, src: Rect, dst: Rect, opacity: f32, sampling: Sampling, tint: Option<Color>| {
        list.push(DrawCmd::Image { image: id, src, dst, opacity, sampling, tint });
    };
    image(&mut list, rect(0.0, 0.0, 8.0, 8.0), rect(10.0, 10.0, 64.0, 64.0), 1.0, Sampling::Nearest, None);
    image(&mut list, rect(8.0, 0.0, 8.0, 8.0), rect(90.0, 10.0, 40.0, 40.0), 1.0, Sampling::Nearest, None);
    image(&mut list, rect(0.0, 0.0, 16.0, 8.0), rect(150.0, 10.0, 96.0, 48.0), 0.6, Sampling::Nearest, None);
    image(
        &mut list,
        rect(0.0, 0.0, 8.0, 8.0),
        rect(260.0, 10.0, 48.0, 48.0),
        1.0,
        Sampling::Nearest,
        Some(Color::rgb(240, 184, 112)),
    );
    image(&mut list, rect(2.5, 1.5, 5.0, 5.0), rect(320.0, 10.3, 37.5, 37.5), 1.0, Sampling::Nearest, None);
    image(&mut list, rect(0.0, 0.0, 16.0, 8.0), rect(10.0, 100.0, 160.0, 80.0), 1.0, Sampling::Smooth, None);
    image(&mut list, rect(8.0, 0.0, 8.0, 8.0), rect(190.0, 100.0, 70.0, 70.0), 0.8, Sampling::Smooth, None);
    // Under a UI scale.
    list.save();
    list.scale(1.35, 1.35);
    image(&mut list, rect(0.0, 0.0, 8.0, 8.0), rect(200.0, 80.0, 32.0, 32.0), 1.0, Sampling::Nearest, None);
    list.restore();
    list
}

#[test]
fn shapes_match_the_cpu_rasterizer() {
    let Some(mut renderer) = gpu() else { return };
    check("shapes", &mut renderer, 400, 320, &shapes(), &ImageStore::default());
}

#[test]
fn text_matches_the_cpu_rasterizer() {
    let Some(mut renderer) = gpu() else { return };
    check("text", &mut renderer, 400, 380, &texts(), &ImageStore::default());
}

#[test]
fn images_match_the_cpu_rasterizer() {
    let Some(mut renderer) = gpu() else { return };
    let mut store = ImageStore::default();
    let list = images(&mut store);
    check("images", &mut renderer, 400, 220, &list, &store);
}

#[test]
fn frames_reuse_the_atlases_and_survive_resizes() {
    let Some(mut renderer) = gpu() else { return };
    let mut store = ImageStore::default();
    let list = images(&mut store);
    // The same renderer through several sizes and lists: the caches carry over correctly.
    check("again-images", &mut renderer, 400, 220, &list, &store);
    check("again-text", &mut renderer, 420, 400, &texts(), &ImageStore::default());
    check("again-shapes", &mut renderer, 400, 320, &shapes(), &ImageStore::default());
    check("again-images-2", &mut renderer, 401, 221, &list, &store);
}

/// Text in many sizes needs more glyph room than the first atlas has: it is cleared, then grown.
#[test]
fn full_atlases_are_cleared_and_grown() {
    let Some(mut renderer) = gpu() else { return };
    let mut list = DrawList::new();
    for (row, size) in [40.0f32, 70.0, 100.0, 130.0, 155.0].into_iter().enumerate() {
        let y = 40.0 + row as f32 * 150.0;
        list.text("ABCDEFGHIJKLMNOPQRSTUVWXYZ", 4.0, y, FontId::Bold, size, Color::rgb(230, 230, 200));
        list.text("abcdefghijklmnopqrstuvwxyz", 4.0, y + size * 0.5, FontId::Regular, size * 0.9, Color::WHITE);
    }
    check("atlas-growth", &mut renderer, 1800, 800, &list, &ImageStore::default());
    assert!(renderer.atlas_sides().0 > 1024, "the glyph atlas grew: {:?}", renderer.atlas_sides());
    check("atlas-growth-again", &mut renderer, 1800, 800, &list, &ImageStore::default());
}

#[test]
fn failures_are_errors_not_panics() {
    let Some(mut renderer) = gpu() else { return };
    let list = shapes();
    let images = ImageStore::default();
    let huge =
        Composition { width: 20_000, height: 100, letterbox: Color::BLACK, world: None, ui: &list, ui_images: &images };
    assert_eq!(renderer.render(&huge), Err(farm_gpu::GpuError::TooLarge));
    // Still usable after refusing a frame.
    check("after-too-large", &mut renderer, 400, 320, &list, &images);
    // A lost device (a driver reset) makes every later call an error.
    renderer.lose_device();
    let frame =
        Composition { width: 64, height: 64, letterbox: Color::BLACK, world: None, ui: &list, ui_images: &images };
    let result = renderer.render(&frame).and_then(|()| renderer.read_frame().map(|_| ()));
    assert!(matches!(result, Err(farm_gpu::GpuError::Lost(_))), "{result:?}");
    assert!(matches!(renderer.render(&frame), Err(farm_gpu::GpuError::Lost(_))));
}
