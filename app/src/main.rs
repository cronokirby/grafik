#![forbid(unsafe_code)]

use std::sync::Arc;

use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GPUSphere {
    center_radius: [f32; 4],
}

/// Size of the image computed by the compute shader.
const WIDTH: u32 = 600;
const HEIGHT: u32 = 600;

/// Presentation settings for drawing the computed image into the window.
const PRESENTATION: PresentationConfig = PresentationConfig {
    letterbox_color: wgpu::Color::BLACK,
};

/// Texture format shared by the compute output, the blit sampler, and PNG export.
const IMAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

struct PresentationConfig {
    /// Color used to clear the surface area outside the aspect-preserving image.
    letterbox_color: wgpu::Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Viewport {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

fn letterboxed_viewport(
    surface_width: u32,
    surface_height: u32,
    image_width: u32,
    image_height: u32,
) -> Viewport {
    let surface_width = surface_width.max(1) as f32;
    let surface_height = surface_height.max(1) as f32;
    let image_aspect = image_width.max(1) as f32 / image_height.max(1) as f32;
    let surface_aspect = surface_width / surface_height;

    let (width, height) = if surface_aspect > image_aspect {
        (surface_height * image_aspect, surface_height)
    } else {
        (surface_width, surface_width / image_aspect)
    };

    Viewport {
        x: (surface_width - width) * 0.5,
        y: (surface_height - height) * 0.5,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_viewport_close(actual: Viewport, expected: Viewport) {
        const EPSILON: f32 = 0.01;
        assert!((actual.x - expected.x).abs() < EPSILON, "{actual:?}");
        assert!((actual.y - expected.y).abs() < EPSILON, "{actual:?}");
        assert!(
            (actual.width - expected.width).abs() < EPSILON,
            "{actual:?}"
        );
        assert!(
            (actual.height - expected.height).abs() < EPSILON,
            "{actual:?}"
        );
    }

    #[test]
    fn viewport_fills_matching_aspect() {
        assert_viewport_close(
            letterboxed_viewport(800, 600, 400, 300),
            Viewport {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            },
        );
    }

    #[test]
    fn viewport_letterboxes_wide_surface() {
        assert_viewport_close(
            letterboxed_viewport(1200, 600, 600, 600),
            Viewport {
                x: 300.0,
                y: 0.0,
                width: 600.0,
                height: 600.0,
            },
        );
    }

    #[test]
    fn viewport_letterboxes_tall_surface() {
        assert_viewport_close(
            letterboxed_viewport(600, 1200, 600, 600),
            Viewport {
                x: 0.0,
                y: 300.0,
                width: 600.0,
                height: 600.0,
            },
        );
    }

    #[test]
    fn viewport_handles_zero_dimensions() {
        assert_viewport_close(
            letterboxed_viewport(0, 0, 0, 0),
            Viewport {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
        );
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("event loop error");
}

/// Encode a linear [0, 1] value into sRGB. Mirrors `linear_to_srgb` in blit.wgsl
/// so the saved PNG matches what is shown on screen.
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Encode one linear 8-bit channel into an sRGB 8-bit channel.
fn linear_u8_to_srgb_u8(linear: u8) -> u8 {
    let v = linear_to_srgb(linear as f32 / 255.0);
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

#[derive(Default)]
struct App {
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Grafik (press 's' to save a PNG)")
                    .with_inner_size(size)
                    .with_min_inner_size(size),
            )
            .expect("failed to create window");

        let state = pollster::block_on(State::new(Arc::new(window)));
        // Run the compute pass once to generate the image.
        state.compute();
        state.window.request_redraw();
        self.state = Some(state);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Keep redrawing so the first visible frame is presented once the window
        // is no longer occluded (e.g. it starts behind the launching terminal).
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => match code {
                KeyCode::Escape => event_loop.exit(),
                KeyCode::KeyS => state.save_png_with_picker(),
                _ => {}
            },
            WindowEvent::Resized(new_size) => {
                state.resize(new_size.width, new_size.height);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => state.render(),
            _ => {}
        }
    }
}

struct State {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    image_texture: wgpu::Texture,

    compute_pipeline: wgpu::ComputePipeline,
    compute_bind_group: wgpu::BindGroup,

    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group: wgpu::BindGroup,
}

impl State {
    async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .expect("failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("no suitable GPU adapter found");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("device"),
                ..Default::default()
            })
            .await
            .expect("failed to create device");

        // Configure the surface to match the window. Prefer a non-sRGB format so
        // what is displayed matches the bytes we write to the PNG exactly.
        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            // Fifo (vsync) is always supported and throttles our continuous redraw.
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // The image the compute shader writes into and the blit pass reads from.
        let image_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image texture"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: IMAGE_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let image_view = image_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let spheres = [
            GPUSphere {
                center_radius: [-0.8, 0.0, -4.0, 1.2],
            },
            GPUSphere {
                center_radius: [1.5, 0.5, -6.0, 1.0],
            },
            GPUSphere {
                center_radius: [0.0, -1.25, -5.0, 0.75],
            },
        ];
        let sphere_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sphere buffer"),
            contents: bytemuck::cast_slice(&spheres),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // --- Compute pipeline ---
        let compute_shader = device.create_shader_module(wgpu::include_wgsl!("compute.wgsl"));
        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("compute bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: IMAGE_FORMAT,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compute bind group"),
            layout: &compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&image_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: sphere_buffer.as_entire_binding(),
                },
            ],
        });
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("compute pipeline layout"),
                bind_group_layouts: &[Some(&compute_bind_group_layout)],
                immediate_size: 0,
            });
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("compute pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // --- Blit (render) pipeline ---
        let blit_shader = device.create_shader_module(wgpu::include_wgsl!("blit.wgsl"));
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blit sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let blit_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("blit bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let blit_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit bind group"),
            layout: &blit_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&image_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit pipeline layout"),
            bind_group_layouts: &[Some(&blit_bind_group_layout)],
            immediate_size: 0,
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit pipeline"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            window,
            surface,
            device,
            queue,
            config,
            image_texture,
            compute_pipeline,
            compute_bind_group,
            blit_pipeline,
            blit_bind_group,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Run the compute shader to fill the image texture.
    fn compute(&self) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("compute encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, &self.compute_bind_group, &[]);
            let wg_x = WIDTH.div_ceil(8);
            let wg_y = HEIGHT.div_ceil(8);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }
        self.queue.submit([encoder.finish()]);
    }

    /// Draw the computed image into the window.
    fn render(&mut self) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                // Reconfigure and try again on the next redraw.
                let size = self.window.inner_size();
                self.resize(size.width, size.height);
                return;
            }
            // Timeout / Occluded / Validation: skip this frame.
            _ => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(PRESENTATION.letterbox_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let viewport =
                letterboxed_viewport(self.config.width, self.config.height, WIDTH, HEIGHT);
            pass.set_viewport(
                viewport.x,
                viewport.y,
                viewport.width,
                viewport.height,
                0.0,
                1.0,
            );
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, &self.blit_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        frame.present();
    }

    /// Copy the image texture back to the CPU and write it out as a PNG.
    /// Prompt for a destination with a native file dialog, then save a PNG there.
    fn save_png_with_picker(&self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save image as PNG")
            .add_filter("PNG image", &["png"])
            .set_file_name("grafik.png")
            .save_file()
        else {
            // Dialog cancelled.
            return;
        };
        self.save_png(&path);
    }

    fn save_png(&self, path: &std::path::Path) {
        // Rows in a copy must be aligned to COPY_BYTES_PER_ROW_ALIGNMENT (256).
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = WIDTH * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

        let buffer_size = (padded_bytes_per_row * HEIGHT) as wgpu::BufferAddress;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("png readback buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.image_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(HEIGHT),
                },
            },
            wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);

        // Map the buffer and block until the copy is done.
        let (tx, rx) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("failed to poll device");
        match rx.recv() {
            Ok(Ok(())) => {}
            other => {
                eprintln!("failed to map readback buffer: {other:?}");
                return;
            }
        }

        // Strip the row padding and collect tightly packed RGBA bytes, encoding
        // the linear image into sRGB (matching the blit shader). Alpha is linear
        // and passes through unchanged.
        let mut pixels = Vec::with_capacity((unpadded_bytes_per_row * HEIGHT) as usize);
        {
            let data = buffer.slice(..).get_mapped_range();
            for row in data.chunks_exact(padded_bytes_per_row as usize) {
                for px in row[..unpadded_bytes_per_row as usize].chunks_exact(4) {
                    pixels.push(linear_u8_to_srgb_u8(px[0]));
                    pixels.push(linear_u8_to_srgb_u8(px[1]));
                    pixels.push(linear_u8_to_srgb_u8(px[2]));
                    pixels.push(px[3]);
                }
            }
        }
        buffer.unmap();

        match image::RgbaImage::from_raw(WIDTH, HEIGHT, pixels) {
            Some(img) => match img.save_with_format(path, image::ImageFormat::Png) {
                Ok(()) => println!("saved {}", path.display()),
                Err(err) => eprintln!("failed to save {}: {err}", path.display()),
            },
            None => eprintln!("failed to build image from GPU data"),
        }
    }
}
