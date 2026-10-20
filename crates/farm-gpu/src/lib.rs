//! `farm-gpu`: a [wgpu] renderer for farm-render's draw lists, the desktop player's GPU backend.
//!
//! It executes the same [`Composition`] the CPU path rasterizes (farm-player's `FrameRenderer`),
//! so the two look the same; the CPU rasterizer stays the reference (goldens, screenshots, the
//! editor's Play Mode, the web player). A frame is drawn in passes, so later effects (lighting,
//! palette swaps, particles) can slot in between:
//!
//! 1. **World** into an offscreen texture at the native pixel-art size: the world's draw list,
//!    then the UI's backdrop tints.
//! 2. **Frame** into an offscreen texture at the window's size: the letterbox colour, the native
//!    world scaled up with nearest neighbour (the CPU path's exact columns and rows), then the UI
//!    at full resolution.
//! 3. **Present**: the frame copied into the window's surface (or read back, in tests).
//!
//! How each command is drawn is in [`encode`]: rectangles, rounded rectangles, images and glyphs
//! are drawn by the shader with the CPU's arithmetic; everything else is rasterized once by the
//! CPU rasterizer into a cached sprite. Glyphs and sprites live in two atlases that are cleared
//! (and grown, up to the device's limit) when a frame needs more room.
//!
//! Failures are errors, never panics or black frames: no adapter, a device that cannot be created
//! or is lost, a validation error, a surface that cannot be configured. The host then falls back to
//! the CPU renderer ([`GpuError`]). Only Vulkan, DirectX 12 and Metal are enabled (no OpenGL).

mod atlas;
pub mod encode;
mod window;

pub use window::{Presented, WindowRenderer};

use encode::{Atlases, Batch, Blend, Encoder, Full, ImageRef, Instance, INSTANCE_SIZE};
use farm_render::tiny_skia::Transform;
use farm_render::{Color, Composition, ImageId, ImageStore};
use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

/// Format of the offscreen targets: plain RGBA8, so blending works on the stored values exactly
/// like the CPU rasterizer's (no sRGB conversion).
const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Starting atlas sizes (texels per side); they double when a frame needs more.
const GLYPH_ATLAS: u32 = 1024;
const SPRITE_ATLAS: u32 = 2048;
/// Largest texture side used, whatever the device allows.
const MAX_SIDE: u32 = 8192;
/// Images not drawn for this many frames lose their textures.
const IMAGE_FRAMES: u64 = 600;

/// Why the GPU renderer cannot draw (the host falls back to the CPU renderer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuError {
    /// No suitable adapter (no Vulkan, DirectX 12 or Metal device; or only a software one when
    /// those are not allowed).
    Unavailable(String),
    /// The device could not be created.
    Device(String),
    /// The window's surface could not be created or configured.
    Surface(String),
    /// The device was lost or reported an error; the renderer cannot be used any more.
    Lost(String),
    /// The frame is larger than the device's textures, or needs more atlas room than it has.
    TooLarge,
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::Unavailable(message) => write!(f, "no GPU adapter: {message}"),
            GpuError::Device(message) => write!(f, "the GPU device could not be created: {message}"),
            GpuError::Surface(message) => write!(f, "the window surface failed: {message}"),
            GpuError::Lost(message) => write!(f, "the GPU device failed: {message}"),
            GpuError::TooLarge => write!(f, "the frame is too large for the GPU renderer"),
        }
    }
}

impl std::error::Error for GpuError {}

/// Runs a wgpu future to completion (native wgpu resolves them at once).
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        std::thread::yield_now();
    }
}

fn new_instance() -> wgpu::Instance {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle().with_env();
    descriptor.backends &= wgpu::Backends::VULKAN | wgpu::Backends::DX12 | wgpu::Backends::METAL;
    wgpu::Instance::new(descriptor)
}

/// A one-line description of an adapter, for logs.
fn describe(info: &wgpu::AdapterInfo) -> String {
    let kind = match info.device_type {
        wgpu::DeviceType::IntegratedGpu => "integrated GPU",
        wgpu::DeviceType::DiscreteGpu => "discrete GPU",
        wgpu::DeviceType::VirtualGpu => "virtual GPU",
        wgpu::DeviceType::Cpu => "software",
        wgpu::DeviceType::Other => "GPU",
    };
    format!("{} ({kind}, {:?})", info.name.trim(), info.backend)
}

