use crate::item::ItemType;
use cgmath::{Point3, Vector3};

/// Represents an item entity in the world (dropped item with physics)
#[derive(Clone, Debug)]
pub struct ItemEntity {
    pub position: Point3<f32>,
    pub velocity: Vector3<f32>,
    pub item: ItemType,
    pub age: f32,           // Time alive in seconds
    pub pickup_delay: f32,  // Time before can be picked up
    pub rotation: f32,      // Y-axis rotation for spinning effect
}

impl ItemEntity {
    /// Creates a new item entity at the given position
    pub fn new(position: Point3<f32>, item: ItemType) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        // Use system time + position for seed to get true randomness
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let pos_seed = ((position.x * 1000.0) as i64 +
                       (position.y * 1000.0) as i64 +
                       (position.z * 1000.0) as i64) as u64;
        let combined_seed = seed.wrapping_add(pos_seed);

        // Simple LCG for random numbers
        let mut rng = combined_seed;
        let mut next_rand = || {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((rng >> 32) as f32) / (u32::MAX as f32)
        };

        let velocity = Vector3::new(
            (next_rand() - 0.5) * 2.0,  // Random X velocity (-1 to 1)
            3.0 + next_rand() * 2.0,     // Pop upward (3-5)
            (next_rand() - 0.5) * 2.0,  // Random Z velocity (-1 to 1)
        );

        let rotation = next_rand() * 6.28;  // Random starting rotation

        Self {
            position,
            velocity,
            item,
            age: 0.0,
            pickup_delay: 0.5,  // 0.5 second delay before pickup
            rotation,
        }
    }

    /// Update physics and state
    pub fn update(&mut self, dt: f32, world: &crate::world::World) -> bool {
        self.age += dt;
        self.rotation += dt * 2.0; // 2 radians per second spin

        // Despawn after 5 minutes
        if self.age > 300.0 {
            return false;
        }

        // Decrease pickup delay
        if self.pickup_delay > 0.0 {
            self.pickup_delay -= dt;
        }

        // Apply gravity
        const GRAVITY: f32 = 20.0;
        self.velocity.y -= GRAVITY * dt;

        // Apply drag (air resistance)
        const DRAG: f32 = 0.98;
        self.velocity.x *= DRAG;
        self.velocity.z *= DRAG;

        // Update position
        let new_pos = Point3::new(
            self.position.x + self.velocity.x * dt,
            self.position.y + self.velocity.y * dt,
            self.position.z + self.velocity.z * dt,
        );

        // Ground collision (check block below)
        let ground_y = new_pos.y.floor() as i32;
        let ground_block = world.get_block(
            new_pos.x.floor() as i32,
            ground_y,
            new_pos.z.floor() as i32,
        );

        if ground_block.is_solid() && new_pos.y < (ground_y as f32 + 1.0) {
            // Hit ground, bounce with energy loss
            self.position.y = (ground_y as f32 + 1.0) + 0.125; // Item height offset
            self.velocity.y = -self.velocity.y * 0.3; // 30% bounce

            // Stop bouncing if velocity too low
            if self.velocity.y.abs() < 0.1 {
                self.velocity.y = 0.0;
            }

            // Apply ground friction
            self.velocity.x *= 0.8;
            self.velocity.z *= 0.8;
        } else {
            self.position = new_pos;
        }

        true // Keep alive
    }

    /// Check if this entity can be picked up
    pub fn can_pickup(&self) -> bool {
        self.pickup_delay <= 0.0
    }

    /// Check if player is within pickup range
    pub fn in_pickup_range(&self, player_pos: Point3<f32>) -> bool {
        let dx = self.position.x - player_pos.x;
        let dy = self.position.y - player_pos.y;
        let dz = self.position.z - player_pos.z;
        let dist_sq = dx * dx + dy * dy + dz * dz;
        const PICKUP_RANGE_SQ: f32 = 1.5 * 1.5;
        dist_sq < PICKUP_RANGE_SQ
    }
}
