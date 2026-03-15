use alloc::borrow::Cow;
use alloc::vec::Vec;

use shared::{LaserInstance, LaserUniforms, PointCloudUniforms};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Adapter, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState, Buffer,
    BufferBindingType, BufferDescriptor, BufferUsages, Color, ColorTargetState, ColorWrites,
    CommandEncoderDescriptor, Device, DeviceDescriptor, Features, FragmentState, FrontFace,
    Instance, Limits, LoadOp, MemoryHints, Operations, PipelineLayoutDescriptor, PolygonMode,
    PowerPreference, PrimitiveState, PrimitiveTopology, Queue, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp, Surface, SurfaceConfiguration,
    TextureFormat, TextureViewDescriptor, VertexAttribute, VertexBufferLayout, VertexFormat,
    VertexState, VertexStepMode,
};
use winit::{dpi::PhysicalSize, event_loop::EventLoopProxy, window::Window};

#[cfg(target_arch = "wasm32")]
pub type Rc<T> = alloc::rc::Rc<T>;

#[cfg(not(target_arch = "wasm32"))]
pub type Rc<T> = alloc::sync::Arc<T>;

pub enum RenderLevel {
    High,
    Medium,
    Low,
}

#[derive(Debug)]
pub struct PointCloudActor {
    pub uniform_buffer: Buffer,
    pub bind_group: BindGroup,
    pub position_buffer: Option<Buffer>,
    pub data_buffer: Option<Buffer>,
    pub point_count: u32,
}

impl PointCloudActor {
    fn new(device: &Device, layout: &BindGroupLayout) -> Self {
        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Point Cloud Uniform Buffer"),
            size: core::mem::size_of::<PointCloudUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Point Cloud Bind Group"),
            layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        Self {
            uniform_buffer,
            bind_group,
            position_buffer: None,
            data_buffer: None,
            point_count: 0,
        }
    }

    fn load_point_cloud(&mut self, device: &Device, positions: &[f32], point_data: &[f32]) {
        let point_count = positions.len() / 3;
        let vertex_count = point_count * 6;

        let mut position_data = Vec::new();
        for i in (0..positions.len()).step_by(3) {
            if i + 2 < positions.len() {
                let x = positions[i];
                let y = positions[i + 1];
                let z = positions[i + 2];
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
                for _ in 0..6 {
                    data_buffer.extend_from_slice(&[active, size, layer, delay]);
                }
            }
        }

        let position_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Point Cloud Position Buffer"),
            contents: bytemuck::cast_slice(&position_data),
            usage: BufferUsages::VERTEX,
        });

        let data_buffer_gpu = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Point Cloud Data Buffer"),
            contents: bytemuck::cast_slice(&data_buffer),
            usage: BufferUsages::VERTEX,
        });

        self.position_buffer = Some(position_buffer);
        self.data_buffer = Some(data_buffer_gpu);
        self.point_count = vertex_count as u32;
    }

    fn update_uniforms(&self, queue: &Queue, uniforms: &PointCloudUniforms) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    fn has_data(&self) -> bool {
        self.position_buffer.is_some() && self.data_buffer.is_some() && self.point_count > 0
    }
}

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
            required_limits: Limits::downlevel_defaults(),
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
        create_point_cloud_pipeline(&device, surface_config.format);

    let current_actor = PointCloudActor::new(&device, &point_bind_group_layout);
    let backup_actor = PointCloudActor::new(&device, &point_bind_group_layout);

    let (laser_pipeline, laser_bind_group_layout) =
        create_laser_pipeline(&device, surface_config.format);

    // Create laser uniform buffer
    let laser_uniform_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Laser Uniform Buffer"),
        size: core::mem::size_of::<LaserUniforms>() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Create laser storage buffer
    const MAX_LASER_INSTANCES: usize = 2000;
    let laser_storage_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Laser Storage Buffer"),
        size: (core::mem::size_of::<LaserInstance>() * MAX_LASER_INSTANCES) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Create laser bind group
    let laser_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Laser Bind Group"),
        layout: &laser_bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: laser_uniform_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: laser_storage_buffer.as_entire_binding(),
            },
        ],
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
        current_actor,
        backup_actor,
        laser_pipeline,
        laser_uniform_buffer,
        laser_storage_buffer,
        laser_bind_group,
        laser_instance_count: 0,
    };

    let _ = proxy.send_event(gfx);
}

