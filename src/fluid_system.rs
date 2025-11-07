use std::collections::{HashSet, VecDeque};
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::chunk::{CHUNK_AREA, CHUNK_HEIGHT, CHUNK_SIZE};
use crate::fluid_gpu::{
    FluidGpu, TileChunkInfo, TileInput, TileOutput, DEFAULT_SIMULATION_ITERATIONS, TILE_EDGE_CHUNKS,
};
use crate::npu;
use crate::profiler;
use crate::world::{ChunkPos, World};

const MAX_IN_FLIGHT: usize = 2;
const GPU_THRESHOLD_MS: f32 = 6.0;
const GPU_RECOVER_RATIO: f32 = 0.45;
const GPU_COOLDOWN_MS: u64 = 80;
const CPU_FALLBACK_COOLDOWN_MS: u64 = 16;
const TILE_PADDING: usize = 1;
const PADDED_TILE_EDGE: usize = TILE_EDGE_CHUNKS + TILE_PADDING * 2;

enum WorkerCommand {
    Run(TileInput),
    Shutdown,
}

type WorkerResponse = Result<TileOutput>;

pub struct FluidSystem {
    sender: Option<Sender<WorkerCommand>>,
    result_receiver: Receiver<WorkerResponse>,
    pending_tiles: HashSet<(i32, i32)>,
    worker_handle: Option<thread::JoinHandle<()>>,
    gpu_times: VecDeque<f32>,
    gpu_overloaded_until: Instant,
    npu_available: bool,
    fallback_ready_at: Option<Instant>,
}

impl FluidSystem {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let (command_tx, command_rx) = mpsc::channel::<WorkerCommand>();
        let (result_tx, result_rx) = mpsc::channel::<WorkerResponse>();

        let handle = thread::Builder::new()
            .name("fluid-worker".into())
            .spawn(move || {
                let gpu = match FluidGpu::new(device.as_ref()) {
                    Ok(gpu) => gpu,
                    Err(err) => {
                        let _ = result_tx.send(Err(err));
                        return;
                    }
                };

                while let Ok(command) = command_rx.recv() {
                    match command {
                        WorkerCommand::Run(request) => {
                            let result = gpu.run_tile(device.as_ref(), queue.as_ref(), request);
                            let _ = result_tx.send(result);
                        }
                        WorkerCommand::Shutdown => break,
                    }
                }
            });

        let handle = match handle {
            Ok(h) => Some(h),
            Err(e) => {
                eprintln!("Warning: Failed to spawn fluid worker thread: {e}");
                eprintln!("Fluid simulation will fall back to CPU processing");
                None
            }
        };