fn request_adapter(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
    allow_software: bool,
) -> Result<wgpu::Adapter, GpuError> {
    let options = wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: surface,
        apply_limit_buckets: false,
    };
    let adapter =
        block_on(instance.request_adapter(&options)).map_err(|error| GpuError::Unavailable(error.to_string()))?;
    let info = adapter.get_info();
    if info.device_type == wgpu::DeviceType::Cpu && !allow_software {
        return Err(GpuError::Unavailable(format!("only a software adapter: {}", describe(&info))));
    }
    Ok(adapter)
}

/// A texture the renderer draws into and samples.
#[derive(Debug)]
struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// Binds it as the `image` of a batch.
    image: wgpu::BindGroup,
    width: u32,
    height: u32,
}

#[derive(Debug)]
struct GpuImage {
    group: wgpu::BindGroup,
    used: u64,
    _texture: wgpu::Texture,
}

/// What [`GpuRenderer::encode`] produced for one frame.
#[derive(Debug, Default)]
struct Encoded {
    instances: Vec<Instance>,
    world: Vec<Batch>,
    frame: Vec<Batch>,
    images: Vec<(u64, ImageId)>,
    /// The lookup rows: width, then `rows` rows of `width` values.
    lut_width: u32,
    lut: Vec<u32>,
    native: Option<(u32, u32)>,
}

/// Draws [`Composition`]s into an offscreen frame texture.
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    info: wgpu::AdapterInfo,
    failure: Arc<Mutex<Option<String>>>,
    max_side: u32,
    group0_layout: wgpu::BindGroupLayout,
    group1_layout: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    over: wgpu::RenderPipeline,
    modulate: wgpu::RenderPipeline,
    replace: wgpu::RenderPipeline,
    atlases: Atlases,
    glyph_texture: wgpu::Texture,
    sprite_texture: wgpu::Texture,
    lut_texture: wgpu::Texture,
    world_uniform: wgpu::Buffer,
    frame_uniform: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    blank: wgpu::BindGroup,
    native: Option<Target>,
    frame: Option<Target>,
    images: BTreeMap<(u64, ImageId), GpuImage>,
    frames: u64,
    encoded: Encoded,
    /// The pipeline copying the frame into a window surface of this format.
    present: Option<wgpu::RenderPipeline>,
}

impl std::fmt::Debug for GpuRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuRenderer")
            .field("adapter", &self.summary())
            .field("frames", &self.frames)
            .finish_non_exhaustive()
    }
}

fn texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("farm-gpu"),
        size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

fn sampled_entry(binding: u32, sample_type: wgpu::TextureSampleType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    let attributes = wgpu::vertex_attr_array![0 => Float32x4, 1 => Uint32x4, 2 => Float32x4, 3 => Float32x4];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("farm-gpu draw"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: INSTANCE_SIZE as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &attributes,
            })],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// The premultiplied bytes a CPU pixmap is filled with for `color` (tiny-skia's `fill`).
fn clear_color(color: Color) -> wgpu::Color {
    let premultiplied =
        farm_render::tiny_skia::Color::from_rgba8(color.r, color.g, color.b, color.a).premultiply().to_color_u8();
    let unit = |byte: u8| f64::from(byte) / 255.0;
    wgpu::Color {
        r: unit(premultiplied.red()),
        g: unit(premultiplied.green()),
        b: unit(premultiplied.blue()),
        a: unit(premultiplied.alpha()),
    }
}

impl GpuRenderer {
    /// A renderer on the best adapter, without a window (tests, offscreen rendering).
    /// `allow_software`: accept a software adapter (llvmpipe, WARP).
    pub fn headless(allow_software: bool) -> Result<Self, GpuError> {
        let instance = new_instance();
        let adapter = request_adapter(&instance, None, allow_software)?;
        Self::new(&adapter)
    }

