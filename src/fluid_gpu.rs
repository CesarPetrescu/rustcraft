use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

use anyhow::{anyhow, Result};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::chunk::{CHUNK_HEIGHT, CHUNK_SIZE, CHUNK_VOLUME};
use crate::world::{ChunkPos, MAX_FLUID_LEVEL};

pub const TILE_EDGE_CHUNKS: usize = 3;
pub const DEFAULT_SIMULATION_ITERATIONS: u32 = 4;
const MAX_FLUID_LEVEL_U32: u32 = MAX_FLUID_LEVEL as u32;

const VERTICAL_WORKGROUP: (u32, u32, u32) = (8, 8, 1);
const LATERAL_WORKGROUP: (u32, u32, u32) = (8, 8, 1);

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SimParams {
    grid_width_blocks: u32,
    grid_depth_blocks: u32,
    grid_height: u32,
    _pad: u32,
}

#[derive(Clone, Copy)]
pub struct TileChunkInfo {
    pub pos: ChunkPos,
    pub exists: bool,
    pub is_core: bool,
}

#[derive(Clone)]
pub struct TileInput {
    pub base_chunk: ChunkPos,
    pub chunks_wide: usize,
    pub chunks_deep: usize,
    pub tile_width_blocks: usize,
    pub tile_depth_blocks: usize,
    pub original: Vec<u32>,
    pub solid: Vec<u32>,
    pub iterations: u32,
    pub chunk_info: Vec<TileChunkInfo>,
}

pub struct ChunkUpdate {
    pub pos: ChunkPos,
    pub fluids: Vec<u8>,
    pub changed: bool,
    pub has_fluid: bool,
    pub exists: bool,
    pub is_core: bool,
}

pub struct TileOutput {
    pub base_chunk: ChunkPos,
    pub chunk_updates: Vec<ChunkUpdate>,
    pub compute_time_ms: f32,
}

pub struct FluidGpu {
    resource_layout: wgpu::BindGroupLayout,
    io_layout: wgpu::BindGroupLayout,
    vertical_pipeline: wgpu::ComputePipeline,
    lateral_x_pipeline: wgpu::ComputePipeline,
    lateral_z_pipeline: wgpu::ComputePipeline,
}

