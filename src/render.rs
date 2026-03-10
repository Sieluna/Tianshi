use alloc::borrow::Cow;
use alloc::vec::Vec;

use shared::PointCloudUniforms;
use wgpu::util::DeviceExt;
use wgpu::{
    Adapter, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, Buffer, BufferBindingType, BufferUsages,
    Color, CommandEncoderDescriptor, Device, DeviceDescriptor, Features, FragmentState, Instance,
    Limits, LoadOp, MemoryHints, Operations, PowerPreference, PrimitiveTopology, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp, Surface,
    SurfaceConfiguration, TextureFormat, TextureViewDescriptor, VertexState,
};
use winit::{dpi::PhysicalSize, event_loop::EventLoopProxy, window::Window};

#[cfg(target_arch = "wasm32")]
pub type Rc<T> = alloc::rc::Rc<T>;

#[cfg(not(target_arch = "wasm32"))]
pub type Rc<T> = alloc::sync::Arc<T>;

pub async fn create_graphics(window: Rc<Window>, proxy: EventLoopProxy<Graphics>) {
    let instance = Instance::default();
    let surface = instance.create_surface(Rc::clone(&window)).unwrap();
    let adapter = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .await
        .expect("Failed at adapter creation.");

    let (device, queue) = adapter
        .request_device(&DeviceDescriptor {
            label: None,
            required_features: Features::empty(),
            required_limits: Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            experimental_features: Default::default(),
            memory_hints: MemoryHints::Performance,
            trace: Default::default(),
        })
        .await
        .expect("Failed to get device.");

    let size = window.inner_size();
    let width = size.width.max(1);
    let height = size.height.max(1);
    let surface_config = surface.get_default_config(&adapter, width, height).unwrap();

    surface.configure(&device, &surface_config);

    let (point_cloud_pipeline, point_bind_group_layout) =
        create_pipeline(&device, surface_config.format);

    // Create uniform buffer for point cloud
    let point_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Point Cloud Uniform Buffer"),
        size: core::mem::size_of::<PointCloudUniforms>() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Create bind group for point cloud
    let point_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Point Cloud Bind Group"),
        layout: &point_bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: point_uniform_buffer.as_entire_binding(),
        }],
    });

    let gfx = Graphics {
        window: window.clone(),
        instance,
        surface,
        surface_config,
        adapter,
        device,
        queue,
        point_cloud_pipeline,
        point_uniform_buffer,
        point_bind_group,
        point_cloud_position_buffer: None,
        point_cloud_data_buffer: None,
        point_count: 0,
    };

    let _ = proxy.send_event(gfx);
}

fn create_pipeline(
    device: &Device,
    swap_chain_format: TextureFormat,
) -> (RenderPipeline, BindGroupLayout) {
    // Load SPIR-V shaders
    let data: &[u8] = include_bytes!(env!("point_cloud.spv"));
    let spirv = Cow::Owned(wgpu::util::make_spirv_raw(&data).into_owned());
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("Point Cloud Shader"),
        source: ShaderSource::SpirV(spirv),
    });

    // Create bind group layout for point cloud uniforms
    let point_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("Point Cloud Bind Group Layout"),
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Point Cloud Pipeline Layout"),
        bind_group_layouts: &[&point_bind_group_layout],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("Point Cloud Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("point_cloud_vs"),
            buffers: &[
                // Position buffer (location 0)
                wgpu::VertexBufferLayout {
                    array_stride: 12, // 3 * f32
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    }],
                },
                // Point data buffer (location 1): active, size, layer, delay
                wgpu::VertexBufferLayout {
                    array_stride: 16, // 4 * f32
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 0,
                        shader_location: 1,
                    }],
                },
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("point_cloud_fs"),
            targets: &[Some(swap_chain_format.into())],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });

    (pipeline, point_bind_group_layout)
}

#[derive(Debug)]
pub struct Graphics {
    window: Rc<Window>,
    instance: Instance,
    surface: Surface<'static>,
    surface_config: SurfaceConfiguration,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    point_cloud_pipeline: RenderPipeline,
    point_uniform_buffer: Buffer,
    point_bind_group: BindGroup,
    point_cloud_position_buffer: Option<Buffer>,
    point_cloud_data_buffer: Option<Buffer>,
    point_count: u32,
}

impl Graphics {
    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn surface_config(&self) -> &SurfaceConfiguration {
        &self.surface_config
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.surface_config.width = new_size.width.max(1);
        self.surface_config.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub fn load_point_cloud(&mut self, positions: &[f32], point_data: &[f32]) {
        // positions: flattened [x, y, z, x, y, z, ...] array
        // point_data: flattened [active, size, layer, delay, active, size, layer, delay, ...] array
        let point_count = positions.len() / 3;
        let vertex_count = point_count * 6;

        let mut position_data = Vec::new();
        for i in (0..positions.len()).step_by(3) {
            if i + 2 < positions.len() {
                let x = positions[i];
                let y = positions[i + 1];
                let z = positions[i + 2];
                // Each point creates 6 vertices (for quad)
                for _ in 0..6 {
                    position_data.extend_from_slice(&[x, y, z]);
                }
            }
        }

        let mut data_buffer = Vec::new();
        for i in (0..point_data.len()).step_by(4) {
            if i + 3 < point_data.len() {
                let active = point_data[i];
                let size = point_data[i + 1];
                let layer = point_data[i + 2];
                let delay = point_data[i + 3];
                // Each point's data is repeated for 6 vertices
                for _ in 0..6 {
                    data_buffer.extend_from_slice(&[active, size, layer, delay]);
                }
            }
        }

        // Create position buffer
        let position_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Point Cloud Position Buffer"),
                contents: bytemuck::cast_slice(&position_data),
                usage: wgpu::BufferUsages::VERTEX,
            });

        // Create point data buffer
        let data_buffer_gpu = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Point Cloud Data Buffer"),
                contents: bytemuck::cast_slice(&data_buffer),
                usage: wgpu::BufferUsages::VERTEX,
            });

        self.point_cloud_position_buffer = Some(position_buffer);
        self.point_cloud_data_buffer = Some(data_buffer_gpu);
        self.point_count = vertex_count as u32;
    }

    pub fn update_point_uniforms(&self, uniforms: &PointCloudUniforms) {
        self.queue
            .write_buffer(&self.point_uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    pub fn draw(&mut self) {
        let frame = self
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture.");

        let view = frame.texture.create_view(&TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });

        {
            let mut r_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // Draw point cloud
            if let Some(ref pos_buffer) = self.point_cloud_position_buffer {
                if let Some(ref data_buffer) = self.point_cloud_data_buffer {
                    r_pass.set_pipeline(&self.point_cloud_pipeline);
                    r_pass.set_bind_group(0, &self.point_bind_group, &[]);
                    r_pass.set_vertex_buffer(0, pos_buffer.slice(..));
                    r_pass.set_vertex_buffer(1, data_buffer.slice(..));
                    r_pass.draw(0..self.point_count, 0..1);
                }
            }
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}