    fn new(adapter: &wgpu::Adapter) -> Result<Self, GpuError> {
        let info = adapter.get_info();
        let max_side = adapter.limits().max_texture_dimension_2d.min(MAX_SIDE);
        if max_side < 2048 {
            return Err(GpuError::Unavailable(format!("{}: textures of at most {max_side} pixels", describe(&info))));
        }
        let limits = wgpu::Limits { max_texture_dimension_2d: max_side, ..wgpu::Limits::downlevel_defaults() };
        let descriptor = wgpu::DeviceDescriptor {
            label: Some("farm-gpu"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            ..wgpu::DeviceDescriptor::default()
        };
        let (device, queue) =
            block_on(adapter.request_device(&descriptor)).map_err(|error| GpuError::Device(error.to_string()))?;
        let failure = Arc::new(Mutex::new(None::<String>));
        let errors = Arc::clone(&failure);
        device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
            if let Ok(mut slot) = errors.lock() {
                slot.get_or_insert_with(|| error.to_string());
            }
        }));
        let lost = Arc::clone(&failure);
        device.set_device_lost_callback(move |reason, message| {
            if let Ok(mut slot) = lost.lock() {
                slot.get_or_insert_with(|| format!("device lost ({reason:?}): {message}"));
            }
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("farm-gpu draw.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("draw.wgsl").into()),
        });
        let float = wgpu::TextureSampleType::Float { filterable: false };
        let group0_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("farm-gpu targets"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                sampled_entry(1, float),
                sampled_entry(2, float),
                sampled_entry(3, wgpu::TextureSampleType::Uint),
            ],
        });
        let group1_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("farm-gpu image"),
            entries: &[sampled_entry(0, float)],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("farm-gpu"),
            bind_group_layouts: &[Some(&group0_layout), Some(&group1_layout)],
            immediate_size: 0,
        });
        let modulate_component = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Dst,
            dst_factor: wgpu::BlendFactor::Zero,
            operation: wgpu::BlendOperation::Add,
        };
        let over =
            pipeline(&device, &layout, &shader, TARGET_FORMAT, Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING));
        let modulate = pipeline(
            &device,
            &layout,
            &shader,
            TARGET_FORMAT,
            Some(wgpu::BlendState { color: modulate_component, alpha: modulate_component }),
        );
        let replace = pipeline(&device, &layout, &shader, TARGET_FORMAT, None);

        let sampled = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        let glyph_texture = texture(&device, GLYPH_ATLAS, GLYPH_ATLAS, wgpu::TextureFormat::R8Unorm, sampled);
        let sprite_texture = texture(&device, SPRITE_ATLAS, SPRITE_ATLAS, TARGET_FORMAT, sampled);
        let lut_texture = texture(&device, 1, 1, wgpu::TextureFormat::R32Uint, sampled);
        let uniform = |label| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let world_uniform = uniform("farm-gpu world size");
        let frame_uniform = uniform("farm-gpu frame size");
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("farm-gpu instances"),
            size: (INSTANCE_SIZE * 1024) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let blank_texture = texture(&device, 1, 1, TARGET_FORMAT, sampled);
        let blank = image_group(&device, &group1_layout, &blank_texture.create_view(&Default::default()));
        let renderer = Self {
            device,
            queue,
            info,
            failure,
            max_side,
            group0_layout,
            group1_layout,
            layout,
            shader,
            over,
            modulate,
            replace,
            atlases: Atlases::new(GLYPH_ATLAS, SPRITE_ATLAS),
            glyph_texture,
            sprite_texture,
            lut_texture,
            world_uniform,
            frame_uniform,
            instance_buffer,
            blank,
            native: None,
            frame: None,
            images: BTreeMap::new(),
            frames: 0,
            encoded: Encoded::default(),
            present: None,
        };
        renderer.check()?;
        Ok(renderer)
    }

    /// The adapter, for logs ("NVIDIA GeForce … (discrete GPU, Vulkan)").
    pub fn summary(&self) -> String {
        describe(&self.info)
    }

    /// Whether the adapter is a software rasterizer (llvmpipe, WARP).
    pub fn is_software(&self) -> bool {
        self.info.device_type == wgpu::DeviceType::Cpu
    }

    /// The glyph and sprite atlases' sides, in texels (tests).
    #[doc(hidden)]
    pub fn atlas_sides(&self) -> (u32, u32) {
        (self.atlases.glyph_packer.size().0, self.atlases.sprite_packer.size().0)
    }

    /// Destroys the device, as a driver reset would (tests of the fallback).
    #[doc(hidden)]
    pub fn lose_device(&self) {
        self.device.destroy();
    }

    /// The first error the device reported, if any.
    fn check(&self) -> Result<(), GpuError> {
        match self.failure.lock() {
            Ok(slot) => match slot.as_ref() {
                Some(message) => Err(GpuError::Lost(message.clone())),
                None => Ok(()),
            },
            Err(_) => Err(GpuError::Lost("error state poisoned".to_owned())),
        }
    }

    /// A render target of `width`×`height` (kept while the size stays).
    fn target(&mut self, frame: bool, width: u32, height: u32) {
        let slot = if frame { &self.frame } else { &self.native };
        if slot.as_ref().is_some_and(|target| target.width == width && target.height == height) {
            return;
        }
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST;
        let texture = texture(&self.device, width, height, TARGET_FORMAT, usage);
        let view = texture.create_view(&Default::default());
        let image = image_group(&self.device, &self.group1_layout, &view);
        let target = Some(Target { texture, view, image, width, height });
        if frame {
            self.frame = target;
        } else {
            self.native = target;
        }
    }

    /// Encodes the frame's draw lists; an atlas without room for it is reported.
    fn encode(&mut self, composition: &Composition<'_>) -> Result<(), Full> {
        let encoded = &mut self.encoded;
        encoded.instances.clear();
        encoded.world.clear();
        encoded.frame.clear();
        encoded.images.clear();
        encoded.lut.clear();
        encoded.native = None;
        encoded.lut_width = 1;
        let (width, height) = (composition.width.max(1), composition.height.max(1));
        let mut upscale = None;
        if let Some(world) = &composition.world {
            let (native_w, native_h) = (world.native_width.max(1), world.native_height.max(1));
            encoded.native = Some((native_w, native_h));
            let mut encoder = Encoder {
                atlases: &mut self.atlases,
                instances: &mut encoded.instances,
                batches: &mut encoded.world,
                images_used: &mut encoded.images,
                width: native_w,
                height: native_h,
                max_side: self.max_side,
            };
            encoder.list(world.list, world.images, Transform::identity())?;
            for index in 0..world.tints.len() {
                let instance = Instance {
                    rect: [0.0, 0.0, native_w as f32, native_h as f32],
                    data: [encode::TINT, 0, 0, 2 + index as u32],
                    ..Instance::default()
                };
                encoder.push(instance, Blend::Over, None, [0, 0, native_w, native_h]);
            }
            // The lookup rows: the upscale's columns and rows, then each tint per native row.
            let (x0, y0, dest_w, dest_h) = world.upscale.destination(width, height);
            let lut_width = dest_w.max(dest_h).max(native_h as usize).max(1);
            encoded.lut_width = lut_width as u32;
            encoded.lut.resize(lut_width * (2 + world.tints.len()), 0);
            for x in 0..dest_w {
                encoded.lut[x] = world.upscale.source_column(x, native_w as usize) as u32;
            }
            for y in 0..dest_h {
                encoded.lut[lut_width + y] = world.upscale.source_row(y, native_h as usize) as u32;
            }
            for (index, (top, bottom)) in world.tints.iter().enumerate() {
                let row = &mut encoded.lut[lut_width * (2 + index)..lut_width * (3 + index)];
                for (native_row, out) in row.iter_mut().enumerate().take(native_h as usize) {
                    let t = world.upscale.tint_position(native_row, height);
                    if let Some([r, g, b, a]) = farm_render::compose::tint_source(*top, *bottom, t) {
                        *out = r | (g << 8) | (b << 16) | (a << 24);
                    }
                }
            }
            if dest_w > 0 && dest_h > 0 {
                upscale = Some([x0 as f32, y0 as f32, (x0 + dest_w) as f32, (y0 + dest_h) as f32]);
            }
        }
        if encoded.lut.is_empty() {
            encoded.lut.push(0);
        }
        let mut encoder = Encoder {
            atlases: &mut self.atlases,
            instances: &mut encoded.instances,
            batches: &mut encoded.frame,
            images_used: &mut encoded.images,
            width,
            height,
            max_side: self.max_side,
        };
        if let Some(rect) = upscale {
            let instance = Instance {
                rect,
                data: [encode::UPSCALE, 0, 0, 0],
                p0: [rect[0], rect[1], 0.0, 0.0],
                ..Instance::default()
            };
            encoder.push(instance, Blend::Replace, Some(ImageRef::Native), [0, 0, width, height]);
        }
        encoder.list(composition.ui, composition.ui_images, Transform::identity())
    }

    /// Encodes with room in the atlases: a full atlas is cleared and the frame encoded again, then
    /// grown (doubled) if the frame alone fills it.
    fn encode_with_room(&mut self, composition: &Composition<'_>) -> Result<(), GpuError> {
        let mut cleared = [false; 2];
        loop {
            match self.encode(composition) {
                Ok(()) => return Ok(()),
                Err(full) => {
                    let index = match full {
                        Full::Glyphs => 0,
                        Full::Sprites => 1,
                    };
                    if cleared[index] {
                        self.grow(full)?;
                    }
                    cleared[index] = true;
                    match full {
                        Full::Glyphs => self.atlases.clear_glyphs(),
                        Full::Sprites => self.atlases.clear_sprites(),
                    }
                }
            }
        }
    }

    fn grow(&mut self, full: Full) -> Result<(), GpuError> {
        let sampled = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        let (packer, format) = match full {
            Full::Glyphs => (&mut self.atlases.glyph_packer, wgpu::TextureFormat::R8Unorm),
            Full::Sprites => (&mut self.atlases.sprite_packer, TARGET_FORMAT),
        };
        let side = packer.size().0;
        if side >= self.max_side {
            return Err(GpuError::TooLarge);
        }
        let side = (side * 2).min(self.max_side);
        *packer = atlas::Packer::new(side, side);
        let texture = texture(&self.device, side, side, format, sampled);
        match full {
            Full::Glyphs => self.glyph_texture = texture,
            Full::Sprites => self.sprite_texture = texture,
        }
        Ok(())
    }

    /// The texture of an image, uploaded on first use.
    fn upload_image(&mut self, key: (u64, ImageId), stores: [&ImageStore; 2]) {
        let frame = self.frames;
        if let Some(image) = self.images.get_mut(&key) {
            image.used = frame;
            return;
        }
        let Some(image) = stores.iter().find(|store| store.uid() == key.0).and_then(|store| store.image(key.1)) else {
            return;
        };
        let sampled = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        let texture = texture(&self.device, image.width(), image.height(), TARGET_FORMAT, sampled);
        write_texels(&self.queue, &texture, 0, 0, image.width(), image.height(), 4, image.pixels());
        let group = image_group(&self.device, &self.group1_layout, &texture.create_view(&Default::default()));
        self.images.insert(key, GpuImage { group, used: frame, _texture: texture });
    }

    /// Draws `composition` into the frame texture (submitted, not yet read or presented).
    pub fn render(&mut self, composition: &Composition<'_>) -> Result<(), GpuError> {
        self.render_with(composition, None)
    }

    fn render_with(
        &mut self,
        composition: &Composition<'_>,
        present: Option<&wgpu::TextureView>,
    ) -> Result<(), GpuError> {
        self.check()?;
        let (width, height) = (composition.width.max(1), composition.height.max(1));
        let native = composition.world.map(|world| (world.native_width.max(1), world.native_height.max(1)));
        let too_large = |(w, h): (u32, u32)| w > self.max_side || h > self.max_side;
        if too_large((width, height)) || native.is_some_and(too_large) {
            return Err(GpuError::TooLarge);
        }
        self.frames += 1;
        self.encode_with_room(composition)?;
        if (self.encoded.lut_width as usize) > self.max_side as usize {
            return Err(GpuError::TooLarge);
        }

        // Uploads: atlas entries, images, lookup rows, instances, sizes.
        for upload in std::mem::take(&mut self.atlases.glyph_uploads) {
            write_texels(
                &self.queue,
                &self.glyph_texture,
                upload.slot.x,
                upload.slot.y,
                upload.width,
                upload.height,
                1,
                &upload.data,
            );
        }
        for upload in std::mem::take(&mut self.atlases.sprite_uploads) {
            write_texels(
                &self.queue,
                &self.sprite_texture,
                upload.slot.x,
                upload.slot.y,
                upload.width,
                upload.height,
                4,
                &upload.data,
            );
        }
        let stores = [composition.world.map_or(composition.ui_images, |world| world.images), composition.ui_images];
        for key in std::mem::take(&mut self.encoded.images) {
            self.upload_image(key, stores);
        }
        let frame = self.frames;
        self.images.retain(|_, image| frame.saturating_sub(image.used) < IMAGE_FRAMES);
        let (lut_w, lut_h) = (self.encoded.lut_width, (self.encoded.lut.len() as u32).div_ceil(self.encoded.lut_width));
        if self.lut_texture.width() < lut_w || self.lut_texture.height() < lut_h {
            let sampled = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
            let (w, h) = (lut_w.max(self.lut_texture.width()), lut_h.max(self.lut_texture.height()));
            self.lut_texture = texture(&self.device, w, h, wgpu::TextureFormat::R32Uint, sampled);
        }
        let lut_bytes: Vec<u8> = self.encoded.lut.iter().flat_map(|value| value.to_le_bytes()).collect();
        write_texels(&self.queue, &self.lut_texture, 0, 0, lut_w, lut_h, 4, &lut_bytes);
        if present.is_some() {
            let instance = Instance {
                rect: [0.0, 0.0, width as f32, height as f32],
                data: [encode::COPY, 0, 0, 0],
                ..Instance::default()
            };
            self.encoded.instances.push(instance);
        }
        let mut bytes = Vec::with_capacity(self.encoded.instances.len() * INSTANCE_SIZE);
        for instance in &self.encoded.instances {
            instance.write(&mut bytes);
        }
        if self.instance_buffer.size() < bytes.len() as u64 {
            self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("farm-gpu instances"),
                size: (bytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !bytes.is_empty() {
            self.queue.write_buffer(&self.instance_buffer, 0, &bytes);
        }
        let size = size_bytes;
        if let Some((w, h)) = self.encoded.native {
            self.queue.write_buffer(&self.world_uniform, 0, &size(w, h));
            self.target(false, w, h);
        }
        self.queue.write_buffer(&self.frame_uniform, 0, &size(width, height));
        self.target(true, width, height);

        let world_group = self.targets_group(&self.world_uniform);
        let frame_group = self.targets_group(&self.frame_uniform);

        let mut commands =
            self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("farm-gpu frame") });
        if let (Some(world), Some(native)) = (&composition.world, &self.native) {
            if self.encoded.native.is_some() {
                let mut pass = begin_pass(&mut commands, &native.view, Some(clear_color(world.background)));
                self.draw_batches(&mut pass, &world_group, &self.encoded.world);
            }
        }
        let frame_target = self.frame.as_ref().expect("the frame target was just made");
        {
            let mut pass = begin_pass(&mut commands, &frame_target.view, Some(clear_color(composition.letterbox)));
            self.draw_batches(&mut pass, &frame_group, &self.encoded.frame);
        }
        if let Some(view) = present {
            let present_pipeline = self.present.as_ref().expect("a window renderer has a present pipeline");
            let mut pass = begin_pass(&mut commands, view, None);
            pass.set_pipeline(present_pipeline);
            pass.set_bind_group(0, &frame_group, &[]);
            pass.set_bind_group(1, &frame_target.image, &[]);
            pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            let index = self.encoded.instances.len() as u32 - 1;
            pass.draw(0..6, index..index + 1);
        }
        self.queue.submit([commands.finish()]);
        self.check()
    }

    /// Bind group 0 for a target: its size, the atlases and the lookup rows.
    fn targets_group(&self, uniform: &wgpu::Buffer) -> wgpu::BindGroup {
        let glyphs = self.glyph_texture.create_view(&Default::default());
        let sprites = self.sprite_texture.create_view(&Default::default());
        let lut = self.lut_texture.create_view(&Default::default());
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("farm-gpu targets"),
            layout: &self.group0_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&glyphs) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&sprites) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&lut) },
            ],
        })
    }

    fn draw_batches(&self, pass: &mut wgpu::RenderPass<'_>, group0: &wgpu::BindGroup, batches: &[Batch]) {
        pass.set_bind_group(0, group0, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        for batch in batches {
            let image = match batch.image {
                None => Some(&self.blank),
                Some(ImageRef::Store(uid, id)) => self.images.get(&(uid, id)).map(|image| &image.group),
                Some(ImageRef::Native) => self.native.as_ref().map(|target| &target.image),
            };
            // Every image of a frame is uploaded before it is drawn; should one be missing, its
            // batch still draws its other commands, the image reading as transparent.
            let image = image.unwrap_or(&self.blank);
            pass.set_pipeline(match batch.blend {
                Blend::Over => &self.over,
                Blend::Modulate => &self.modulate,
                Blend::Replace => &self.replace,
            });
            pass.set_bind_group(1, image, &[]);
            let [x, y, w, h] = batch.scissor;
            pass.set_scissor_rect(x, y, w, h);
            pass.draw(0..6, batch.start..batch.end);
        }
    }

    /// Puts CPU pixels (premultiplied RGBA, `width`×`height`) into the frame texture, in place of
    /// a rendered frame (the error screen, a frame the CPU renderer drew).
    pub fn upload_frame(&mut self, width: u32, height: u32, pixels: &[u8]) -> Result<(), GpuError> {
        self.check()?;
        let (width, height) = (width.max(1), height.max(1));
        if width > self.max_side || height > self.max_side || pixels.len() != width as usize * height as usize * 4 {
            return Err(GpuError::TooLarge);
        }
        self.target(true, width, height);
        let frame = self.frame.as_ref().expect("the frame target was just made");
        write_texels(&self.queue, &frame.texture, 0, 0, width, height, 4, pixels);
        self.check()
    }

    /// The frame texture's pixels (premultiplied RGBA, row-major), waiting for the GPU.
    pub fn read_frame(&self) -> Result<(u32, u32, Vec<u8>), GpuError> {
        self.check()?;
        let Some(frame) = &self.frame else { return Ok((0, 0, Vec::new())) };
        let (width, height) = (frame.width, frame.height);
        let row = width * 4;
        let padded = row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("farm-gpu readback"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut commands =
            self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("farm-gpu readback") });
        commands.copy_texture_to_buffer(
            frame.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit([commands.finish()]);
        let mapped = Arc::new(Mutex::new(None));
        let done = Arc::clone(&mapped);
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
            if let Ok(mut slot) = done.lock() {
                *slot = Some(result.is_ok());
            }
        });
        self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|error| GpuError::Lost(error.to_string()))?;
        if mapped.lock().ok().and_then(|slot| *slot) != Some(true) {
            return Err(GpuError::Lost("the frame could not be read back".to_owned()));
        }
        let view = buffer.slice(..).get_mapped_range().map_err(|error| GpuError::Lost(format!("{error:?}")))?;
        let mut pixels = Vec::with_capacity(row as usize * height as usize);
        for line in view.chunks_exact(padded as usize) {
            pixels.extend_from_slice(&line[..row as usize]);
        }
        drop(view);
        buffer.unmap();
        self.check()?;
        Ok((width, height, pixels))
    }
}

/// The target-size uniform.
fn size_bytes(width: u32, height: u32) -> Vec<u8> {
    [width as f32, height as f32, 0.0, 0.0].iter().flat_map(|value| value.to_le_bytes()).collect()
}

fn image_group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, view: &wgpu::TextureView) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("farm-gpu image"),
        layout,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) }],
    })
}

#[allow(clippy::too_many_arguments)]
fn write_texels(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    bytes_per_texel: u32,
    data: &[u8],
) {
    if width == 0 || height == 0 {
        return;
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * bytes_per_texel),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
}

fn begin_pass<'a>(
    commands: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    clear: Option<wgpu::Color>,
) -> wgpu::RenderPass<'a> {
    commands.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("farm-gpu pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: clear.map_or(wgpu::LoadOp::Load, wgpu::LoadOp::Clear),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}