impl FluidGpu {
    pub fn new(device: &wgpu::Device) -> Result<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fluid_compute_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("fluid_compute.wgsl").into()),
        });

        let resource_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fluid_resource_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let io_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fluid_io_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fluid_pipeline_layout"),
            bind_group_layouts: &[&resource_layout, &io_layout],
            push_constant_ranges: &[],
        });

        let vertical_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("fluid_vertical_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "vertical_pass",
        });

        let lateral_x_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("fluid_lateral_x_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "equalize_x",
        });

        let lateral_z_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("fluid_lateral_z_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "equalize_z",
        });

        Ok(Self {
            resource_layout,
            io_layout,
            vertical_pipeline,
            lateral_x_pipeline,
            lateral_z_pipeline,
        })
    }

    pub fn run_tile(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input: TileInput,
    ) -> Result<TileOutput> {
        let start_time = Instant::now();

        let TileInput {
            base_chunk,
            chunks_wide,
            chunks_deep,
            tile_width_blocks,
            tile_depth_blocks,
            original,
            solid,
            iterations,
            chunk_info,
            ..
        } = input;

        if chunk_info.len() != chunks_wide * chunks_deep {
            return Err(anyhow!(
                "chunk info length {} does not match grid {}x{}",
                chunk_info.len(),
                chunks_wide,
                chunks_deep
            ));
        }

        let total_cells = tile_width_blocks * tile_depth_blocks * CHUNK_HEIGHT;
        if original.len() != total_cells || solid.len() != total_cells {
            return Err(anyhow!(
                "tile buffers have incorrect length (expected {}, got orig {} solid {})",
                total_cells,
                original.len(),
                solid.len()
            ));
        }

        let buffer_size = (total_cells * std::mem::size_of::<u32>()) as u64;

        let original_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fluid_original_tile_buffer"),
            contents: bytemuck::cast_slice(&original),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let current_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fluid_current_tile_buffer"),
            contents: bytemuck::cast_slice(&original),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });

        let temp_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fluid_temp_tile_buffer"),
            contents: bytemuck::cast_slice(&original),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });

        let solid_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fluid_solid_tile_buffer"),
            contents: bytemuck::cast_slice(&solid),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let params = SimParams {
            grid_width_blocks: tile_width_blocks as u32,
            grid_depth_blocks: tile_depth_blocks as u32,
            grid_height: CHUNK_HEIGHT as u32,
            _pad: 0,
        };

        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fluid_tile_params_buffer"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let resources_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fluid_tile_resources"),
            layout: &self.resource_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: original_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: solid_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("fluid_tile_encoder"),
        });

        let mut src_buffer = &current_buffer;
        let mut dst_buffer = &temp_buffer;
        let iteration_count = iterations.max(1);

        for _iter in 0..iteration_count {
            run_pass(
                device,
                &mut encoder,
                &self.io_layout,
                &resources_bind_group,
                &self.vertical_pipeline,
                src_buffer,
                dst_buffer,
                "fluid_tile_vertical",
                dispatch_counts(
                    tile_width_blocks as u32,
                    tile_depth_blocks as u32,
                    VERTICAL_WORKGROUP,
                ),
            );
            std::mem::swap(&mut src_buffer, &mut dst_buffer);

            run_pass(
                device,
                &mut encoder,
                &self.io_layout,
                &resources_bind_group,
                &self.lateral_x_pipeline,
                src_buffer,
                dst_buffer,
                "fluid_tile_lateral_x",
                dispatch_counts(
                    CHUNK_HEIGHT as u32,
                    tile_depth_blocks as u32,
                    LATERAL_WORKGROUP,
                ),
            );
            std::mem::swap(&mut src_buffer, &mut dst_buffer);

            run_pass(
                device,
                &mut encoder,
                &self.io_layout,
                &resources_bind_group,
                &self.lateral_z_pipeline,
                src_buffer,
                dst_buffer,
                "fluid_tile_lateral_z",
                dispatch_counts(
                    CHUNK_HEIGHT as u32,
                    tile_width_blocks as u32,
                    LATERAL_WORKGROUP,
                ),
            );
            std::mem::swap(&mut src_buffer, &mut dst_buffer);
        }

        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fluid_tile_readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        encoder.copy_buffer_to_buffer(src_buffer, 0, &readback_buffer, 0, buffer_size);

        queue.submit(Some(encoder.finish()));

        let buffer_slice = readback_buffer.slice(..);
        let map_signal = Arc::new((Mutex::new(None), Condvar::new()));
        {
            let map_signal = Arc::clone(&map_signal);
            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                let (lock, cvar) = &*map_signal;
                let mut guard = lock.lock().expect("map_async poison");
                *guard = Some(result);
                cvar.notify_one();
            });
        }

        device.poll(wgpu::Maintain::Wait);

        let (lock, cvar) = &*map_signal;
        let mut guard = lock.lock().expect("map_async wait poison");
        while guard.is_none() {
            guard = cvar.wait(guard).expect("map_async wait poison");
        }
        guard
            .take()
            .unwrap()
            .map_err(|e| anyhow!("Failed to map fluid buffer: {e:?}"))?;

        let data = buffer_slice.get_mapped_range();
        let final_fluids: Vec<u32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        readback_buffer.unmap();

        let mut updates = Vec::with_capacity(chunk_info.len());

        for (index, info) in chunk_info.iter().enumerate() {
            if !info.is_core || !info.exists {
                updates.push(ChunkUpdate {
                    pos: info.pos,
                    fluids: Vec::new(),
                    changed: false,
                    has_fluid: false,
                    exists: info.exists,
                    is_core: info.is_core,
                });
                continue;
            }

            let mut chunk_fluids = vec![0u8; CHUNK_VOLUME];
            let mut chunk_changed = false;
            let mut chunk_has_fluid = false;

            let dz = index / chunks_wide;
            let dx = index % chunks_wide;
            let chunk_offset_x = dx * CHUNK_SIZE;
            let chunk_offset_z = dz * CHUNK_SIZE;

            for y in 0..CHUNK_HEIGHT {
                for local_z in 0..CHUNK_SIZE {
                    for local_x in 0..CHUNK_SIZE {
                        let global_x = chunk_offset_x + local_x;
                        let global_z = chunk_offset_z + local_z;
                        let idx =
                            index_3d(global_x, y, global_z, tile_width_blocks, tile_depth_blocks);
                        let new_amount = final_fluids[idx].min(MAX_FLUID_LEVEL_U32) as u8;
                        let prev_amount = original[idx] as u8;
                        if new_amount != prev_amount {
                            chunk_changed = true;
                        }
                        if new_amount > 0 {
                            chunk_has_fluid = true;
                        }
                        let local_idx = chunk_index(local_x, y, local_z);
                        chunk_fluids[local_idx] = new_amount;
                    }
                }
            }

            updates.push(ChunkUpdate {
                pos: info.pos,
                fluids: chunk_fluids,
                changed: chunk_changed,
                has_fluid: chunk_has_fluid,
                exists: true,
                is_core: true,
            });
        }

        Ok(TileOutput {
            base_chunk,
            chunk_updates: updates,
            compute_time_ms: start_time.elapsed().as_secs_f32() * 1000.0,
        })
    }
}

fn dispatch_counts(dim_x: u32, dim_y: u32, group: (u32, u32, u32)) -> (u32, u32, u32) {
    let dispatch_x = div_ceil(dim_x, group.0);
    let dispatch_y = div_ceil(dim_y, group.1);
    (dispatch_x, dispatch_y, group.2)
}

fn run_pass(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    io_layout: &wgpu::BindGroupLayout,
    resources_bind_group: &wgpu::BindGroup,
    pipeline: &wgpu::ComputePipeline,
    src: &wgpu::Buffer,
    dst: &wgpu::Buffer,
    label: &str,
    dispatch: (u32, u32, u32),
) {
    let io_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout: io_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: src.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: dst.as_entire_binding(),
            },
        ],
    });

    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some(label),
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, resources_bind_group, &[]);
    pass.set_bind_group(1, &io_bind_group, &[]);
    pass.dispatch_workgroups(dispatch.0, dispatch.1, dispatch.2);
}

fn div_ceil(value: u32, denom: u32) -> u32 {
    (value + denom - 1) / denom
}

fn index_3d(x: usize, y: usize, z: usize, width: usize, depth: usize) -> usize {
    x + width * (z + depth * y)
}

fn chunk_index(x: usize, y: usize, z: usize) -> usize {
    x + CHUNK_SIZE * (z + CHUNK_SIZE * y)
}
