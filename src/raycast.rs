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

    // Current voxel position
    let mut voxel_x = origin.x.floor() as i32;
    let mut voxel_y = origin.y.floor() as i32;
    let mut voxel_z = origin.z.floor() as i32;

    // Direction to step in each axis (-1, 0, or 1)
    let step_x = if direction.x > 0.0 { 1 } else if direction.x < 0.0 { -1 } else { 0 };
    let step_y = if direction.y > 0.0 { 1 } else if direction.y < 0.0 { -1 } else { 0 };
    let step_z = if direction.z > 0.0 { 1 } else if direction.z < 0.0 { -1 } else { 0 };

    // Distance to travel along ray to cross one voxel boundary in each axis
    let t_delta_x = if direction.x != 0.0 { (1.0 / direction.x).abs() } else { f32::MAX };
    let t_delta_y = if direction.y != 0.0 { (1.0 / direction.y).abs() } else { f32::MAX };
    let t_delta_z = if direction.z != 0.0 { (1.0 / direction.z).abs() } else { f32::MAX };

    // Distance along ray to next voxel boundary in each axis
    let mut t_max_x = if direction.x != 0.0 {
        let boundary = if direction.x > 0.0 {
            (voxel_x + 1) as f32
        } else {
            voxel_x as f32
        };
        (boundary - origin.x) / direction.x
    } else {
        f32::MAX
    };

    let mut t_max_y = if direction.y != 0.0 {
        let boundary = if direction.y > 0.0 {
            (voxel_y + 1) as f32
        } else {
            voxel_y as f32
        };
        (boundary - origin.y) / direction.y
    } else {
        f32::MAX
    };

    let mut t_max_z = if direction.z != 0.0 {
        let boundary = if direction.z > 0.0 {
            (voxel_z + 1) as f32
        } else {
            voxel_z as f32
        };
        (boundary - origin.z) / direction.z
    } else {
        f32::MAX
    };

    // Track which face we entered from (for normal calculation)
    let mut normal = Vector3::new(0.0, 0.0, 0.0);
    let mut current_t = 0.0;

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
                current_t = t_max_x;
                if current_t > max_distance {
                    break;
                }
                voxel_x += step_x;
                t_max_x += t_delta_x;
                normal = Vector3::new(-step_x as f32, 0.0, 0.0);
            } else {
                current_t = t_max_z;
                if current_t > max_distance {
                    break;
                }
                voxel_z += step_z;
                t_max_z += t_delta_z;
                normal = Vector3::new(0.0, 0.0, -step_z as f32);
            }
        } else if t_max_y < t_max_z {
            current_t = t_max_y;
            if current_t > max_distance {
                break;
            }
            voxel_y += step_y;
            t_max_y += t_delta_y;
            normal = Vector3::new(0.0, -step_y as f32, 0.0);
        } else {
            current_t = t_max_z;
            if current_t > max_distance {
                break;
            }
            voxel_z += step_z;
            t_max_z += t_delta_z;
            normal = Vector3::new(0.0, 0.0, -step_z as f32);
        }
    }

    None
}