fn create_point_cloud_pipeline(
    device: &Device,
    swap_chain_format: TextureFormat,
) -> (RenderPipeline, BindGroupLayout) {
    // Load SPIR-V shaders
    let data: &[u8] = include_bytes!(env!("point_cloud.spv"));
    let spirv = Cow::Owned(wgpu::util::make_spirv_raw(data).into_owned());
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
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
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
                VertexBufferLayout {
                    array_stride: 12, // 3 * f32
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[VertexAttribute {
                        format: VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    }],
                },
                // Point data buffer (location 1): active, size, layer, delay
                VertexBufferLayout {
                    array_stride: 16, // 4 * f32
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[VertexAttribute {
                        format: VertexFormat::Float32x4,
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
            targets: &[Some(ColorTargetState {
                format: swap_chain_format,
                blend: Some(BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });

    (pipeline, point_bind_group_layout)
}

fn create_laser_pipeline(
    device: &Device,
    swap_chain_format: TextureFormat,
) -> (RenderPipeline, BindGroupLayout) {
    // Load SPIR-V shader
    let data: &[u8] = include_bytes!(env!("laser.spv"));
    let spirv = Cow::Owned(wgpu::util::make_spirv_raw(data).into_owned());
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("Laser Shader"),
        source: ShaderSource::SpirV(spirv),
    });

    // Create bind group layout for laser uniforms
    let laser_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("Laser Bind Group Layout"),
        entries: &[
            // Uniform buffer
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX_FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // Storage buffer (read-only)
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("Laser Pipeline Layout"),
        bind_group_layouts: &[&laser_bind_group_layout],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("Laser Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("laser_vs"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("laser_fs"),
            targets: &[Some(ColorTargetState {
                format: swap_chain_format,
                blend: Some(BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: PrimitiveState {
            topology: PrimitiveTopology::LineList,
            strip_index_format: None,
            front_face: FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });

    (pipeline, laser_bind_group_layout)
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

    // Point cloud rendering
    point_cloud_pipeline: RenderPipeline,
    current_actor: PointCloudActor,
    backup_actor: PointCloudActor,

    // Laser rendering
    laser_pipeline: RenderPipeline,
    laser_uniform_buffer: Buffer,
    laser_storage_buffer: Buffer,
    laser_bind_group: BindGroup,
    laser_instance_count: u32,
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

    pub fn load_current_point_cloud(&mut self, positions: &[f32], point_data: &[f32]) {
        self.current_actor
            .load_point_cloud(&self.device, positions, point_data);
    }

    pub fn load_backup_point_cloud(&mut self, positions: &[f32], point_data: &[f32]) {
        self.backup_actor
            .load_point_cloud(&self.device, positions, point_data);
    }

    pub fn clear_backup_point_cloud(&mut self) {
        self.backup_actor.position_buffer = None;
        self.backup_actor.data_buffer = None;
        self.backup_actor.point_count = 0;
    }

    pub fn swap_actors(&mut self) {
        core::mem::swap(&mut self.current_actor, &mut self.backup_actor);
    }

    pub fn update_current_uniforms(&self, uniforms: &PointCloudUniforms) {
        self.current_actor.update_uniforms(&self.queue, uniforms);
    }

    pub fn update_backup_uniforms(&self, uniforms: &PointCloudUniforms) {
        self.backup_actor.update_uniforms(&self.queue, uniforms);
    }

    pub fn update_laser_uniforms(&self, uniforms: &LaserUniforms) {
        self.queue
            .write_buffer(&self.laser_uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    pub fn update_laser_instances(&mut self, instances: &[LaserInstance]) {
        if !instances.is_empty() {
            self.queue.write_buffer(
                &self.laser_storage_buffer,
                0,
                bytemuck::cast_slice(instances),
            );
        }
        self.laser_instance_count = instances.len() as u32;
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
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if self.backup_actor.has_data()
                && let (Some(pos_buf), Some(data_buf)) = (
                    &self.backup_actor.position_buffer,
                    &self.backup_actor.data_buffer,
                )
            {
                r_pass.set_pipeline(&self.point_cloud_pipeline);
                r_pass.set_bind_group(0, &self.backup_actor.bind_group, &[]);
                r_pass.set_vertex_buffer(0, pos_buf.slice(..));
                r_pass.set_vertex_buffer(1, data_buf.slice(..));
                r_pass.draw(0..self.backup_actor.point_count, 0..1);
            }

            if self.current_actor.has_data()
                && let (Some(pos_buf), Some(data_buf)) = (
                    &self.current_actor.position_buffer,
                    &self.current_actor.data_buffer,
                )
            {
                r_pass.set_pipeline(&self.point_cloud_pipeline);
                r_pass.set_bind_group(0, &self.current_actor.bind_group, &[]);
                r_pass.set_vertex_buffer(0, pos_buf.slice(..));
                r_pass.set_vertex_buffer(1, data_buf.slice(..));
                r_pass.draw(0..self.current_actor.point_count, 0..1);
            }

            // Draw laser rays
            if self.laser_instance_count > 0 {
                r_pass.set_pipeline(&self.laser_pipeline);
                r_pass.set_bind_group(0, &self.laser_bind_group, &[]);
                r_pass.draw(0..2, 0..self.laser_instance_count);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}
