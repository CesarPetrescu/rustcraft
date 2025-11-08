use crate::{block::RenderKind, world::World};
use cgmath::{InnerSpace, Point3, Vector3};

pub struct RaycastHit {
    pub block_pos: (i32, i32, i32),
    pub normal: Vector3<f32>,
}

/// DDA (Digital Differential Analyzer) voxel traversal raycast
/// This is significantly more efficient than fixed-step raycasting
/// Complexity: O(distance_in_blocks) instead of O(distance/step_size)
pub fn raycast(
    world: &World,
    origin: Point3<f32>,
    direction: Vector3<f32>,
    max_distance: f32,
) -> Option<RaycastHit> {
    let direction = direction.normalize();

    // Convert world position to voxel indices (blocks are centered on integers)
    let mut voxel_x = block_index(origin.x);
    let mut voxel_y = block_index(origin.y);
    let mut voxel_z = block_index(origin.z);

    // Direction to step in each axis (-1, 0, or 1)
    let step_x = if direction.x > 0.0 { 1 } else if direction.x < 0.0 { -1 } else { 0 };
    let step_y = if direction.y > 0.0 { 1 } else if direction.y < 0.0 { -1 } else { 0 };
    let step_z = if direction.z > 0.0 { 1 } else if direction.z < 0.0 { -1 } else { 0 };

    // Distance to travel along ray to cross one voxel boundary in each axis
    let t_delta_x = if direction.x != 0.0 { (1.0 / direction.x).abs() } else { f32::MAX };
    let t_delta_y = if direction.y != 0.0 { (1.0 / direction.y).abs() } else { f32::MAX };
    let t_delta_z = if direction.z != 0.0 { (1.0 / direction.z).abs() } else { f32::MAX };

    // Distance along ray to next voxel boundary in each axis
    let mut t_max_x = next_boundary_t(origin.x, direction.x, voxel_x);
    let mut t_max_y = next_boundary_t(origin.y, direction.y, voxel_y);
    let mut t_max_z = next_boundary_t(origin.z, direction.z, voxel_z);

    // Track which face we entered from (for normal calculation)
    let mut normal = Vector3::new(0.0, 0.0, 0.0);
    // Traverse voxels
    let max_steps = (max_distance * 2.0) as i32; // Safety limit
    for _ in 0..max_steps {
        // Check current voxel
        let block = world.get_block(voxel_x, voxel_y, voxel_z);
        if block.is_solid() || matches!(block.render_kind(), RenderKind::Electrical(_)) {
            return Some(RaycastHit {
                block_pos: (voxel_x, voxel_y, voxel_z),
                normal,
            });
        }

        // Step to next voxel along the axis with the smallest t_max
        if t_max_x < t_max_y {
            if t_max_x < t_max_z {
                if t_max_x > max_distance {
                    break;
                }
                voxel_x += step_x;
                t_max_x += t_delta_x;
                normal = Vector3::new(-step_x as f32, 0.0, 0.0);
            } else {
                if t_max_z > max_distance {
                    break;
                }
                voxel_z += step_z;
                t_max_z += t_delta_z;
                normal = Vector3::new(0.0, 0.0, -step_z as f32);
            }
        } else if t_max_y < t_max_z {
            if t_max_y > max_distance {
                break;
            }
            voxel_y += step_y;
            t_max_y += t_delta_y;
            normal = Vector3::new(0.0, -step_y as f32, 0.0);
        } else {
            if t_max_z > max_distance {
                break;
            }
            voxel_z += step_z;
            t_max_z += t_delta_z;
            normal = Vector3::new(0.0, 0.0, -step_z as f32);
        }
    }

    None
}

fn block_index(coord: f32) -> i32 {
    (coord + 0.5).floor() as i32
}

fn next_boundary_t(origin: f32, direction: f32, voxel: i32) -> f32 {
    if direction > 0.0 {
        let boundary = voxel as f32 + 0.5;
        (boundary - origin) / direction
    } else if direction < 0.0 {
        let boundary = voxel as f32 - 0.5;
        (boundary - origin) / direction
    } else {
        f32::MAX
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockType;
    use crate::chunk::{Chunk, CHUNK_SIZE};
    use crate::world::{ChunkPos, World};
    use cgmath::{point3, vec3};
    use std::collections::HashMap;

    fn place_block(world: &mut World, pos: (i32, i32, i32), block: BlockType) {
        let (x, y, z) = pos;
        let chunk_pos = ChunkPos {
            x: x.div_euclid(CHUNK_SIZE as i32),
            z: z.div_euclid(CHUNK_SIZE as i32),
        };
        let local_x = x.rem_euclid(CHUNK_SIZE as i32) as usize;
        let local_y = y as usize;
        let local_z = z.rem_euclid(CHUNK_SIZE as i32) as usize;

        let chunk_map: &mut HashMap<ChunkPos, Chunk> = world.chunks_mut();
        let chunk = chunk_map.entry(chunk_pos).or_insert_with(Chunk::new);
        chunk.set_block(local_x, local_y, local_z, block);
    }

    #[test]
    fn ray_hits_block_directly_ahead() {
        let mut world = World::new();
        place_block(&mut world, (0, 80, 5), BlockType::Stone);

        let origin = point3(0.0, 80.0, 0.0);
        let direction = vec3(0.0, 0.0, 1.0);
        let hit = raycast(&world, origin, direction, 10.0).expect("should hit block");
        assert_eq!(hit.block_pos, (0, 80, 5));
    }

    #[test]
    fn tiny_horizontal_bias_does_not_skip_center_block() {
        let mut world = World::new();
        place_block(&mut world, (0, 64, 6), BlockType::Stone);

        let origin = point3(0.0, 64.0, 0.0);
        let mut direction = vec3(-1e-6, 0.0, 1.0);
        direction = direction.normalize();

        let hit = raycast(&world, origin, direction, 10.0).expect("should hit front block");
        assert_eq!(hit.block_pos, (0, 64, 6));
    }
}