        Self {
            sender: Some(command_tx),
            result_receiver: result_rx,
            pending_tiles: HashSet::new(),
            worker_handle: Some(handle),
            gpu_times: VecDeque::new(),
            gpu_overloaded_until: Instant::now(),
            npu_available: npu::is_available(),
            fallback_ready_at: None,
        }
    }

    pub fn pump(&mut self, world: &World) {
        if self.sender.is_none() {
            return;
        }

        while self.pending_tiles.len() < MAX_IN_FLIGHT {
            if self.is_overloaded() {
                break;
            }
            let mut scheduled = false;
            let active_chunks = world.active_fluid_chunks_snapshot();
            for chunk_pos in active_chunks {
                let base = (chunk_pos.x - 1, chunk_pos.z - 1);
                if self.pending_tiles.contains(&base) {
                    continue;
                }

                if let Some(request) = Self::build_tile_input(
                    world,
                    base.0,
                    base.1,
                    TILE_EDGE_CHUNKS,
                    TILE_EDGE_CHUNKS,
                ) {
                    if let Some(sender) = &self.sender {
                        if sender.send(WorkerCommand::Run(request)).is_ok() {
                            self.pending_tiles.insert(base);
                            scheduled = true;
                        } else {
                            self.sender = None;
                        }
                    }
                    break;
                }
            }

            if !scheduled {
                break;
            }
        }
    }

    pub fn poll_results(&mut self, world: &mut World) -> bool {
        let mut world_changed = false;
        loop {
            let response = self.result_receiver.try_recv();
            let output = match response {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.sender = None;
                    break;
                }
            };

            match output {
                Ok(tile_output) => {
                    self.update_gpu_load(tile_output.compute_time_ms);
                    self.handle_tile_output(world, tile_output, &mut world_changed);
                }
                Err(err) => {
                    eprintln!("Fluid worker failed: {err:?}");
                }
            }
        }

        world_changed
    }

    fn handle_tile_output(
        &mut self,
        world: &mut World,
        output: TileOutput,
        world_changed: &mut bool,
    ) {
        self.pending_tiles
            .remove(&(output.base_chunk.x, output.base_chunk.z));

        profiler::record_background(
            "fluid_gpu_tile",
            Duration::from_secs_f32(output.compute_time_ms / 1000.0),
        );

        {
            let chunks_map = world.chunks_mut();
            for update in output
                .chunk_updates
                .iter()
                .filter(|u| u.exists && u.is_core)
            {
                if let Some(chunk) = chunks_map.get_mut(&update.pos) {
                    chunk.apply_fluids(&update.fluids);
                }
            }
        }

        for update in output.chunk_updates {
            if !update.is_core {
                continue;
            }

            if !update.exists {
                world.finalize_fluid_chunk_state(update.pos, false, false);
                continue;
            }

            if update.changed {
                *world_changed = true;
            }

            world.finalize_fluid_chunk_state(update.pos, update.changed, update.has_fluid);
        }
    }

    fn update_gpu_load(&mut self, tile_ms: f32) {
        if !tile_ms.is_finite() {
            return;
        }
        self.gpu_times.push_back(tile_ms);
        if self.gpu_times.len() > 32 {
            self.gpu_times.pop_front();
        }

        let sum: f32 = self.gpu_times.iter().copied().sum();
        let avg = sum / self.gpu_times.len() as f32;

        if self.gpu_times.len() >= 4 && avg > GPU_THRESHOLD_MS {
            self.gpu_overloaded_until = Instant::now() + Duration::from_millis(GPU_COOLDOWN_MS);
        } else if avg < GPU_THRESHOLD_MS * GPU_RECOVER_RATIO {
            if Instant::now() >= self.gpu_overloaded_until {
                self.gpu_overloaded_until = Instant::now();
            }
        }
    }

    pub fn is_overloaded(&self) -> bool {
        Instant::now() < self.gpu_overloaded_until
    }

    pub fn fallback_step(&mut self, world: &mut World) -> bool {
        if !self.is_overloaded() {
            return false;
        }

        let now = Instant::now();
        if let Some(ready) = self.fallback_ready_at {
            if now < ready {
                return false;
            }
        }

        let changed = if self.npu_available {
            npu::process_world(world)
        } else {
            world.step_fluids()
        };

        self.fallback_ready_at = Some(now + Duration::from_millis(CPU_FALLBACK_COOLDOWN_MS));

        if changed {
            self.gpu_times.clear();
            self.gpu_overloaded_until = Instant::now();
        }

        changed
    }

    fn build_tile_input(
        world: &World,
        base_chunk_x: i32,
        base_chunk_z: i32,
        _chunks_wide: usize,
        _chunks_deep: usize,
    ) -> Option<TileInput> {
        let total_chunks_wide = PADDED_TILE_EDGE;
        let total_chunks_deep = PADDED_TILE_EDGE;
        let core_start_x = TILE_PADDING;
        let core_start_z = TILE_PADDING;
        let core_end_x = core_start_x + TILE_EDGE_CHUNKS;
        let core_end_z = core_start_z + TILE_EDGE_CHUNKS;

        let tile_width_blocks = total_chunks_wide * CHUNK_SIZE;
        let tile_depth_blocks = total_chunks_deep * CHUNK_SIZE;
        let total_cells = tile_width_blocks * tile_depth_blocks * CHUNK_HEIGHT;

        let mut original = vec![0u32; total_cells];
        let mut solid = vec![0u32; total_cells];
        let mut chunk_info = Vec::with_capacity(total_chunks_wide * total_chunks_deep);

        let mut any_core_exists = false;

        for dz in 0..total_chunks_deep {
            for dx in 0..total_chunks_wide {
                let chunk_pos = ChunkPos {
                    x: base_chunk_x + dx as i32 - TILE_PADDING as i32,
                    z: base_chunk_z + dz as i32 - TILE_PADDING as i32,
                };

                let is_core =
                    dx >= core_start_x && dx < core_end_x && dz >= core_start_z && dz < core_end_z;

                if let Some(chunk) = world.chunks().get(&chunk_pos) {
                    if is_core {
                        any_core_exists = true;
                    }

                    let cell_state = chunk.cell_state();
                    for (linear, state) in cell_state.iter().copied().enumerate() {
                        let y = linear / CHUNK_AREA;
                        let rem = linear % CHUNK_AREA;
                        let local_z = rem / CHUNK_SIZE;
                        let local_x = rem % CHUNK_SIZE;
                        let global_x = dx * CHUNK_SIZE + local_x;
                        let global_z = dz * CHUNK_SIZE + local_z;
                        let idx =
                            index_3d(global_x, y, global_z, tile_width_blocks, tile_depth_blocks);
                        if state < 0 {
                            solid[idx] = 1;
                            original[idx] = 0;
                        } else {
                            solid[idx] = 0;
                            original[idx] = state as u32;
                        }
                    }

                    chunk_info.push(TileChunkInfo {
                        pos: chunk_pos,
                        exists: true,
                        is_core,
                    });
                } else {
                    if is_core {
                        return None;
                    }

                    chunk_info.push(TileChunkInfo {
                        pos: chunk_pos,
                        exists: false,
                        is_core,
                    });

                    let chunk_offset_x = dx * CHUNK_SIZE;
                    let chunk_offset_z = dz * CHUNK_SIZE;
                    for y in 0..CHUNK_HEIGHT {
                        for local_z in 0..CHUNK_SIZE {
                            for local_x in 0..CHUNK_SIZE {
                                let idx = index_3d(
                                    chunk_offset_x + local_x,
                                    y,
                                    chunk_offset_z + local_z,
                                    tile_width_blocks,
                                    tile_depth_blocks,
                                );
                                solid[idx] = 1;
                            }
                        }
                    }
                }
            }
        }

        if !any_core_exists {
            return None;
        }

        Some(TileInput {
            base_chunk: ChunkPos {
                x: base_chunk_x,
                z: base_chunk_z,
            },
            chunks_wide: total_chunks_wide,
            chunks_deep: total_chunks_deep,
            tile_width_blocks,
            tile_depth_blocks,
            original,
            solid,
            iterations: DEFAULT_SIMULATION_ITERATIONS,
            chunk_info,
        })
    }
}

impl Drop for FluidSystem {
    fn drop(&mut self) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(WorkerCommand::Shutdown);
        }

        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

fn index_3d(x: usize, y: usize, z: usize, width: usize, depth: usize) -> usize {
    x + width * (z + depth * y)
}
