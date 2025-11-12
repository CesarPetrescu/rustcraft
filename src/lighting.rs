use std::collections::VecDeque;

use crate::block::BlockType;
use crate::chunk::{CHUNK_HEIGHT, CHUNK_SIZE};
use crate::world::{ChunkPos, World};

/// Light propagation system for skylight and blocklight
pub struct LightingSystem;

impl LightingSystem {
    /// Calculate initial skylight for a chunk (top-down flood fill)
    pub fn calculate_skylight(world: &mut World, chunk_pos: ChunkPos) {
        // Step 1: Set top layer to max skylight (15)
        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                if let Some(chunk) = world.chunks_mut().get_mut(&chunk_pos) {
                    chunk.set_skylight(x, CHUNK_HEIGHT - 1, z, 15);
                }
            }
        }

        // Step 2: Propagate skylight downward through transparent blocks
        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let mut light_level = 15u8;

                for y in (0..CHUNK_HEIGHT).rev() {
                    if let Some(chunk) = world.chunks_mut().get_mut(&chunk_pos) {
                        let block = chunk.get_block(x, y, z);

                        // Set current light level
                        chunk.set_skylight(x, y, z, light_level);

                        // If block is opaque, stop skylight
                        if block.occludes() {
                            light_level = 0;
                        }
                    }
                }
            }
        }

        // Step 3: Propagate laterally (BFS)
        Self::propagate_skylight(world, chunk_pos);
    }

    /// Propagate skylight laterally using BFS
    fn propagate_skylight(world: &mut World, chunk_pos: ChunkPos) {
        let mut queue = VecDeque::new();

        // Collect all lit blocks as starting points
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_HEIGHT {
                for z in 0..CHUNK_SIZE {
                    if let Some(chunk) = world.chunks().get(&chunk_pos) {
                        let light = chunk.get_skylight(x, y, z);
                        if light > 0 {
                            let world_x = chunk_pos.x * CHUNK_SIZE as i32 + x as i32;
                            let world_z = chunk_pos.z * CHUNK_SIZE as i32 + z as i32;
                            queue.push_back((world_x, y as i32, world_z, light));
                        }
                    }
                }
            }
        }

        // BFS propagation
        while let Some((wx, wy, wz, light)) = queue.pop_front() {
            if light <= 1 {
                continue; // Can't propagate further
            }

            let new_light = light - 1;

            // Check all 6 neighbors
            for (dx, dy, dz) in [
                (1, 0, 0), (-1, 0, 0),
                (0, 1, 0), (0, -1, 0),
                (0, 0, 1), (0, 0, -1),
            ] {
                let nx = wx + dx;
                let ny = wy + dy;
                let nz = wz + dz;

                if ny < 0 || ny >= CHUNK_HEIGHT as i32 {
                    continue;
                }

                let neighbor_chunk_pos = ChunkPos {
                    x: nx.div_euclid(CHUNK_SIZE as i32),
                    z: nz.div_euclid(CHUNK_SIZE as i32),
                };

                let local_x = nx.rem_euclid(CHUNK_SIZE as i32) as usize;
                let local_y = ny as usize;
                let local_z = nz.rem_euclid(CHUNK_SIZE as i32) as usize;

                if let Some(neighbor_chunk) = world.chunks_mut().get_mut(&neighbor_chunk_pos) {
                    let block = neighbor_chunk.get_block(local_x, local_y, local_z);
                    let current_light = neighbor_chunk.get_skylight(local_x, local_y, local_z);

                    // Only propagate if neighbor is transparent and has less light
                    if !block.occludes() && current_light < new_light {
                        neighbor_chunk.set_skylight(local_x, local_y, local_z, new_light);
                        queue.push_back((nx, ny, nz, new_light));
                    }
                }
            }
        }
    }

    /// Calculate blocklight from light-emitting blocks
    pub fn calculate_blocklight(world: &mut World, chunk_pos: ChunkPos) {
        let mut queue = VecDeque::new();

        // Find all light-emitting blocks
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_HEIGHT {
                for z in 0..CHUNK_SIZE {
                    if let Some(chunk) = world.chunks().get(&chunk_pos) {
                        let block = chunk.get_block(x, y, z);
                        let emission = block.light_emission();

                        if emission > 0.0 {
                            // Convert 0.0-1.0 emission to 0-15 light level
                            let light_level = (emission * 15.0).round() as u8;

                            if let Some(chunk) = world.chunks_mut().get_mut(&chunk_pos) {
                                chunk.set_blocklight(x, y, z, light_level);

                                let world_x = chunk_pos.x * CHUNK_SIZE as i32 + x as i32;
                                let world_z = chunk_pos.z * CHUNK_SIZE as i32 + z as i32;
                                queue.push_back((world_x, y as i32, world_z, light_level));
                            }
                        }
                    }
                }
            }
        }

        // BFS propagation
        while let Some((wx, wy, wz, light)) = queue.pop_front() {
            if light <= 1 {
                continue;
            }

            let new_light = light - 1;

            // Check all 6 neighbors
            for (dx, dy, dz) in [
                (1, 0, 0), (-1, 0, 0),
                (0, 1, 0), (0, -1, 0),
                (0, 0, 1), (0, 0, -1),
            ] {
                let nx = wx + dx;
                let ny = wy + dy;
                let nz = wz + dz;

                if ny < 0 || ny >= CHUNK_HEIGHT as i32 {
                    continue;
                }

                let neighbor_chunk_pos = ChunkPos {
                    x: nx.div_euclid(CHUNK_SIZE as i32),
                    z: nz.div_euclid(CHUNK_SIZE as i32),
                };

                let local_x = nx.rem_euclid(CHUNK_SIZE as i32) as usize;
                let local_y = ny as usize;
                let local_z = nz.rem_euclid(CHUNK_SIZE as i32) as usize;

                if let Some(neighbor_chunk) = world.chunks_mut().get_mut(&neighbor_chunk_pos) {
                    let block = neighbor_chunk.get_block(local_x, local_y, local_z);
                    let current_light = neighbor_chunk.get_blocklight(local_x, local_y, local_z);

                    // Propagate through transparent blocks
                    if !block.occludes() && current_light < new_light {
                        neighbor_chunk.set_blocklight(local_x, local_y, local_z, new_light);
                        queue.push_back((nx, ny, nz, new_light));
                    }
                }
            }
        }
    }

    /// Recalculate lighting after block placement/removal
    pub fn update_light_at(world: &mut World, world_x: i32, world_y: i32, world_z: i32) {
        let chunk_pos = ChunkPos {
            x: world_x.div_euclid(CHUNK_SIZE as i32),
            z: world_z.div_euclid(CHUNK_SIZE as i32),
        };

        // Recalculate both skylight and blocklight for affected chunk
        Self::calculate_skylight(world, chunk_pos);
        Self::calculate_blocklight(world, chunk_pos);

        // Also update adjacent chunks that might be affected
        for dx in -1..=1 {
            for dz in -1..=1 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                let neighbor_pos = ChunkPos {
                    x: chunk_pos.x + dx,
                    z: chunk_pos.z + dz,
                };
                if world.chunks().contains_key(&neighbor_pos) {
                    Self::calculate_skylight(world, neighbor_pos);
                    Self::calculate_blocklight(world, neighbor_pos);
                }
            }
        }
    }
}
