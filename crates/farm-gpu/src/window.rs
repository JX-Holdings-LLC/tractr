//! Presenting into a window: the surface, its configuration and the present pass.

use crate::encode::{self, Instance, INSTANCE_SIZE};
use crate::{begin_pass, describe, new_instance, pipeline, request_adapter, GpuError, GpuRenderer};
use farm_render::Composition;

/// Frames in a row without a surface texture (timeouts, a surface lost again and again) after
/// which presenting fails, so the host falls back to the CPU renderer.
const MAX_STALLS: u32 = 120;

/// A [`GpuRenderer`] presenting into a window's surface.
pub struct WindowRenderer {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    /// The format the present pass writes (the surface's, without sRGB).
    view_format: wgpu::TextureFormat,
    /// Frames in a row the surface timed out or had to be configured again.
    stalls: u32,
    renderer: GpuRenderer,
}

impl std::fmt::Debug for WindowRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowRenderer")
            .field("renderer", &self.renderer)
            .field("format", &self.config.format)
            .finish_non_exhaustive()
    }
}

/// What presenting a frame did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presented {
    /// The frame is on screen.
    Shown,
    /// The surface was not ready (minimized, occluded, resized, timed out); the next frame tries
    /// again.
    Skipped,
}

impl WindowRenderer {
    /// A renderer for `window` (for example an `Arc<winit::window::Window>`) of `width`×`height`
    /// pixels. `allow_software`: accept a software adapter (llvmpipe, WARP).
    pub fn new(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        allow_software: bool,
    ) -> Result<Self, GpuError> {
        let instance = new_instance();
        let surface = instance.create_surface(window).map_err(|error| GpuError::Surface(error.to_string()))?;
        let adapter = request_adapter(&instance, Some(&surface), allow_software)?;
        let mut renderer = GpuRenderer::new(&adapter)?;
        let capabilities = surface.get_capabilities(&adapter);
        let Some(format) = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| !format.is_srgb())
            .or_else(|| capabilities.formats.first().copied())
        else {
            return Err(GpuError::Surface(format!("{} cannot present to this window", describe(&renderer.info))));
        };
        // The frame holds the CPU renderer's values; an sRGB surface is written through a plain view
        // so they reach the screen unconverted, as the CPU path's do.
        let view_format = format.remove_srgb_suffix();
        let alpha_mode = if capabilities.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            capabilities.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto)
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.clamp(1, renderer.max_side),
            height: height.clamp(1, renderer.max_side),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: if view_format == format { Vec::new() } else { vec![view_format] },
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&renderer.device, &config);
        renderer.check()?;
        let present = pipeline(&renderer.device, &renderer.layout, &renderer.shader, view_format, None);
        renderer.present = Some(present);
        renderer.check()?;
        Ok(Self { surface, config, view_format, stalls: 0, renderer })
    }

    /// The adapter, for logs.
    pub fn summary(&self) -> String {
        self.renderer.summary()
    }

    /// Whether the adapter is a software rasterizer.
    pub fn is_software(&self) -> bool {
        self.renderer.is_software()
    }

    fn configure(&mut self, width: u32, height: u32) {
        let (width, height) = (width.clamp(1, self.renderer.max_side), height.clamp(1, self.renderer.max_side));
        if (self.config.width, self.config.height) != (width, height) {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.renderer.device, &self.config);
        }
    }

    /// Draws `composition` and presents it.
    pub fn present(&mut self, composition: &Composition<'_>) -> Result<Presented, GpuError> {
        self.present_with(composition.width, composition.height, |renderer, view| {
            renderer.render_with(composition, Some(view))
        })
    }

    /// Presents CPU pixels (premultiplied RGBA, `width`×`height`): the error screen, or a frame
    /// the CPU renderer drew instead.
    pub fn present_pixels(&mut self, width: u32, height: u32, pixels: &[u8]) -> Result<Presented, GpuError> {
        self.renderer.upload_frame(width, height, pixels)?;
        self.present_with(width, height, |renderer, view| renderer.copy_frame(width, height, view))
    }

    fn present_with(
        &mut self,
        width: u32,
        height: u32,
        draw: impl FnOnce(&mut GpuRenderer, &wgpu::TextureView) -> Result<(), GpuError>,
    ) -> Result<Presented, GpuError> {
        self.renderer.check()?;
        self.configure(width, height);
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                // Present this one; the next frame configures the surface again.
                self.config.width = 0;
                texture
            }
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(Presented::Skipped),
            stalled @ (wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Outdated
            | wgpu::CurrentSurfaceTexture::Lost) => {
                if !matches!(stalled, wgpu::CurrentSurfaceTexture::Timeout) {
                    self.config.width = 0;
                }
                // A surface that keeps failing would leave the window black: give up on it.
                self.stalls += 1;
                if self.stalls >= MAX_STALLS {
                    return Err(GpuError::Surface(format!("no frame could be presented for {MAX_STALLS} frames")));
                }
                return Ok(Presented::Skipped);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(GpuError::Surface("the surface texture could not be acquired".to_owned()));
            }
        };
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor { format: Some(self.view_format), ..Default::default() });
        self.stalls = 0;
        let result = draw(&mut self.renderer, &view);
        self.renderer.queue.present(texture);
        result?;
        self.renderer.check()?;
        Ok(Presented::Shown)
    }
}

impl GpuRenderer {
    /// Copies the frame texture into a window's surface view.
    fn copy_frame(&mut self, width: u32, height: u32, view: &wgpu::TextureView) -> Result<(), GpuError> {
        let (Some(present), Some(frame)) = (&self.present, &self.frame) else { return Ok(()) };
        let (width, height) = (width.max(1), height.max(1));
        let instance = Instance {
            rect: [0.0, 0.0, width as f32, height as f32],
            data: [encode::COPY, 0, 0, 0],
            ..Instance::default()
        };
        let mut bytes = Vec::with_capacity(INSTANCE_SIZE);
        instance.write(&mut bytes);
        self.queue.write_buffer(&self.instance_buffer, 0, &bytes);
        self.queue.write_buffer(&self.frame_uniform, 0, &crate::size_bytes(width, height));
        let group = self.targets_group(&self.frame_uniform);
        let mut commands =
            self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("farm-gpu present") });
        {
            let mut pass = begin_pass(&mut commands, view, None);
            pass.set_pipeline(present);
            pass.set_bind_group(0, &group, &[]);
            pass.set_bind_group(1, &frame.image, &[]);
            pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            pass.draw(0..6, 0..1);
        }
        self.queue.submit([commands.finish()]);
        self.check()
    }
}
