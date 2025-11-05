use crate::{block::RenderKind, world::World};
use cgmath::{InnerSpace, Point3, Vector3};

pub struct RaycastHit {
    pub block_pos: (i32, i32, i32),
    pub normal: Vector3<f32>,
}

pub fn raycast(
    world: &World,
    origin: Point3<f32>,
    direction: Vector3<f32>,
    max_distance: f32,
) -> Option<RaycastHit> {
    let direction = direction.normalize();
    let step = 0.1;
    let max_steps = (max_distance / step) as i32;

    let mut prev_block_pos = (
        origin.x.floor() as i32,
        origin.y.floor() as i32,
        origin.z.floor() as i32,
    );

    for i in 1..max_steps {
        let t = i as f32 * step;
        let point = origin + direction * t;
        let block_pos = (
            point.x.floor() as i32,
            point.y.floor() as i32,
            point.z.floor() as i32,
        );

        let block = world.get_block(block_pos.0, block_pos.1, block_pos.2);

        if block.is_solid() || matches!(block.render_kind(), RenderKind::Electrical(_)) {
            // Calculate normal from previous position
            let normal = Vector3::new(
                (prev_block_pos.0 - block_pos.0) as f32,
                (prev_block_pos.1 - block_pos.1) as f32,
                (prev_block_pos.2 - block_pos.2) as f32,
            );

            return Some(RaycastHit { block_pos, normal });
        }

        prev_block_pos = block_pos;
    }

    None
}
