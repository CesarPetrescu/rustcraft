use std::collections::{HashMap, HashSet};
use std::mem;
use std::sync::Arc;

use anyhow::Context;
use cgmath::{InnerSpace, Matrix, SquareMatrix};
use cgmath::{Matrix4, Quaternion, Rad, Rotation, Rotation3, Vector3, Vector4};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::block::BlockType;
use crate::camera::{Camera, Projection};
use crate::electric::{ComponentTelemetry, ElectricalComponent};
use crate::chunk::{CHUNK_HEIGHT, CHUNK_SIZE};
use crate::mesh::{self, MeshData, Vertex as BlockVertex};
use crate::texture::TextureAtlas;
use crate::world::{AtmosphereSample, ChunkPos, World};

const SHADER_SOURCE: &str = include_str!("shader.wgsl");
const SKY_SHADER_SOURCE: &str = include_str!("sky.wgsl");
const HIGHLIGHT_SHADER_SOURCE: &str = include_str!("highlight.wgsl");
const UI_SHADER_SOURCE: &str = include_str!("ui_shader.wgsl");

const INITIAL_HIGHLIGHT_CAPACITY: usize = 128;
const INITIAL_POWER_CAPACITY: usize = 512;
const INITIAL_HAND_VERTEX_CAPACITY: usize = 128;
const INITIAL_HAND_INDEX_CAPACITY: usize = 192;
const INITIAL_ENTITY_VERTEX_CAPACITY: usize = 2048;
const INITIAL_ENTITY_INDEX_CAPACITY: usize = 3072;
const INITIAL_UI_VERTEX_CAPACITY: usize = 512;
const INITIAL_UI_INDEX_CAPACITY: usize = 1024;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    fn identity() -> Self {
        Self {
            view_proj: Matrix4::<f32>::identity().into(),
        }
    }

    fn from_matrix(matrix: Matrix4<f32>) -> Self {
        Self {
            view_proj: matrix.into(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct EnvironmentUniform {
    sky_zenith: [f32; 4],
    sky_horizon: [f32; 4],
    fog_color: [f32; 4],
    camera_position: [f32; 4],
    fog_params: [f32; 4],
    time_params: [f32; 4],
    screen_params: [f32; 4],
}

impl EnvironmentUniform {
    fn new() -> Self {
        Self {
            sky_zenith: [0.0; 4],
            sky_horizon: [0.0; 4],
            fog_color: [0.0; 4],
            camera_position: [0.0; 4],
            fog_params: [0.0; 4],
            time_params: [0.0; 4],
            screen_params: [0.0; 4],
        }
    }

    fn from_sample(
        sample: &AtmosphereSample,
        camera_pos: [f32; 3],
        size: PhysicalSize<u32>,
    ) -> Self {
        let mut uniform = Self::new();
        uniform.sky_zenith = [
            sample.sky_zenith[0],
            sample.sky_zenith[1],
            sample.sky_zenith[2],
            1.0,
        ];
        uniform.sky_horizon = [
            sample.sky_horizon[0],
            sample.sky_horizon[1],
            sample.sky_horizon[2],
            1.0,
        ];
        uniform.fog_color = [
            sample.fog_color[0],
            sample.fog_color[1],
            sample.fog_color[2],
            1.0,
        ];
        uniform.camera_position = [camera_pos[0], camera_pos[1], camera_pos[2], 1.0];
        uniform.fog_params = [
            sample.fog_density,
            sample.ambient_strength.max(0.05),
            sample.vignette_strength,
            0.035,
        ];
        uniform.time_params = [
            sample.daylight,
            sample.sun_elevation,
            sample.twilight,
            sample.time_of_day,
        ];

        let width = size.width.max(1) as f32;
        let height = size.height.max(1) as f32;
        uniform.screen_params = [width, height, 1.0 / width, 1.0 / height];
        uniform
    }
}

struct DepthTexture {
    view: wgpu::TextureView,
}

impl DepthTexture {
    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    fn create(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth_texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self { view }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct HighlightVertex {
    position: [f32; 3],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
    pub uv: [f32; 2],
    pub mode: f32,
}

struct ChunkGpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
}

#[derive(Clone, Copy)]
struct Plane {
    normal: Vector3<f32>,
    d: f32,
}

impl Plane {
    fn from_vec4(v: Vector4<f32>) -> Self {
        let normal = Vector3::new(v.x, v.y, v.z);
        let length = normal.magnitude();
        if length > 0.0 {
            let inv = 1.0 / length;
            Self {
                normal: normal * inv,
                d: v.w * inv,
            }
        } else {
            Self { normal, d: v.w }
        }
    }

    fn distance_to_point(&self, point: [f32; 3]) -> f32 {
        self.normal.x * point[0] + self.normal.y * point[1] + self.normal.z * point[2] + self.d
    }
}

struct Frustum {
    planes: [Plane; 6],
}

impl Frustum {
    fn from_matrix(matrix: Matrix4<f32>) -> Self {
        let m = matrix.transpose();
        let rows = [m.x, m.y, m.z, m.w];
        let planes = [
            Plane::from_vec4(rows[3] + rows[0]), // Left
            Plane::from_vec4(rows[3] - rows[0]), // Right
            Plane::from_vec4(rows[3] + rows[1]), // Bottom
            Plane::from_vec4(rows[3] - rows[1]), // Top
            Plane::from_vec4(rows[3] + rows[2]), // Near
            Plane::from_vec4(rows[3] - rows[2]), // Far
        ];
        Self { planes }
    }

    fn intersects_aabb(&self, min: [f32; 3], max: [f32; 3]) -> bool {
        for plane in &self.planes {
            let mut positive = [0.0; 3];
            positive[0] = if plane.normal.x >= 0.0 { max[0] } else { min[0] };
            positive[1] = if plane.normal.y >= 0.0 { max[1] } else { min[1] };
            positive[2] = if plane.normal.z >= 0.0 { max[2] } else { min[2] };
            if plane.distance_to_point(positive) < 0.0 {
                return false;
            }
        }
        true
    }
}

pub struct Renderer<'window> {
    size: PhysicalSize<u32>,
    surface: wgpu::Surface<'window>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    config: wgpu::SurfaceConfiguration,
    depth_texture: DepthTexture,
    texture_atlas: TextureAtlas,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    _camera_bind_group_layout: wgpu::BindGroupLayout,
    environment_buffer: wgpu::Buffer,
    environment_bind_group: wgpu::BindGroup,
    _environment_bind_group_layout: wgpu::BindGroupLayout,
    render_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    highlight_pipeline: wgpu::RenderPipeline,
    ui_pipeline: wgpu::RenderPipeline,
    chunk_meshes: HashMap<ChunkPos, ChunkGpuMesh>,
    last_view_proj: Matrix4<f32>,
    highlight_vertex_buffer: wgpu::Buffer,
    highlight_vertex_capacity: usize,
    highlight_vertex_count: u32,
    highlight_vertices: Vec<HighlightVertex>,
    power_vertex_buffer: wgpu::Buffer,
    power_vertex_capacity: usize,
    power_vertex_count: u32,
    power_vertices: Vec<HighlightVertex>,
    hand_vertex_buffer: wgpu::Buffer,
    hand_index_buffer: wgpu::Buffer,
    hand_vertex_capacity: usize,
    hand_index_capacity: usize,
    hand_index_count: u32,
    entity_vertex_buffer: wgpu::Buffer,
    entity_index_buffer: wgpu::Buffer,
    entity_vertex_capacity: usize,
    entity_index_capacity: usize,
    entity_index_count: u32,
    ui_vertex_buffer: wgpu::Buffer,
    ui_index_buffer: wgpu::Buffer,
    ui_vertex_capacity: usize,
    ui_index_capacity: usize,
    ui_index_count: u32,
    ui_vertices: Vec<UiVertex>,
    ui_indices: Vec<u16>,
    clear_color: [f32; 4],
}

impl<'window> Renderer<'window> {
    pub fn new(window: &'window Window) -> anyhow::Result<Self> {
        pollster::block_on(Self::new_async(window))
    }

    async fn new_async(window: &'window Window) -> anyhow::Result<Self> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            dx12_shader_compiler: Default::default(),
            gles_minor_version: wgpu::Gles3MinorVersion::Automatic,
        });

        let surface = instance.create_surface(window)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("failed to find a suitable GPU adapter")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("renderer_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let present_mode = surface_caps
            .present_modes
            .iter()
            .copied()
            .find(|mode| {
                matches!(
                    mode,
                    wgpu::PresentMode::Mailbox | wgpu::PresentMode::AutoVsync
                )
            })
            .unwrap_or(wgpu::PresentMode::Fifo);
        let alpha_mode = surface_caps
            .alpha_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::CompositeAlphaMode::Opaque)
            .unwrap_or(surface_caps.alpha_modes[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(device.as_ref(), &config);

        let texture_atlas = TextureAtlas::new(device.as_ref(), queue.as_ref());

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera_buffer"),
            contents: bytemuck::bytes_of(&CameraUniform::identity()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bind_group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let environment_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("environment_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let environment_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("environment_buffer"),
            contents: bytemuck::bytes_of(&EnvironmentUniform::new()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let environment_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("environment_bind_group"),
            layout: &environment_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: environment_buffer.as_entire_binding(),
            }],
        });

        let world_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("world_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });
        let sky_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky_shader"),
            source: wgpu::ShaderSource::Wgsl(SKY_SHADER_SOURCE.into()),
        });
        let highlight_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("highlight_shader"),
            source: wgpu::ShaderSource::Wgsl(HIGHLIGHT_SHADER_SOURCE.into()),
        });
        let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui_shader"),
            source: wgpu::ShaderSource::Wgsl(UI_SHADER_SOURCE.into()),
        });

        let world_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("world_pipeline_layout"),
                bind_group_layouts: &[
                    &camera_bind_group_layout,
                    &texture_atlas.bind_group_layout,
                    &environment_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("world_pipeline"),
            layout: Some(&world_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &world_shader,
                entry_point: "vs_main",
                buffers: &[block_vertex_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &world_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTexture::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let sky_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky_pipeline_layout"),
            bind_group_layouts: &[&environment_bind_group_layout],
            push_constant_ranges: &[],
        });

        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky_pipeline"),
            layout: Some(&sky_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sky_shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &sky_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTexture::FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let highlight_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("highlight_pipeline_layout"),
                bind_group_layouts: &[&camera_bind_group_layout],
                push_constant_ranges: &[],
            });

        let highlight_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("highlight_pipeline"),
            layout: Some(&highlight_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &highlight_shader,
                entry_point: "vs_main",
                buffers: &[highlight_vertex_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &highlight_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTexture::FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let ui_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui_pipeline_layout"),
            bind_group_layouts: &[&texture_atlas.bind_group_layout],
            push_constant_ranges: &[],
        });

        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui_pipeline"),
            layout: Some(&ui_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &ui_shader,
                entry_point: "vs_main",
                buffers: &[ui_vertex_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &ui_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let highlight_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("highlight_vertex_buffer"),
            size: (INITIAL_HIGHLIGHT_CAPACITY.max(1) * mem::size_of::<HighlightVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let power_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("power_vertex_buffer"),
            size: (INITIAL_POWER_CAPACITY.max(1) * mem::size_of::<HighlightVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let hand_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hand_vertex_buffer"),
            size: (INITIAL_HAND_VERTEX_CAPACITY.max(1) * mem::size_of::<BlockVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hand_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hand_index_buffer"),
            size: (INITIAL_HAND_INDEX_CAPACITY.max(1) * mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let entity_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("entity_vertex_buffer"),
            size: (INITIAL_ENTITY_VERTEX_CAPACITY.max(1) * mem::size_of::<BlockVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let entity_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("entity_index_buffer"),
            size: (INITIAL_ENTITY_INDEX_CAPACITY.max(1) * mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let ui_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui_vertex_buffer"),
            size: (INITIAL_UI_VERTEX_CAPACITY.max(1) * mem::size_of::<UiVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ui_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui_index_buffer"),
            size: (INITIAL_UI_INDEX_CAPACITY.max(1) * mem::size_of::<u16>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let depth_texture = DepthTexture::create(device.as_ref(), &config);

        Ok(Self {
            size,
            surface,
            device,
            queue,
            config,
            depth_texture,
            texture_atlas,
            camera_buffer,
            camera_bind_group,
            _camera_bind_group_layout: camera_bind_group_layout,
            environment_buffer,
            environment_bind_group,
            _environment_bind_group_layout: environment_bind_group_layout,
            render_pipeline,
            sky_pipeline,
            highlight_pipeline,
            ui_pipeline,
            chunk_meshes: HashMap::new(),
            last_view_proj: Matrix4::identity(),
            highlight_vertex_buffer,
            highlight_vertex_capacity: INITIAL_HIGHLIGHT_CAPACITY.max(1),
            highlight_vertex_count: 0,
            highlight_vertices: Vec::new(),
            power_vertex_buffer,
            power_vertex_capacity: INITIAL_POWER_CAPACITY.max(1),
            power_vertex_count: 0,
            power_vertices: Vec::new(),
            hand_vertex_buffer,
            hand_index_buffer,
            hand_vertex_capacity: INITIAL_HAND_VERTEX_CAPACITY.max(1),
            hand_index_capacity: INITIAL_HAND_INDEX_CAPACITY.max(1),
            hand_index_count: 0,
            entity_vertex_buffer,
            entity_index_buffer,
            entity_vertex_capacity: INITIAL_ENTITY_VERTEX_CAPACITY.max(1),
            entity_index_capacity: INITIAL_ENTITY_INDEX_CAPACITY.max(1),
            entity_index_count: 0,
            ui_vertex_buffer,
            ui_index_buffer,
            ui_vertex_capacity: INITIAL_UI_VERTEX_CAPACITY.max(1),
            ui_index_capacity: INITIAL_UI_INDEX_CAPACITY.max(1),
            ui_index_count: 0,
            ui_vertices: Vec::new(),
            ui_indices: Vec::new(),
            clear_color: [0.52, 0.73, 0.86, 1.0],
        })
    }

    pub fn device_handle(&self) -> Arc<wgpu::Device> {
        Arc::clone(&self.device)
    }

    pub fn queue_handle(&self) -> Arc<wgpu::Queue> {
        Arc::clone(&self.queue)
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>, projection: &mut Projection) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }

        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        projection.resize(new_size.width, new_size.height);
        self.surface.configure(self.device.as_ref(), &self.config);
        self.depth_texture = DepthTexture::create(self.device.as_ref(), &self.config);
    }

    fn reconfigure_surface(&mut self) {
        self.surface.configure(self.device.as_ref(), &self.config);
        self.depth_texture = DepthTexture::create(self.device.as_ref(), &self.config);
    }

    pub fn update_camera(&mut self, camera: &Camera, projection: &Projection) {
        let matrix = camera.calc_matrix(projection);
        let uniform = CameraUniform::from_matrix(matrix);
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
        self.last_view_proj = matrix;
    }

    pub fn update_environment(&mut self, atmosphere: &AtmosphereSample, camera_position: [f32; 3]) {
        let uniform = EnvironmentUniform::from_sample(atmosphere, camera_position, self.size);
        self.queue
            .write_buffer(&self.environment_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    pub fn set_clear_color(&mut self, color: [f32; 3]) {
        self.clear_color = [color[0], color[1], color[2], 1.0];
    }

    pub fn rebuild_world_mesh(&mut self, world: &World) {
        self.chunk_meshes.clear();
        for (&pos, chunk) in world.chunks() {
            let mesh = mesh::generate_chunk_mesh(world, pos, chunk);
            self.upload_chunk_mesh(pos, mesh);
        }
    }

    pub fn update_chunks(&mut self, world: &World, dirty_chunks: &HashSet<ChunkPos>) {
        if dirty_chunks.is_empty() {
            return;
        }

        for pos in dirty_chunks {
            if let Some(chunk) = world.chunks().get(pos) {
                let mesh = mesh::generate_chunk_mesh(world, *pos, chunk);
                self.upload_chunk_mesh(*pos, mesh);
            } else {
                self.chunk_meshes.remove(pos);
            }
        }
    }

    fn upload_chunk_mesh(&mut self, pos: ChunkPos, mesh: MeshData) {
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            self.chunk_meshes.remove(&pos);
            return;
        }

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("chunk_vertex_buffer"),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("chunk_index_buffer"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        let base_x = (pos.x * CHUNK_SIZE as i32) as f32;
        let base_z = (pos.z * CHUNK_SIZE as i32) as f32;
        let bounds_min = [base_x - 0.5, -0.5, base_z - 0.5];
        let bounds_max = [
            base_x + CHUNK_SIZE as f32 - 0.5,
            CHUNK_HEIGHT as f32 - 0.5,
            base_z + CHUNK_SIZE as f32 - 0.5,
        ];

        let gpu_mesh = ChunkGpuMesh {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
            bounds_min,
            bounds_max,
        };
        self.chunk_meshes.insert(pos, gpu_mesh);
    }

    fn draw_world_chunks<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        frustum: &Frustum,
    ) {
        for mesh in self.chunk_meshes.values() {
            if mesh.index_count == 0 {
                continue;
            }
            if !frustum.intersects_aabb(mesh.bounds_min, mesh.bounds_max) {
                continue;
            }
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }

    pub fn update_highlight(&mut self, bounds: Option<([f32; 3], [f32; 3])>, breaking_progress: f32) {
        self.highlight_vertices.clear();

        if let Some((min, max)) = bounds {
            let corners = [
                [min[0], min[1], min[2]],
                [max[0], min[1], min[2]],
                [max[0], max[1], min[2]],
                [min[0], max[1], min[2]],
                [min[0], min[1], max[2]],
                [max[0], min[1], max[2]],
                [max[0], max[1], max[2]],
                [min[0], max[1], max[2]],
            ];
            const EDGES: [(usize, usize); 12] = [
                (0, 1),
                (1, 2),
                (2, 3),
                (3, 0),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 4),
                (0, 4),
                (1, 5),
                (2, 6),
                (3, 7),
            ];
            // Color transitions from yellow (no breaking) to red (almost broken)
            let progress = breaking_progress.clamp(0.0, 1.0);
            let red = 1.0;
            let green = 0.95 - progress * 0.5; // 0.95 -> 0.45
            let blue = 0.45 - progress * 0.45; // 0.45 -> 0.0
            let alpha = 0.85 + progress * 0.15; // 0.85 -> 1.0 (more visible as breaking)
            let color = [red, green, blue, alpha];
            for &(a, b) in &EDGES {
                self.highlight_vertices.push(HighlightVertex {
                    position: corners[a],
                    color,
                });
                self.highlight_vertices.push(HighlightVertex {
                    position: corners[b],
                    color,
                });
            }
        }

        self.highlight_vertex_count = self.highlight_vertices.len() as u32;
        self.ensure_highlight_capacity(self.highlight_vertices.len());
        if self.highlight_vertex_count > 0 {
            self.queue.write_buffer(
                &self.highlight_vertex_buffer,
                0,
                bytemuck::cast_slice(&self.highlight_vertices),
            );
        }
    }

    pub fn update_power_overlays(
        &mut self,
        overlays: &[(Vector3<f32>, ElectricalComponent, ComponentTelemetry)],
        animation_time: f32,
    ) {
        self.power_vertices.clear();

        for (index, (pos, component, telemetry)) in overlays.iter().enumerate() {
            let base_color = component_color(*component);
            let current_strength = telemetry.current.abs();
            let voltage_strength = telemetry.voltage_local.abs();
            let intensity = (current_strength * 0.4 + voltage_strength * 0.05).min(3.0);
            let pulse = (animation_time * 4.0 + index as f32 * 0.7).sin() * 0.5 + 0.5;
            let brightness = (0.6 + intensity * 0.25 + pulse * 0.2).clamp(0.0, 2.0);
            let color = [
                (base_color[0] * brightness).clamp(0.0, 1.0),
                (base_color[1] * brightness).clamp(0.0, 1.0),
                (base_color[2] * brightness).clamp(0.0, 1.0),
                (base_color[3] * (0.6 + pulse * 0.4)).clamp(0.2, 1.0),
            ];

            let center = Vector3::new(pos.x, pos.y, pos.z) + Vector3::new(0.5, 0.5, 0.5);
            let radius = 0.16 + 0.08 * intensity.min(1.5);
            let axes = [
                Vector3::new(radius, 0.0, 0.0),
                Vector3::new(0.0, radius, 0.0),
                Vector3::new(0.0, 0.0, radius),
            ];
            for dir in axes {
                let a = center + dir;
                let b = center - dir;
                self.power_vertices.push(HighlightVertex {
                    position: [a.x, a.y, a.z],
                    color,
                });
                self.power_vertices.push(HighlightVertex {
                    position: [b.x, b.y, b.z],
                    color,
                });
            }
        }

        self.power_vertex_count = self.power_vertices.len() as u32;
        self.ensure_power_capacity(self.power_vertices.len());
        if self.power_vertex_count > 0 {
            self.queue.write_buffer(
                &self.power_vertex_buffer,
                0,
                bytemuck::cast_slice(&self.power_vertices),
            );
        }
    }

    pub fn update_hand(
        &mut self,
        block_type: Option<BlockType>,
        camera: &Camera,
        animation_time: f32,
        breaking_progress: f32,
        placement_progress: f32,
    ) {
        let Some(block_type) = block_type else {
            self.hand_index_count = 0;
            return;
        };

        let scale = 0.18;
        let origin = Vector3::new(0.0, 0.0, 0.0);
        let mut mesh = mesh::generate_block_mesh(block_type, origin, scale);

        // Base hand position
        let mut hand_offset =
            camera.right() * 0.32 + camera.direction() * 0.5 - Vector3::new(0.0, 0.45, 0.0);

        // Idle sway animation (subtle bob and sway)
        let idle_sway_x = (animation_time * 1.5).sin() * 0.01;
        let idle_sway_y = (animation_time * 2.0).sin() * 0.008;
        hand_offset += Vector3::new(idle_sway_x, idle_sway_y, 0.0);

        // Breaking animation (shake)
        if breaking_progress > 0.0 {
            let shake_intensity = breaking_progress * 0.025;
            let shake_x = (animation_time * 25.0).sin() * shake_intensity;
            let shake_y = (animation_time * 30.0).cos() * shake_intensity;
            hand_offset += Vector3::new(shake_x, shake_y, 0.0);
        }

        // Placement animation (forward thrust that decays)
        if placement_progress > 0.0 {
            // Quick forward motion that eases out
            let thrust = (1.0 - placement_progress).powi(2) * 0.15;
            hand_offset += camera.direction() * thrust;
            hand_offset -= Vector3::new(0.0, (1.0 - placement_progress).powi(2) * 0.05, 0.0);
        }

        let hand_pos = Vector3::new(
            camera.position.x + hand_offset.x,
            camera.position.y + hand_offset.y,
            camera.position.z + hand_offset.z,
        );

        let rotation =
            Quaternion::from_angle_y(camera.yaw) * Quaternion::from_angle_x(Rad(-camera.pitch.0));

        for vertex in &mut mesh.vertices {
            let v = Vector3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
            let mut v = rotation.rotate_vector(v);
            v += hand_pos;
            vertex.position = [v.x, v.y, v.z];
            vertex.tint = [1.0, 1.0, 1.0];
        }

        self.ensure_hand_capacity(mesh.vertices.len(), mesh.indices.len());
        if !mesh.vertices.is_empty() {
            self.queue.write_buffer(
                &self.hand_vertex_buffer,
                0,
                bytemuck::cast_slice(&mesh.vertices),
            );
        }
        if !mesh.indices.is_empty() {
            self.queue.write_buffer(
                &self.hand_index_buffer,
                0,
                bytemuck::cast_slice(&mesh.indices),
            );
        }
        self.hand_index_count = mesh.indices.len() as u32;
    }

    pub fn update_entities(&mut self, entities: &[crate::entity::ItemEntity]) {
        use crate::mesh;
        use cgmath::Quaternion;

        let mut combined_vertices = Vec::new();
        let mut combined_indices = Vec::new();

        for entity in entities {
            let scale = 0.25; // Small item size
            let origin = Vector3::new(0.0, 0.0, 0.0);

            // Get the block type to render (for tools, use stone as placeholder)
            let block_to_render = match entity.item {
                crate::item::ItemType::Block(block) => block,
                crate::item::ItemType::Tool(_, _) => crate::block::BlockType::Stone, // TODO: Tool models
            };
            let mut item_mesh = mesh::generate_block_mesh(block_to_render, origin, scale);

            // Apply rotation (spin on Y axis)
            let rotation = Quaternion::from_angle_y(Rad(entity.rotation));

            let base_index = combined_vertices.len() as u32;

            for vertex in &mut item_mesh.vertices {
                let v = Vector3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
                let v = rotation.rotate_vector(v);

                // Translate to entity position
                vertex.position = [
                    v.x + entity.position.x,
                    v.y + entity.position.y,
                    v.z + entity.position.z,
                ];
                vertex.tint = [1.0, 1.0, 1.0];
                combined_vertices.push(*vertex);
            }

            for &index in &item_mesh.indices {
                combined_indices.push(base_index + index);
            }
        }

        self.ensure_entity_capacity(combined_vertices.len(), combined_indices.len());

        if !combined_vertices.is_empty() {
            self.queue.write_buffer(
                &self.entity_vertex_buffer,
                0,
                bytemuck::cast_slice(&combined_vertices),
            );
        }
        if !combined_indices.is_empty() {
            self.queue.write_buffer(
                &self.entity_index_buffer,
                0,
                bytemuck::cast_slice(&combined_indices),
            );
        }
        self.entity_index_count = combined_indices.len() as u32;
    }

    pub fn update_ui(&mut self, vertices: &[UiVertex], indices: &[u16]) {
        self.ui_vertices.clear();
        self.ui_vertices.extend_from_slice(vertices);
        self.ui_indices.clear();
        self.ui_indices.extend_from_slice(indices);

        self.ensure_ui_capacity(self.ui_vertices.len(), self.ui_indices.len());

        if !self.ui_vertices.is_empty() {
            self.queue.write_buffer(
                &self.ui_vertex_buffer,
                0,
                bytemuck::cast_slice(&self.ui_vertices),
            );
        }
        if !self.ui_indices.is_empty() {
            self.queue.write_buffer(
                &self.ui_index_buffer,
                0,
                bytemuck::cast_slice(&self.ui_indices),
            );
        }
        self.ui_index_count = self.ui_indices.len() as u32;
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(err) => {
                return match err {
                    wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => {
                        self.reconfigure_surface();
                        Ok(())
                    }
                    wgpu::SurfaceError::Timeout => Ok(()),
                    wgpu::SurfaceError::OutOfMemory => Err(err),
                };
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("renderer_encoder"),
            });

        let clear_color = wgpu::Color {
            r: self.clear_color[0] as f64,
            g: self.clear_color[1] as f64,
            b: self.clear_color[2] as f64,
            a: self.clear_color[3] as f64,
        };

        let frustum = Frustum::from_matrix(self.last_view_proj);

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.sky_pipeline);
            pass.set_bind_group(0, &self.environment_bind_group, &[]);
            pass.draw(0..3, 0..1);

            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(1, &self.texture_atlas.bind_group, &[]);
            pass.set_bind_group(2, &self.environment_bind_group, &[]);
            self.draw_world_chunks(&mut pass, &frustum);

            // Draw item entities
            if self.entity_index_count > 0 {
                pass.set_vertex_buffer(0, self.entity_vertex_buffer.slice(..));
                pass.set_index_buffer(self.entity_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.entity_index_count, 0, 0..1);
            }

            if self.highlight_vertex_count > 0 || self.power_vertex_count > 0 {
                pass.set_pipeline(&self.highlight_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                if self.highlight_vertex_count > 0 {
                    pass.set_vertex_buffer(0, self.highlight_vertex_buffer.slice(..));
                    pass.draw(0..self.highlight_vertex_count, 0..1);
                }
                if self.power_vertex_count > 0 {
                    pass.set_vertex_buffer(0, self.power_vertex_buffer.slice(..));
                    pass.draw(0..self.power_vertex_count, 0..1);
                }

                pass.set_pipeline(&self.render_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_bind_group(1, &self.texture_atlas.bind_group, &[]);
                pass.set_bind_group(2, &self.environment_bind_group, &[]);
            }

            if self.hand_index_count > 0 {
                pass.set_vertex_buffer(0, self.hand_vertex_buffer.slice(..));
                pass.set_index_buffer(self.hand_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.hand_index_count, 0, 0..1);
            }
        }

        if self.ui_index_count > 0 {
            let mut ui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            ui_pass.set_pipeline(&self.ui_pipeline);
            ui_pass.set_bind_group(0, &self.texture_atlas.bind_group, &[]);
            ui_pass.set_vertex_buffer(0, self.ui_vertex_buffer.slice(..));
            ui_pass.set_index_buffer(self.ui_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            ui_pass.draw_indexed(0..self.ui_index_count, 0, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();
        Ok(())
    }

    fn ensure_highlight_capacity(&mut self, required: usize) {
        let required = required.max(1);
        if required > self.highlight_vertex_capacity {
            self.highlight_vertex_capacity = required.next_power_of_two();
            self.highlight_vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("highlight_vertex_buffer"),
                size: (self.highlight_vertex_capacity * mem::size_of::<HighlightVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
    }

    fn ensure_power_capacity(&mut self, required: usize) {
        let required = required.max(1);
        if required > self.power_vertex_capacity {
            self.power_vertex_capacity = required.next_power_of_two();
            self.power_vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("power_vertex_buffer"),
                size: (self.power_vertex_capacity * mem::size_of::<HighlightVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
    }

    fn ensure_hand_capacity(&mut self, vertices: usize, indices: usize) {
        let vertices = vertices.max(1);
        if vertices > self.hand_vertex_capacity {
            self.hand_vertex_capacity = vertices.next_power_of_two();
            self.hand_vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("hand_vertex_buffer"),
                size: (self.hand_vertex_capacity * mem::size_of::<BlockVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        let indices = indices.max(1);
        if indices > self.hand_index_capacity {
            self.hand_index_capacity = indices.next_power_of_two();
            self.hand_index_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("hand_index_buffer"),
                size: (self.hand_index_capacity * mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
    }

    fn ensure_entity_capacity(&mut self, vertices: usize, indices: usize) {
        let vertices = vertices.max(1);
        if vertices > self.entity_vertex_capacity {
            self.entity_vertex_capacity = vertices.next_power_of_two();
            self.entity_vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("entity_vertex_buffer"),
                size: (self.entity_vertex_capacity * mem::size_of::<BlockVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        let indices = indices.max(1);
        if indices > self.entity_index_capacity {
            self.entity_index_capacity = indices.next_power_of_two();
            self.entity_index_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("entity_index_buffer"),
                size: (self.entity_index_capacity * mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
    }

    fn ensure_ui_capacity(&mut self, vertices: usize, indices: usize) {
        let vertices = vertices.max(1);
        if vertices > self.ui_vertex_capacity {
            self.ui_vertex_capacity = vertices.next_power_of_two();
            self.ui_vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ui_vertex_buffer"),
                size: (self.ui_vertex_capacity * mem::size_of::<UiVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        let indices = indices.max(1);
        if indices > self.ui_index_capacity {
            self.ui_index_capacity = indices.next_power_of_two();
            self.ui_index_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ui_index_buffer"),
                size: (self.ui_index_capacity * mem::size_of::<u16>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
    }
}

fn block_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: mem::size_of::<BlockVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 12,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 24,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 32,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 36,
                shader_location: 4,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 48,
                shader_location: 5,
            },
        ],
    }
}

fn highlight_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: mem::size_of::<HighlightVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 12,
                shader_location: 1,
            },
        ],
    }
}

fn ui_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: mem::size_of::<UiVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 8,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 24,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 32,
                shader_location: 3,
            },
        ],
    }
}

fn component_color(component: ElectricalComponent) -> [f32; 4] {
    match component {
        ElectricalComponent::Wire => [0.95, 0.55, 0.25, 0.9],
        ElectricalComponent::Resistor => [0.4, 0.8, 1.0, 0.9],
        ElectricalComponent::VoltageSource => [1.0, 0.35, 0.45, 0.95],
        ElectricalComponent::Ground => [0.6, 0.65, 0.7, 0.85],
    }
}
