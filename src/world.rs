use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::f32::consts::TAU;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::block::{Axis, BlockFace, BlockType};
use crate::chunk::{Chunk, CHUNK_HEIGHT, CHUNK_SIZE};
use crate::electric::{BlockPos3, ElectricalSystem};
use cgmath::Point3;
use noise::{NoiseFn, Perlin};
use rand::{rngs::SmallRng, Rng, SeedableRng};

/// Maximum fluid level in a single block (12 = full block)
pub const MAX_FLUID_LEVEL: u8 = 12;

/// Minimum fluid level required to act as a source that can spread to neighbors
/// Calculated as MAX_FLUID_LEVEL / 4, with a minimum of 1 to prevent zero division issues
/// For MAX_FLUID_LEVEL = 12, this is 3
pub const FLUID_MIN_SOURCE_LEVEL: u8 = if MAX_FLUID_LEVEL / 4 == 0 {
    1
} else {
    MAX_FLUID_LEVEL / 4
};

/// Fluid level considered to be at the surface (where it stops flowing down)
/// Calculated as the difference between MAX and MIN_SOURCE, clamped to MAX
/// For MAX_FLUID_LEVEL = 12 and FLUID_MIN_SOURCE_LEVEL = 3, this is 9
pub const FLUID_SURFACE_LEVEL: u8 = if MAX_FLUID_LEVEL > FLUID_MIN_SOURCE_LEVEL {
    MAX_FLUID_LEVEL - FLUID_MIN_SOURCE_LEVEL
} else {
    MAX_FLUID_LEVEL
};

/// Minimum level difference required to trigger lateral flow between cells
pub const FLUID_FLOW_THRESHOLD: i16 = 1;

/// Minimum amount of fluid that can flow in a single step
pub const FLUID_MIN_FLOW: u8 = 1;

/// Maximum amount of fluid that can flow laterally in a single step
/// Calculated as (MAX_FLUID_LEVEL + 2) / 3, with a minimum of 1
/// For MAX_FLUID_LEVEL = 12, this is 4
pub const FLUID_LATERAL_FLOW_CAP: u8 = if (MAX_FLUID_LEVEL + 2) / 3 == 0 {
    1
} else {
    (MAX_FLUID_LEVEL + 2) / 3
};
pub const WATER_LEVEL: i32 = 84;
const GLOBAL_TERRAIN_BASE: f64 = 156.0;
const MAX_WATER_FILL_DEPTH: i32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct AtmosphereSample {
    pub time_of_day: f32,
    pub sun_elevation: f32,
    pub daylight: f32,
    pub twilight: f32,
    pub sky_zenith: [f32; 3],
    pub sky_horizon: [f32; 3],
    pub fog_color: [f32; 3],
    pub fog_density: f32,
    pub ambient_strength: f32,
    pub vignette_strength: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BiomeTints {
    pub grass: [f32; 3],
    pub foliage: [f32; 3],
    pub water: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct WorldEnvironment {
    time_of_day: f32,
    day_length_seconds: f32,
}

impl WorldEnvironment {
    pub const DEFAULT_DAY_LENGTH: f32 = 720.0;

    pub fn new() -> Self {
        Self {
            time_of_day: 0.23,
            day_length_seconds: Self::DEFAULT_DAY_LENGTH,
        }
    }

    pub fn time_of_day(&self) -> f32 {
        self.time_of_day
    }

    pub fn set_time_of_day(&mut self, value: f32) {
        self.time_of_day = value.rem_euclid(1.0);
    }

    pub fn set_day_length(&mut self, seconds: f32) {
        if seconds > 1.0 {
            self.day_length_seconds = seconds;
        }
    }

    pub fn advance(&mut self, delta_seconds: f32) {
        if self.day_length_seconds <= 0.0 {
            return;
        }
        let delta = delta_seconds.max(0.0) / self.day_length_seconds;
        self.time_of_day = (self.time_of_day + delta).rem_euclid(1.0);
    }
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn clamp3(color: [f32; 3]) -> [f32; 3] {
    [clamp01(color[0]), clamp01(color[1]), clamp01(color[2])]
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
    ]
}

fn mul3(color: [f32; 3], scalar: f32) -> [f32; 3] {
    [color[0] * scalar, color[1] * scalar, color[2] * scalar]
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp01((x - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiomeType {
    Plains,
    Desert,
    Forest,
    Mountain,
    Swamp,
    Tundra,
    Jungle,
    Mesa,
    Savanna,
    Taiga,
    Meadow,
}

fn biome_fog_density(biome: BiomeType) -> f32 {
    match biome {
        BiomeType::Plains => 0.048,
        BiomeType::Desert => 0.028,
        BiomeType::Forest => 0.065,
        BiomeType::Mountain => 0.038,
        BiomeType::Swamp => 0.095,
        BiomeType::Tundra => 0.052,
        BiomeType::Jungle => 0.085,
        BiomeType::Mesa => 0.034,
        BiomeType::Savanna => 0.042,
        BiomeType::Taiga => 0.060,
        BiomeType::Meadow => 0.050,
    }
}

fn biome_fog_tint(biome: BiomeType) -> [f32; 3] {
    match biome {
        BiomeType::Plains => [0.64, 0.80, 0.92],
        BiomeType::Desert => [0.92, 0.82, 0.60],
        BiomeType::Forest => [0.46, 0.62, 0.52],
        BiomeType::Mountain => [0.72, 0.80, 0.90],
        BiomeType::Swamp => [0.32, 0.42, 0.34],
        BiomeType::Tundra => [0.82, 0.88, 0.94],
        BiomeType::Jungle => [0.38, 0.60, 0.46],
        BiomeType::Mesa => [0.84, 0.60, 0.50],
        BiomeType::Savanna => [0.78, 0.72, 0.55],
        BiomeType::Taiga => [0.56, 0.72, 0.78],
        BiomeType::Meadow => [0.66, 0.82, 0.90],
    }
}

fn base_biome_tints(biome: BiomeType) -> BiomeTints {
    match biome {
        BiomeType::Plains => BiomeTints {
            grass: [0.72, 0.86, 0.52],
            foliage: [0.58, 0.78, 0.42],
            water: [0.18, 0.42, 0.68],
        },
        BiomeType::Desert => BiomeTints {
            grass: [0.86, 0.78, 0.44],
            foliage: [0.78, 0.70, 0.38],
            water: [0.24, 0.48, 0.62],
        },
        BiomeType::Forest => BiomeTints {
            grass: [0.60, 0.82, 0.47],
            foliage: [0.48, 0.70, 0.36],
            water: [0.12, 0.36, 0.58],
        },
        BiomeType::Mountain => BiomeTints {
            grass: [0.78, 0.86, 0.74],
            foliage: [0.68, 0.78, 0.68],
            water: [0.16, 0.44, 0.70],
        },
        BiomeType::Swamp => BiomeTints {
            grass: [0.50, 0.64, 0.32],
            foliage: [0.42, 0.52, 0.30],
            water: [0.14, 0.28, 0.32],
        },
        BiomeType::Tundra => BiomeTints {
            grass: [0.80, 0.88, 0.78],
            foliage: [0.70, 0.80, 0.72],
            water: [0.28, 0.52, 0.70],
        },
        BiomeType::Jungle => BiomeTints {
            grass: [0.46, 0.78, 0.42],
            foliage: [0.34, 0.66, 0.32],
            water: [0.10, 0.38, 0.46],
        },
        BiomeType::Mesa => BiomeTints {
            grass: [0.82, 0.68, 0.42],
            foliage: [0.74, 0.58, 0.36],
            water: [0.22, 0.46, 0.58],
        },
        BiomeType::Savanna => BiomeTints {
            grass: [0.80, 0.76, 0.48],
            foliage: [0.68, 0.64, 0.40],
            water: [0.20, 0.44, 0.60],
        },
        BiomeType::Taiga => BiomeTints {
            grass: [0.66, 0.80, 0.60],
            foliage: [0.48, 0.68, 0.56],
            water: [0.14, 0.40, 0.62],
        },
        BiomeType::Meadow => BiomeTints {
            grass: [0.70, 0.86, 0.62],
            foliage: [0.58, 0.78, 0.54],
            water: [0.18, 0.44, 0.68],
        },
    }
}

#[derive(Clone, Copy)]
struct BiomeConfig {
    surface: BlockType,
    subsurface: BlockType,
    deep_block: BlockType,
    surface_depth: u8,
    tree_density_multiplier: f64,
    tree_height_range: (u8, u8),
    tree_canopy_radius: u8,
    tree_canopy_layers: u8,
    sky_color: [f32; 3],
    flower_density: f64,
    height_offset: f64,
    height_scale: f64,
    detail_amplitude: f64,
    continental_influence: f64,
    river_bed: BlockType,
}

#[derive(Clone, Copy)]
struct ColumnInfo {
    biome: BiomeType,
    config: BiomeConfig,
    height: i32,
    is_river: bool,
    river_strength: f64,
    river_bank: f64,
    altitude: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CaveBiome {
    GlowGrove,
    CrystalGarden,
    SubterraneanLake,
    BasaltChasm,
}

#[derive(Default, Clone)]
pub struct CaveChunkInfo {
    pub glow_emitters: Vec<Point3<i32>>,
    pub hazard_emitters: Vec<Point3<i32>>,
    pub loot_sites: Vec<Point3<i32>>,
}

impl CaveChunkInfo {
    pub fn is_empty(&self) -> bool {
        self.glow_emitters.is_empty()
            && self.hazard_emitters.is_empty()
            && self.loot_sites.is_empty()
    }
}

pub struct GeneratedChunk {
    pub chunk: Chunk,
    pub cave_info: CaveChunkInfo,
    pub has_fluid: bool,
}

#[derive(Clone)]
struct WorldGenContext {
    seed: u64,
    detail_noise: Perlin,
    temperature_noise: Perlin,
    moisture_noise: Perlin,
    elevation_noise: Perlin,
    continental_noise: Perlin,
    macro_biome_noise: Perlin,
    river_noise: Perlin,
    cave_noise: Perlin,
    cave_detail_noise: Perlin,
    cave_mask_noise: Perlin,
    cave_cluster_noise: Perlin,
    cave_biome_noise: Perlin,
    cave_humidity_noise: Perlin,
}

impl WorldGenContext {
    fn new(seed: u64) -> Self {
        Self {
            seed,
            detail_noise: Perlin::new(42),
            temperature_noise: Perlin::new(21),
            moisture_noise: Perlin::new(144),
            elevation_noise: Perlin::new(7),
            continental_noise: Perlin::new(313),
            macro_biome_noise: Perlin::new(487),
            river_noise: Perlin::new(999),
            cave_noise: Perlin::new(771),
            cave_detail_noise: Perlin::new(133),
            cave_mask_noise: Perlin::new(1973),
            cave_cluster_noise: Perlin::new(409),
            cave_biome_noise: Perlin::new(1801),
            cave_humidity_noise: Perlin::new(2903),
        }
    }

    fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
        if edge0 == edge1 {
            if x < edge0 {
                0.0
            } else {
                1.0
            }
        } else {
            let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        }
    }

    fn chunk_rng(&self, pos: ChunkPos) -> SmallRng {
        let mut hasher = DefaultHasher::new();
        pos.hash(&mut hasher);
        let chunk_hash = hasher.finish() ^ self.seed;
        SmallRng::seed_from_u64(chunk_hash)
    }

    fn sample_column(&self, x: i32, z: i32) -> ColumnInfo {
        let fx = x as f64;
        let fz = z as f64;

        let continental = self.continental_noise.get([fx * 0.0015, fz * 0.0015]);
        let continental_norm = (continental + 1.0) * 0.5;
        let continental_signed = continental_norm * 2.0 - 1.0;
        let macro_variation = self.macro_biome_noise.get([fx * 0.0025, fz * 0.0025]);

        let elevation = self.elevation_noise.get([fx * 0.0055, fz * 0.0055]);
        let elevation_norm = (elevation + 1.0) * 0.5;
        let altitude = (elevation_norm + (continental_norm - 0.5) * 0.35).clamp(0.0, 1.0);

        let temp_base = self.temperature_noise.get([fx * 0.0038, fz * 0.0038]);
        let moisture_base = self.moisture_noise.get([fx * 0.0032, fz * 0.0032]);
        let detail = self.detail_noise.get([fx * 0.045, fz * 0.045]);
        let river_val = self.river_noise.get([fx * 0.01, fz * 0.01]);
        let river_abs = river_val.abs();

        let temperature = ((temp_base + macro_variation * 0.35 + continental_signed * 0.28)
            .clamp(-1.0, 1.0)
            + 1.0)
            * 0.5;
        let moisture = ((moisture_base - macro_variation * 0.2 - continental_signed * 0.18)
            .clamp(-1.0, 1.0)
            + 1.0)
            * 0.5;

        let biome = self.select_biome(
            continental_norm,
            temperature.clamp(0.0, 1.0),
            moisture.clamp(0.0, 1.0),
            elevation_norm,
            macro_variation,
        );
        let config = self.biome_config(biome);

        let mut height_offset = config.height_offset;
        let mut height_scale = config.height_scale;
        let mut detail_amplitude = config.detail_amplitude;
        let mut continental_influence = config.continental_influence;

        if biome == BiomeType::Mountain {
            let slope = WorldGenContext::smoothstep(0.58, 0.8, altitude);
            let peak = WorldGenContext::smoothstep(0.75, 0.92, altitude);
            height_offset *= slope;
            height_scale *= slope * 0.7 + 0.3;
            detail_amplitude *= peak * 0.6 + 0.4;
            continental_influence *= slope * 0.65 + 0.35;
        } else if matches!(
            biome,
            BiomeType::Plains
                | BiomeType::Forest
                | BiomeType::Meadow
                | BiomeType::Savanna
                | BiomeType::Taiga
                | BiomeType::Tundra
        ) {
            let foothill = WorldGenContext::smoothstep(0.6, 0.78, altitude);
            if foothill > 0.0 {
                height_offset += 10.0 * foothill;
                height_scale += 16.0 * foothill * foothill;
                detail_amplitude += 2.0 * foothill;
            }
        }

        let elevation_variation = (elevation_norm - 0.5) * height_scale;

        let mut height_f = GLOBAL_TERRAIN_BASE
            + height_offset
            + continental_signed * continental_influence
            + elevation_variation
            + detail * detail_amplitude
            + macro_variation * detail_amplitude * 0.4;

        let channel_mask = (0.024 - river_abs).max(0.0) / 0.024;
        let bank_mask = (0.085 - river_abs).max(0.0) / 0.085;

        let river_bank = bank_mask.powf(1.2);
        let river_strength = channel_mask.powf(1.6);

        if river_bank > 0.0 {
            let target_bank = WATER_LEVEL as f64 + 2.5;
            height_f = height_f * (1.0 - river_bank) + target_bank * river_bank;
            height_f -= river_bank.powf(2.0) * 2.0;
        }

        let mut is_river = false;
        if river_strength > 0.0 {
            let channel_target = (WATER_LEVEL - 2) as f64;
            height_f = height_f * (1.0 - river_strength) + channel_target * river_strength;
            is_river = river_strength > 0.18;
        }

        let mut height = height_f.round() as i32;

        if is_river {
            let max_floor = (WATER_LEVEL - 1).clamp(6, CHUNK_HEIGHT as i32 - 5);
            let min_floor = (WATER_LEVEL - 3).clamp(6, max_floor);
            height = height.clamp(min_floor, max_floor);
        }

        height = height.clamp(6, CHUNK_HEIGHT as i32 - 5);

        ColumnInfo {
            biome,
            config,
            height,
            is_river,
            river_strength,
            river_bank,
            altitude,
        }
    }

    fn biome_config(&self, biome: BiomeType) -> BiomeConfig {
        match biome {
            BiomeType::Plains => BiomeConfig {
                surface: BlockType::Grass,
                subsurface: BlockType::Dirt,
                deep_block: BlockType::Stone,
                surface_depth: 3,
                tree_density_multiplier: 1.0,
                tree_height_range: (4, 5),
                tree_canopy_radius: 1,
                tree_canopy_layers: 3,
                sky_color: [0.52, 0.73, 0.86],
                flower_density: 0.12,
                height_offset: -10.0,
                height_scale: 26.0,
                detail_amplitude: 8.0,
                continental_influence: 24.0,
                river_bed: BlockType::Sand,
            },
            BiomeType::Desert => BiomeConfig {
                surface: BlockType::Sand,
                subsurface: BlockType::Sand,
                deep_block: BlockType::Stone,
                surface_depth: 4,
                tree_density_multiplier: 0.0,
                tree_height_range: (4, 4),
                tree_canopy_radius: 1,
                tree_canopy_layers: 2,
                sky_color: [0.82, 0.76, 0.58],
                flower_density: 0.0,
                height_offset: -18.0,
                height_scale: 24.0,
                detail_amplitude: 6.0,
                continental_influence: 26.0,
                river_bed: BlockType::Sand,
            },
            BiomeType::Forest => BiomeConfig {
                surface: BlockType::Grass,
                subsurface: BlockType::Dirt,
                deep_block: BlockType::Stone,
                surface_depth: 3,
                tree_density_multiplier: 2.2,
                tree_height_range: (5, 6),
                tree_canopy_radius: 1,
                tree_canopy_layers: 3,
                sky_color: [0.48, 0.68, 0.56],
                flower_density: 0.08,
                height_offset: 0.0,
                height_scale: 32.0,
                detail_amplitude: 10.0,
                continental_influence: 28.0,
                river_bed: BlockType::Dirt,
            },
            BiomeType::Mountain => BiomeConfig {
                surface: BlockType::Snow,
                subsurface: BlockType::Stone,
                deep_block: BlockType::Stone,
                surface_depth: 2,
                tree_density_multiplier: 0.2,
                tree_height_range: (4, 5),
                tree_canopy_radius: 1,
                tree_canopy_layers: 2,
                sky_color: [0.67, 0.75, 0.88],
                flower_density: 0.0,
                height_offset: 40.0,
                height_scale: 80.0,
                detail_amplitude: 18.0,
                continental_influence: 64.0,
                river_bed: BlockType::Stone,
            },
            BiomeType::Swamp => BiomeConfig {
                surface: BlockType::Grass,
                subsurface: BlockType::Dirt,
                deep_block: BlockType::Stone,
                surface_depth: 2,
                tree_density_multiplier: 1.6,
                tree_height_range: (4, 5),
                tree_canopy_radius: 1,
                tree_canopy_layers: 3,
                sky_color: [0.36, 0.46, 0.40],
                flower_density: 0.02,
                height_offset: -26.0,
                height_scale: 16.0,
                detail_amplitude: 4.0,
                continental_influence: 14.0,
                river_bed: BlockType::Dirt,
            },
            BiomeType::Tundra => BiomeConfig {
                surface: BlockType::Snow,
                subsurface: BlockType::Dirt,
                deep_block: BlockType::Stone,
                surface_depth: 2,
                tree_density_multiplier: 0.4,
                tree_height_range: (4, 5),
                tree_canopy_radius: 1,
                tree_canopy_layers: 3,
                sky_color: [0.74, 0.81, 0.88],
                flower_density: 0.0,
                height_offset: 24.0,
                height_scale: 56.0,
                detail_amplitude: 12.0,
                continental_influence: 36.0,
                river_bed: BlockType::Stone,
            },
            BiomeType::Jungle => BiomeConfig {
                surface: BlockType::Grass,
                subsurface: BlockType::Dirt,
                deep_block: BlockType::Stone,
                surface_depth: 4,
                tree_density_multiplier: 3.4,
                tree_height_range: (6, 9),
                tree_canopy_radius: 2,
                tree_canopy_layers: 4,
                sky_color: [0.43, 0.70, 0.52],
                flower_density: 0.14,
                height_offset: 12.0,
                height_scale: 36.0,
                detail_amplitude: 12.0,
                continental_influence: 26.0,
                river_bed: BlockType::Dirt,
            },
            BiomeType::Mesa => BiomeConfig {
                surface: BlockType::Terracotta,
                subsurface: BlockType::Terracotta,
                deep_block: BlockType::Stone,
                surface_depth: 5,
                tree_density_multiplier: 0.05,
                tree_height_range: (4, 4),
                tree_canopy_radius: 1,
                tree_canopy_layers: 2,
                sky_color: [0.82, 0.58, 0.42],
                flower_density: 0.01,
                height_offset: 28.0,
                height_scale: 48.0,
                detail_amplitude: 10.0,
                continental_influence: 38.0,
                river_bed: BlockType::Sand,
            },
            BiomeType::Savanna => BiomeConfig {
                surface: BlockType::Grass,
                subsurface: BlockType::Dirt,
                deep_block: BlockType::Stone,
                surface_depth: 3,
                tree_density_multiplier: 0.6,
                tree_height_range: (5, 6),
                tree_canopy_radius: 1,
                tree_canopy_layers: 2,
                sky_color: [0.78, 0.72, 0.54],
                flower_density: 0.02,
                height_offset: -4.0,
                height_scale: 22.0,
                detail_amplitude: 7.0,
                continental_influence: 22.0,
                river_bed: BlockType::Sand,
            },
            BiomeType::Taiga => BiomeConfig {
                surface: BlockType::Snow,
                subsurface: BlockType::Dirt,
                deep_block: BlockType::Stone,
                surface_depth: 3,
                tree_density_multiplier: 1.4,
                tree_height_range: (6, 7),
                tree_canopy_radius: 1,
                tree_canopy_layers: 3,
                sky_color: [0.58, 0.74, 0.78],
                flower_density: 0.0,
                height_offset: 12.0,
                height_scale: 40.0,
                detail_amplitude: 10.0,
                continental_influence: 32.0,
                river_bed: BlockType::Stone,
            },
            BiomeType::Meadow => BiomeConfig {
                surface: BlockType::Grass,
                subsurface: BlockType::Dirt,
                deep_block: BlockType::Stone,
                surface_depth: 3,
                tree_density_multiplier: 0.3,
                tree_height_range: (4, 5),
                tree_canopy_radius: 1,
                tree_canopy_layers: 2,
                sky_color: [0.62, 0.78, 0.72],
                flower_density: 0.18,
                height_offset: 18.0,
                height_scale: 34.0,
                detail_amplitude: 9.0,
                continental_influence: 30.0,
                river_bed: BlockType::Sand,
            },
        }
    }

    fn select_biome(
        &self,
        continental: f64,
        temperature: f64,
        moisture: f64,
        elevation: f64,
        macro_variation: f64,
    ) -> BiomeType {
        let altitude = (elevation + (continental - 0.5) * 0.35).clamp(0.0, 1.0);
        let temp = temperature;
        let wet = moisture;

        if altitude > 0.82 {
            if temp < 0.45 {
                return BiomeType::Tundra;
            }
            return if wet > 0.5 {
                BiomeType::Meadow
            } else {
                BiomeType::Mountain
            };
        }

        if continental < 0.38 && wet > 0.52 && altitude < 0.45 {
            return BiomeType::Swamp;
        }

        if temp < 0.28 {
            return if wet > 0.5 {
                BiomeType::Taiga
            } else {
                BiomeType::Tundra
            };
        }

        if wet < 0.18 {
            return if altitude > 0.55 {
                BiomeType::Mesa
            } else {
                BiomeType::Desert
            };
        }

        if altitude > 0.7 {
            return if temp < 0.5 {
                BiomeType::Taiga
            } else if wet > 0.5 {
                BiomeType::Meadow
            } else {
                BiomeType::Mountain
            };
        }

        if temp > 0.78 && wet < 0.33 {
            return if macro_variation > 0.25 {
                BiomeType::Mesa
            } else {
                BiomeType::Savanna
            };
        }

        if wet > 0.82 && temp > 0.6 {
            return BiomeType::Jungle;
        }

        if temp > 0.68 && wet < 0.4 {
            return BiomeType::Savanna;
        }

        if wet > 0.7 {
            return if altitude > 0.55 {
                BiomeType::Meadow
            } else {
                BiomeType::Forest
            };
        }

        if macro_variation < -0.35 && wet > 0.45 && altitude < 0.5 {
            return BiomeType::Swamp;
        }

        if temp > 0.6 && wet > 0.55 {
            return BiomeType::Forest;
        }

        if altitude > 0.6 {
            return if wet > 0.45 {
                BiomeType::Meadow
            } else {
                BiomeType::Mountain
            };
        }

        if wet < 0.3 {
            return BiomeType::Plains;
        }

        BiomeType::Plains
    }
}

pub struct World {
    chunks: HashMap<ChunkPos, Chunk>,
    active_fluid_chunks: HashSet<ChunkPos>,
    cave_chunk_info: HashMap<ChunkPos, CaveChunkInfo>,
    gen: Arc<WorldGenContext>,
    electrical: ElectricalSystem,
    environment: WorldEnvironment,
}

impl World {
    pub fn chunks(&self) -> &HashMap<ChunkPos, Chunk> {
        &self.chunks
    }

    pub fn electrical(&self) -> &ElectricalSystem {
        &self.electrical
    }

    pub fn electrical_mut(&mut self) -> &mut ElectricalSystem {
        &mut self.electrical
    }

    pub fn environment(&self) -> &WorldEnvironment {
        &self.environment
    }

    pub fn environment_mut(&mut self) -> &mut WorldEnvironment {
        &mut self.environment
    }

    pub fn advance_time(&mut self, delta_seconds: f32) {
        self.environment.advance(delta_seconds);
    }

    pub fn tick_electrical(&mut self) {
        self.electrical.tick();
    }

    pub fn chunks_mut(&mut self) -> &mut HashMap<ChunkPos, Chunk> {
        &mut self.chunks
    }

    pub fn active_fluid_chunks_snapshot(&self) -> Vec<ChunkPos> {
        self.active_fluid_chunks.iter().copied().collect()
    }

    pub fn queue_fluid_chunk(&mut self, pos: ChunkPos) {
        self.active_fluid_chunks.insert(pos);
    }

    pub fn queue_fluid_chunk_with_neighbors(&mut self, pos: ChunkPos) {
        self.queue_fluid_chunk(pos);
        self.queue_loaded_neighbors(pos);
    }

    pub fn step_fluids(&mut self) -> bool {
        // CPU-based fluid simulation fallback
        // This is a simple cellular automaton approach for water flow

        let active_chunks: Vec<ChunkPos> = self.active_fluid_chunks.iter().copied().collect();
        if active_chunks.is_empty() {
            return false;
        }

        let mut any_changed = false;

        // Process each active chunk
        for chunk_pos in active_chunks {
            let mut updates: Vec<(usize, usize, usize, u8)> = Vec::new();

            if let Some(chunk) = self.chunks.get(&chunk_pos) {
                // Collect all fluid positions and their amounts
                let fluid_cells: Vec<(usize, usize, usize, u8)> =
                    chunk.fluids_iter().collect();

                for (x, y, z, amount) in fluid_cells {
                    if amount == 0 {
                        continue;
                    }

                    let world_x = chunk_pos.x * CHUNK_SIZE as i32 + x as i32;
                    let world_y = y as i32;
                    let world_z = chunk_pos.z * CHUNK_SIZE as i32 + z as i32;

                    // Check if block below is air or has room for fluid
                    if world_y > 0 {
                        let below_block = self.get_block(world_x, world_y - 1, world_z);
                        let below_fluid = self.get_fluid_amount(world_x, world_y - 1, world_z);

                        if !below_block.is_solid() && below_fluid < MAX_FLUID_LEVEL {
                            // Flow downward (gravity)
                            let flow_amount = amount.min(MAX_FLUID_LEVEL - below_fluid).min(FLUID_MIN_FLOW * 3);
                            if flow_amount > 0 {
                                updates.push((x, y, z, amount.saturating_sub(flow_amount)));
                                let new_below = (below_fluid as u16 + flow_amount as u16).min(MAX_FLUID_LEVEL as u16) as u8;
                                self.set_fluid_amount(world_x, world_y - 1, world_z, new_below);
                                any_changed = true;
                                continue; // Prioritize downward flow
                            }
                        }
                    }

                    // If can't flow down and has enough fluid, try lateral flow
                    if amount > FLUID_MIN_SOURCE_LEVEL {
                        let neighbors = [
                            (world_x + 1, world_y, world_z),
                            (world_x - 1, world_y, world_z),
                            (world_x, world_y, world_z + 1),
                            (world_x, world_y, world_z - 1),
                        ];

                        let mut total_flow = 0u8;
                        for &(nx, ny, nz) in &neighbors {
                            let neighbor_block = self.get_block(nx, ny, nz);
                            let neighbor_fluid = self.get_fluid_amount(nx, ny, nz);

                            if !neighbor_block.is_solid() {
                                let level_diff = amount.saturating_sub(neighbor_fluid);
                                if level_diff > FLUID_FLOW_THRESHOLD as u8 {
                                    let flow = (level_diff / 4).max(FLUID_MIN_FLOW).min(FLUID_LATERAL_FLOW_CAP);
                                    let actual_flow = flow.min(amount.saturating_sub(total_flow));

                                    if actual_flow > 0 {
                                        total_flow = total_flow.saturating_add(actual_flow);
                                        let new_neighbor = (neighbor_fluid as u16 + actual_flow as u16).min(MAX_FLUID_LEVEL as u16) as u8;
                                        self.set_fluid_amount(nx, ny, nz, new_neighbor);
                                        any_changed = true;
                                    }
                                }
                            }
                        }

                        if total_flow > 0 {
                            updates.push((x, y, z, amount.saturating_sub(total_flow)));
                        }
                    }

                    // Evaporate very small amounts
                    if amount <= FLUID_MIN_FLOW && y > 0 {
                        let below_block = self.get_block(world_x, world_y - 1, world_z);
                        if !below_block.is_solid() || self.get_fluid_amount(world_x, world_y - 1, world_z) == 0 {
                            updates.push((x, y, z, 0));
                            any_changed = true;
                        }
                    }
                }
            }

            // Apply updates to this chunk
            if let Some(chunk) = self.chunks.get_mut(&chunk_pos) {
                for (x, y, z, new_amount) in updates {
                    chunk.set_fluid(x, y, z, new_amount);
                }
            }
        }

        any_changed
    }

    pub fn finalize_fluid_chunk_state(&mut self, pos: ChunkPos, changed: bool, has_fluid: bool) {
        if changed {
            // Chunk has active fluid simulation - keep it in the active set and queue neighbors
            self.active_fluid_chunks.insert(pos);
            self.queue_loaded_neighbors(pos);
        } else {
            // Chunk is stable (no changes this tick) - remove from active simulation
            // This includes both chunks with no fluid and chunks with static fluid
            // Static fluid chunks will be re-added when a neighbor changes
            self.active_fluid_chunks.remove(&pos);
        }
    }

    fn queue_loaded_neighbors(&mut self, pos: ChunkPos) {
        const NEIGHBOR_OFFSETS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        for (dx, dz) in NEIGHBOR_OFFSETS {
            let neighbor = ChunkPos {
                x: pos.x + dx,
                z: pos.z + dz,
            };
            if self.chunks.contains_key(&neighbor) {
                self.active_fluid_chunks.insert(neighbor);
            }
        }
    }

    pub fn biome_at(&self, x: i32, z: i32) -> BiomeType {
        self.gen.sample_column(x, z).biome
    }

    pub fn atmosphere_at(&self, x: i32, z: i32) -> AtmosphereSample {
        let column = self.gen.sample_column(x, z);
        self.atmosphere_from_column(&column)
    }

    pub fn biome_tints_at(&self, x: i32, z: i32) -> BiomeTints {
        let column = self.gen.sample_column(x, z);
        let atmosphere = self.atmosphere_from_column(&column);
        self.biome_tints_from_column(&column, &atmosphere)
    }

    pub fn sky_color_at(&self, x: i32, z: i32) -> [f32; 3] {
        let column = self.gen.sample_column(x, z);
        let atmosphere = self.atmosphere_from_column(&column);
        lerp3(atmosphere.sky_horizon, atmosphere.sky_zenith, 0.65)
    }

    fn atmosphere_from_column(&self, column: &ColumnInfo) -> AtmosphereSample {
        let time = self.environment.time_of_day();
        let sun_phase = time * TAU;
        let sun_elevation = sun_phase.sin();
        let daylight = clamp01(sun_elevation * 0.5 + 0.5);
        let twilight = clamp01(1.0 - sun_elevation.abs() * 6.0);

        let base_sky = column.config.sky_color;
        let zenith_day = lerp3(
            [0.30, 0.52, 0.92],
            lerp3(base_sky, [0.80, 0.90, 1.0], 0.35),
            0.55,
        );
        let horizon_day = lerp3(base_sky, [1.0, 0.86, 0.62], 0.35);
        let zenith_night = [0.02, 0.04, 0.10];
        let horizon_night = [0.05, 0.07, 0.14];

        let dusk_weight = smoothstep(0.48, 0.55, time);
        let dawn_weight = 1.0 - dusk_weight;
        let dawn_glow = [1.0, 0.62, 0.36];
        let dusk_glow = [0.98, 0.46, 0.28];
        let twilight_color = [
            dawn_glow[0] * dawn_weight + dusk_glow[0] * dusk_weight,
            dawn_glow[1] * dawn_weight + dusk_glow[1] * dusk_weight,
            dawn_glow[2] * dawn_weight + dusk_glow[2] * dusk_weight,
        ];

        let mut sky_zenith = lerp3(zenith_night, zenith_day, daylight);
        let mut sky_horizon = lerp3(horizon_night, horizon_day, daylight);

        sky_zenith = lerp3(sky_zenith, twilight_color, twilight * 0.35);
        sky_horizon = lerp3(sky_horizon, twilight_color, twilight * 0.8);

        sky_zenith = clamp3(sky_zenith);
        sky_horizon = clamp3(sky_horizon);

        let biome_fog = biome_fog_tint(column.biome);
        let mut fog_color = lerp3(biome_fog, sky_horizon, 0.5 + daylight * 0.35);
        fog_color = lerp3(fog_color, twilight_color, twilight * 0.5);
        let fog_color = clamp3(fog_color);

        let mut fog_density = biome_fog_density(column.biome);
        let altitude = column.altitude as f32;
        fog_density *= lerp(0.6, 1.1, clamp01(1.0 - altitude));
        fog_density *= 0.7 + (1.0 - daylight) * 0.6 + twilight * 0.3;
        fog_density = fog_density.clamp(0.02, 0.15);

        let ambient_strength = clamp01(lerp(0.18, 0.72, daylight) + twilight * 0.08);
        let vignette_strength = clamp01(lerp(0.18, 0.42, 1.0 - daylight) + twilight * 0.1);

        AtmosphereSample {
            time_of_day: time,
            sun_elevation,
            daylight,
            twilight,
            sky_zenith,
            sky_horizon,
            fog_color,
            fog_density,
            ambient_strength,
            vignette_strength,
        }
    }

    fn biome_tints_from_column(
        &self,
        column: &ColumnInfo,
        atmosphere: &AtmosphereSample,
    ) -> BiomeTints {
        let mut tints = base_biome_tints(column.biome);
        let daylight = atmosphere.daylight;
        let twilight = atmosphere.twilight;

        let mut grass = tints.grass;
        grass = lerp3(grass, mul3(grass, 1.2), daylight * 0.35);
        grass = lerp3(grass, mul3(grass, 0.85), (1.0 - daylight) * 0.25);
        grass = clamp3(grass);

        let mut foliage = tints.foliage;
        foliage = lerp3(foliage, mul3(foliage, 1.18), daylight * 0.25);
        foliage = lerp3(foliage, mul3(foliage, 0.9), twilight * 0.2);
        foliage = clamp3(foliage);

        let mut water = tints.water;
        water = lerp3(
            water,
            atmosphere.fog_color,
            clamp01((1.0 - daylight) * 0.25 + twilight * 0.2),
        );
        if column.is_river {
            water = lerp3(
                water,
                [0.16, 0.38, 0.62],
                clamp01(column.river_strength as f32),
            );
        }
        water = clamp3(water);

        BiomeTints {
            grass,
            foliage,
            water,
        }
    }

    pub fn new() -> Self {
        let seed = 0x5A5A_1234u64;
        let gen = Arc::new(WorldGenContext::new(seed));
        Self {
            chunks: HashMap::new(),
            active_fluid_chunks: HashSet::new(),
            cave_chunk_info: HashMap::new(),
            gen,
            electrical: ElectricalSystem::new(),
            environment: WorldEnvironment::new(),
        }
    }

    pub fn update_loaded_chunks(&mut self, camera_pos: Point3<f32>, render_distance: i32) -> bool {
        let player_chunk_x = (camera_pos.x / CHUNK_SIZE as f32).floor() as i32;
        let player_chunk_z = (camera_pos.z / CHUNK_SIZE as f32).floor() as i32;
        let mut changed = false;

        for cz in (player_chunk_z - render_distance)..=(player_chunk_z + render_distance) {
            for cx in (player_chunk_x - render_distance)..=(player_chunk_x + render_distance) {
                let pos = ChunkPos { x: cx, z: cz };
                if !self.chunks.contains_key(&pos) {
                    let chunk = self.generate_chunk(pos);
                    let has_fluid = chunk.fluids_iter().next().is_some();
                    self.chunks.insert(pos, chunk);
                    if has_fluid {
                        self.queue_fluid_chunk(pos);
                    }
                    changed = true;
                }
            }
        }

        let unload_distance = render_distance + 2;
        self.chunks.retain(|pos, _| {
            let dx = (pos.x - player_chunk_x).abs();
            let dz = (pos.z - player_chunk_z).abs();
            let keep = dx <= unload_distance && dz <= unload_distance;
            if !keep {
                changed = true;
                self.active_fluid_chunks.remove(pos);
            }
            keep
        });

        changed
    }

    fn generate_chunk(&self, pos: ChunkPos) -> Chunk {
        let mut chunk = Chunk::new();
        let mut rng = self.gen.chunk_rng(pos);

        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let world_x = pos.x * CHUNK_SIZE as i32 + x as i32;
                let world_z = pos.z * CHUNK_SIZE as i32 + z as i32;
                let column = self.gen.sample_column(world_x, world_z);
                let biome_cfg = column.config;
                let height = column.height;
                for y in 0..CHUNK_HEIGHT {
                    let world_y = y as i32;
                    let mut block_type = if world_y > height {
                        BlockType::Air
                    } else {
                        self.block_for_column(&column, height, world_y)
                    };

                    if block_type == BlockType::Stone {
                        block_type = self.sample_subsurface_block(&mut rng, world_y);
                    }

                    if block_type != BlockType::Air {
                        if self.should_carve_cave(world_x, world_y, world_z, &column) {
                            block_type = BlockType::Air;
                        }
                    }

                    if block_type != BlockType::Air {
                        chunk.set_block(x, y, z, block_type);
                    }
                }

                if biome_cfg.tree_density_multiplier > 0.0 {
                    let canopy_radius = biome_cfg.tree_canopy_radius as usize;
                    let canopy_layers = biome_cfg.tree_canopy_layers.max(1) as usize;
                    let (min_height, max_height) = biome_cfg.tree_height_range;
                    let trunk_height = if max_height <= min_height {
                        min_height as usize
                    } else {
                        rng.gen_range(min_height..=max_height) as usize
                    };

                    if let Some(ground_y) = self.should_spawn_tree(
                        &mut rng,
                        &chunk,
                        x,
                        z,
                        height,
                        biome_cfg.tree_density_multiplier,
                        canopy_radius,
                    ) {
                        let base_y = ground_y + 1;
                        if self.can_place_tree(
                            &chunk,
                            x,
                            base_y,
                            z,
                            trunk_height,
                            canopy_radius,
                            canopy_layers,
                        ) {
                            self.grow_tree(
                                &mut chunk,
                                x,
                                base_y,
                                z,
                                trunk_height,
                                canopy_radius,
                                canopy_layers,
                            );
                        }
                    }
                }

                if biome_cfg.flower_density > 0.0 && height >= 0 {
                    let ground_y = height.min((CHUNK_HEIGHT - 1) as i32) as usize;
                    let flower_y = height + 1;
                    if flower_y >= 0 && flower_y < CHUNK_HEIGHT as i32 {
                        let flower_usize = flower_y as usize;
                        if chunk.get_block(x, ground_y, z) == biome_cfg.surface
                            && chunk.get_block(x, flower_usize, z) == BlockType::Air
                            && rng.gen_bool(biome_cfg.flower_density)
                        {
                            let flower = self.random_flower(&mut rng);
                            chunk.set_block(x, flower_usize, z, flower);
                        }
                    }
                }

                let ground_index = height.clamp(0, CHUNK_HEIGHT as i32 - 1) as usize;
                let has_ground = chunk.get_block(x, ground_index, z).is_solid();
                let bed_index = (height - 1).clamp(0, CHUNK_HEIGHT as i32 - 1) as usize;
                let has_bed = chunk.get_block(x, bed_index, z).is_solid();

                if column.river_bank > 0.18 && has_ground && height <= WATER_LEVEL + 6 {
                    let layers = ((column.river_bank * 3.0).ceil() as i32).max(0);
                    for step in 0..=layers {
                        let carve_y = (height - step).clamp(0, CHUNK_HEIGHT as i32 - 1) as usize;
                        if chunk.get_block(x, carve_y, z).is_solid() {
                            chunk.set_block(x, carve_y, z, biome_cfg.river_bed);
                        }
                    }
                }

                if column.is_river {
                    if height > WATER_LEVEL + 6 {
                        continue;
                    }
                    if has_ground && has_bed {
                        let fluid_y = (height + 1).clamp(0, CHUNK_HEIGHT as i32 - 1) as usize;
                        let mut target_level =
                            (MAX_FLUID_LEVEL as f64 * column.river_strength.clamp(0.35, 1.0)) as u8;
                        target_level = target_level
                            .max(FLUID_MIN_SOURCE_LEVEL)
                            .min(MAX_FLUID_LEVEL);
                        chunk.set_fluid(x, fluid_y, z, target_level);
                    }
                } else if column.river_bank > 0.55
                    && has_ground
                    && has_bed
                    && height <= WATER_LEVEL + 2
                {
                    let fluid_y = (height + 1).clamp(0, CHUNK_HEIGHT as i32 - 1) as usize;
                    let fill_strength =
                        (column.river_strength * 0.6 + column.river_bank * 0.55).clamp(0.25, 0.75);
                    let mut level = (MAX_FLUID_LEVEL as f64 * fill_strength) as u8;
                    level = level.max(FLUID_MIN_SOURCE_LEVEL).min(FLUID_SURFACE_LEVEL);
                    chunk.set_fluid(x, fluid_y, z, level);
                } else if column.biome == BiomeType::Swamp
                    && has_ground
                    && has_bed
                    && height < WATER_LEVEL - 1
                    && height >= WATER_LEVEL - MAX_WATER_FILL_DEPTH
                {
                    let fluid_y = (height + 1).clamp(0, CHUNK_HEIGHT as i32 - 1) as usize;
                    chunk.set_fluid(x, fluid_y, z, FLUID_SURFACE_LEVEL);
                }

                if column.is_river && column.river_strength > 0.5 {
                    if height > WATER_LEVEL + 6 {
                        continue;
                    }
                    let width = if column.river_strength > 0.9 { 2 } else { 1 };
                    for dx in -width..=width {
                        for dz in -width..=width {
                            if dx == 0 && dz == 0 {
                                continue;
                            }
                            let nx = x as i32 + dx;
                            let nz = z as i32 + dz;
                            if nx >= 0
                                && nx < CHUNK_SIZE as i32
                                && nz >= 0
                                && nz < CHUNK_SIZE as i32
                            {
                                let nxu = nx as usize;
                                let nzu = nz as usize;
                                let carve_y =
                                    (height - 1).clamp(0, CHUNK_HEIGHT as i32 - 1) as usize;
                                chunk.set_block(nxu, carve_y, nzu, biome_cfg.river_bed);
                                let bed_check = carve_y.saturating_sub(1);
                                if !chunk.get_block(nxu, bed_check, nzu).is_solid() {
                                    continue;
                                }
                                let fluid_y =
                                    (height + 1).clamp(0, CHUNK_HEIGHT as i32 - 1) as usize;
                                let mut target_level = (MAX_FLUID_LEVEL as f64
                                    * column.river_strength.clamp(0.35, 1.0))
                                    as u8;
                                target_level = target_level
                                    .max(FLUID_MIN_SOURCE_LEVEL)
                                    .min(MAX_FLUID_LEVEL);
                                chunk.set_fluid(nxu, fluid_y, nzu, target_level);
                            }
                        }
                    }
                }

                if column.biome == BiomeType::Swamp {
                    let mut target_fluid_y = None;
                    if column.is_river {
                        target_fluid_y = Some((height + 1).clamp(0, CHUNK_HEIGHT as i32 - 1));
                    } else if height <= WATER_LEVEL {
                        target_fluid_y = Some(WATER_LEVEL.clamp(0, CHUNK_HEIGHT as i32 - 1));
                    }

                    if let Some(fluid_y) = target_fluid_y {
                        let fluid_y_usize = fluid_y as usize;
                        if chunk.get_fluid(x, fluid_y_usize, z) > 0 {
                            let lily_y = fluid_y_usize + 1;
                            if lily_y < CHUNK_HEIGHT
                                && chunk.get_block(x, lily_y, z) == BlockType::Air
                                && rng.gen_bool(0.15)
                            {
                                chunk.set_block(x, lily_y, z, BlockType::LilyPad);
                            }
                        }
                    }
                }
            }
        }

        chunk
    }

    pub fn get_block(&self, x: i32, y: i32, z: i32) -> BlockType {
        if y < 0 || y >= CHUNK_HEIGHT as i32 {
            return BlockType::Air;
        }

        let chunk_x = x.div_euclid(CHUNK_SIZE as i32);
        let chunk_z = z.div_euclid(CHUNK_SIZE as i32);
        let local_x = x.rem_euclid(CHUNK_SIZE as i32) as usize;
        let local_y = y as usize;
        let local_z = z.rem_euclid(CHUNK_SIZE as i32) as usize;

        let pos = ChunkPos {
            x: chunk_x,
            z: chunk_z,
        };

        // Optimize: do both block and fluid lookup in single chunk access
        if let Some(chunk) = self.chunks.get(&pos) {
            let block = chunk.get_block(local_x, local_y, local_z);
            if matches!(block, BlockType::Air) {
                // Check if there's fluid without doing another chunk lookup
                if chunk.get_fluid(local_x, local_y, local_z) > 0 {
                    BlockType::Water
                } else {
                    BlockType::Air
                }
            } else {
                block
            }
        } else {
            BlockType::Air
        }
    }

    pub fn set_block(&mut self, x: i32, y: i32, z: i32, block_type: BlockType) {
        self.set_block_with_axis(x, y, z, block_type, None, None);
    }

    pub fn set_block_with_axis(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        block_type: BlockType,
        axis: Option<Axis>,
        face: Option<BlockFace>,
    ) {
        if y < 0 || y >= CHUNK_HEIGHT as i32 {
            return;
        }

        let chunk_x = x.div_euclid(CHUNK_SIZE as i32);
        let chunk_z = z.div_euclid(CHUNK_SIZE as i32);
        let local_x = x.rem_euclid(CHUNK_SIZE as i32) as usize;
        let local_y = y as usize;
        let local_z = z.rem_euclid(CHUNK_SIZE as i32) as usize;

        let pos = ChunkPos {
            x: chunk_x,
            z: chunk_z,
        };

        if !self.chunks.contains_key(&pos) {
            self.chunks.insert(pos, self.generate_chunk(pos));
        }

        let world_pos = BlockPos3::new(x, y, z);
        let is_electrical = block_type.is_electrical();

        if let Some(chunk) = self.chunks.get_mut(&pos) {
            if !is_electrical {
                chunk.set_block(local_x, local_y, local_z, block_type);
                if block_type != BlockType::Air {
                    chunk.set_fluid(local_x, local_y, local_z, 0);
                }
            }
        }

        self.electrical.update_block_with(
            pos,
            (local_x, local_y, local_z),
            block_type,
            axis,
            face,
            None,
        );
        if is_electrical || block_type == BlockType::Air {
            self.refresh_electrical_block(world_pos);
        }
        self.queue_fluid_chunk_with_neighbors(pos);
    }

    fn refresh_electrical_block(&mut self, world_pos: BlockPos3) {
        let chunk_x = world_pos.x.div_euclid(CHUNK_SIZE as i32);
        let chunk_z = world_pos.z.div_euclid(CHUNK_SIZE as i32);
        let local_x = world_pos.x.rem_euclid(CHUNK_SIZE as i32) as usize;
        let local_y = world_pos.y as usize;
        let local_z = world_pos.z.rem_euclid(CHUNK_SIZE as i32) as usize;
        let pos = ChunkPos {
            x: chunk_x,
            z: chunk_z,
        };

        if !self.chunks.contains_key(&pos) {
            return;
        }

        if let Some(chunk) = self.chunks.get_mut(&pos) {
            let existing = chunk.get_block(local_x, local_y, local_z);
            let attachment = self.electrical.face_nodes(world_pos).and_then(|faces| {
                faces
                    .iter()
                    .next()
                    .map(|(_, node)| node.component.block_type())
            });

            match attachment {
                Some(block_type) if existing == BlockType::Air || existing.is_electrical() => {
                    chunk.set_block(local_x, local_y, local_z, block_type);
                    chunk.set_fluid(local_x, local_y, local_z, 0);
                }
                None if existing.is_electrical() => {
                    chunk.set_block(local_x, local_y, local_z, BlockType::Air);
                    chunk.set_fluid(local_x, local_y, local_z, 0);
                }
                _ => {}
            }
        }
    }

    pub fn remove_electrical_face(&mut self, x: i32, y: i32, z: i32, face: BlockFace) -> bool {
        if y < 0 || y >= CHUNK_HEIGHT as i32 {
            return false;
        }

        let chunk_x = x.div_euclid(CHUNK_SIZE as i32);
        let chunk_z = z.div_euclid(CHUNK_SIZE as i32);
        let pos = ChunkPos {
            x: chunk_x,
            z: chunk_z,
        };
        if !self.chunks.contains_key(&pos) {
            return false;
        }

        let world_pos = BlockPos3::new(x, y, z);
        if !self.electrical.remove_component(world_pos, face) {
            return false;
        }

        self.refresh_electrical_block(world_pos);
        self.queue_fluid_chunk_with_neighbors(pos);
        true
    }

    pub fn get_fluid_amount(&self, x: i32, y: i32, z: i32) -> u8 {
        if y < 0 || y >= CHUNK_HEIGHT as i32 {
            return 0;
        }

        let chunk_x = x.div_euclid(CHUNK_SIZE as i32);
        let chunk_z = z.div_euclid(CHUNK_SIZE as i32);
        let local_x = x.rem_euclid(CHUNK_SIZE as i32) as usize;
        let local_y = y as usize;
        let local_z = z.rem_euclid(CHUNK_SIZE as i32) as usize;

        self.chunks
            .get(&ChunkPos {
                x: chunk_x,
                z: chunk_z,
            })
            .map(|chunk| chunk.get_fluid(local_x, local_y, local_z))
            .unwrap_or(0)
    }

    pub fn set_fluid_amount(&mut self, x: i32, y: i32, z: i32, amount: u8) {
        if y < 0 || y >= CHUNK_HEIGHT as i32 {
            return;
        }

        let chunk_x = x.div_euclid(CHUNK_SIZE as i32);
        let chunk_z = z.div_euclid(CHUNK_SIZE as i32);
        let local_x = x.rem_euclid(CHUNK_SIZE as i32) as usize;
        let local_y = y as usize;
        let local_z = z.rem_euclid(CHUNK_SIZE as i32) as usize;

        let pos = ChunkPos {
            x: chunk_x,
            z: chunk_z,
        };
        if !self.chunks.contains_key(&pos) {
            self.chunks.insert(pos, self.generate_chunk(pos));
        }

        if let Some(chunk) = self.chunks.get_mut(&pos) {
            chunk.set_fluid(local_x, local_y, local_z, amount.min(MAX_FLUID_LEVEL));
        }

        self.queue_fluid_chunk_with_neighbors(pos);
    }

    pub fn add_fluid(&mut self, x: i32, y: i32, z: i32, amount: u8) {
        let current = self.get_fluid_amount(x, y, z);
        let new_amount = (current as u16 + amount as u16).min(MAX_FLUID_LEVEL as u16) as u8;
        self.set_fluid_amount(x, y, z, new_amount);
    }

    fn sample_subsurface_block(&self, rng: &mut SmallRng, world_y: i32) -> BlockType {
        if world_y <= 32 && rng.gen_bool(0.02) {
            return BlockType::IronOre;
        }
        if world_y <= 48 && rng.gen_bool(0.04) {
            return BlockType::CoalOre;
        }
        BlockType::Stone
    }

    fn random_flower(&self, rng: &mut SmallRng) -> BlockType {
        if rng.gen_bool(0.55) {
            BlockType::FlowerRose
        } else {
            BlockType::FlowerTulip
        }
    }

    fn block_for_column(&self, column: &ColumnInfo, height: i32, world_y: i32) -> BlockType {
        let depth = height - world_y;
        if depth < 0 {
            return BlockType::Air;
        }

        if world_y <= (GLOBAL_TERRAIN_BASE as i32 - 64) {
            return BlockType::Stone;
        }

        if (column.is_river || (column.river_bank > 0.35 && column.height <= WATER_LEVEL + 4))
            && depth <= 0
        {
            return column.config.river_bed;
        }

        match column.biome {
            BiomeType::Plains
            | BiomeType::Forest
            | BiomeType::Jungle
            | BiomeType::Savanna
            | BiomeType::Meadow => {
                if depth == 0 {
                    BlockType::Grass
                } else if depth <= column.config.surface_depth as i32 {
                    BlockType::Dirt
                } else {
                    column.config.deep_block
                }
            }
            BiomeType::Desert => {
                if depth <= column.config.surface_depth as i32 {
                    BlockType::Sand
                } else {
                    column.config.deep_block
                }
            }
            BiomeType::Mountain => {
                if depth == 0 && height >= 10 {
                    BlockType::Snow
                } else if depth <= 2 {
                    BlockType::Stone
                } else {
                    column.config.deep_block
                }
            }
            BiomeType::Swamp => {
                if depth == 0 {
                    BlockType::Grass
                } else if depth <= column.config.surface_depth as i32 {
                    BlockType::Dirt
                } else {
                    column.config.deep_block
                }
            }
            BiomeType::Tundra | BiomeType::Taiga => {
                if depth == 0 {
                    BlockType::Snow
                } else if depth <= column.config.surface_depth as i32 {
                    BlockType::Dirt
                } else {
                    column.config.deep_block
                }
            }
            BiomeType::Mesa => {
                let band = (depth / 2) % 3;
                if depth <= column.config.surface_depth as i32 + 2 {
                    if band == 1 {
                        BlockType::Sand
                    } else {
                        BlockType::Terracotta
                    }
                } else if depth % 6 == 0 {
                    BlockType::Terracotta
                } else {
                    column.config.deep_block
                }
            }
        }
    }

    fn should_carve_cave(&self, x: i32, y: i32, z: i32, column: &ColumnInfo) -> bool {
        if y <= 0 || y >= CHUNK_HEIGHT as i32 {
            return false;
        }

        if column.is_river || column.river_bank > 0.05 {
            return false;
        }

        if y > (GLOBAL_TERRAIN_BASE as i32 - 14) {
            return false;
        }

        let depth = column.height - y;
        if depth < -2 {
            return false;
        }

        let gen = &self.gen;
        let mask = gen
            .cave_mask_noise
            .get([x as f64 * 0.012, z as f64 * 0.012]);
        if mask < -0.1 {
            return false;
        }

        let cluster =
            gen.cave_cluster_noise
                .get([x as f64 * 0.04, y as f64 * 0.04, z as f64 * 0.04]);
        if cluster + mask * 0.5 < 0.3 {
            return false;
        }

        let base_scale = 0.022;
        let detail_scale = 0.09;

        let base = gen.cave_noise.get([
            x as f64 * base_scale,
            y as f64 * base_scale * 0.7,
            z as f64 * base_scale,
        ]);
        let detail = gen.cave_detail_noise.get([
            x as f64 * detail_scale,
            y as f64 * detail_scale,
            z as f64 * detail_scale,
        ]);

        let mask_positive = mask.clamp(0.0, 1.0);
        let shaped_base = base.abs() * (1.18 - 0.12 * mask_positive);
        let mut noise_value = shaped_base - detail * 0.22;
        noise_value -= cluster.max(0.0) * 0.04;

        let mut threshold = 0.21;

        if depth > 24 {
            threshold += ((depth - 24) as f64 / 90.0).min(0.16);
        }

        if y < 36 {
            threshold += ((36 - y) as f64 / 120.0).min(0.12);
        } else if depth < 10 {
            threshold += 0.02;
        }

        if mask > 0.45 {
            threshold -= 0.045;
        } else if mask < 0.05 {
            threshold += 0.03;
        }

        if cluster > 0.6 {
            threshold -= 0.035;
        } else if cluster < 0.2 {
            threshold += 0.018;
        }

        if detail > 0.55 {
            threshold -= 0.02;
        } else if detail < -0.55 {
            threshold += 0.015;
        }

        if column.altitude > 0.75 {
            threshold += 0.02;
        }

        if depth <= 3 {
            threshold += 0.025;
        }

        if noise_value < threshold {
            return true;
        }

        if depth <= 3 && noise_value < threshold + 0.02 {
            return true;
        }

        false
    }

    fn should_spawn_tree(
        &self,
        rng: &mut SmallRng,
        chunk: &Chunk,
        x: usize,
        z: usize,
        height: i32,
        density_multiplier: f64,
        canopy_radius: usize,
    ) -> Option<usize> {
        const TREE_CHANCE: f64 = 0.025;

        if density_multiplier <= 0.0 {
            return None;
        }

        if x < canopy_radius
            || x + canopy_radius >= CHUNK_SIZE
            || z < canopy_radius
            || z + canopy_radius >= CHUNK_SIZE
        {
            return None;
        }

        if height < 0 || height >= CHUNK_HEIGHT as i32 {
            return None;
        }

        let ground_y = height.min((CHUNK_HEIGHT - 1) as i32) as usize;
        let ground_block = chunk.get_block(x, ground_y, z);
        if ground_block != BlockType::Grass && ground_block != BlockType::Snow {
            return None;
        }

        let chance = (TREE_CHANCE * density_multiplier).clamp(0.0, 1.0);

        if rng.gen_bool(chance) {
            Some(ground_y)
        } else {
            None
        }
    }

    fn can_place_tree(
        &self,
        chunk: &Chunk,
        x: usize,
        base_y: usize,
        z: usize,
        trunk_height: usize,
        canopy_radius: usize,
        canopy_layers: usize,
    ) -> bool {
        if base_y == 0 {
            return false;
        }

        if trunk_height == 0 {
            return false;
        }

        for dy in 0..trunk_height {
            if chunk.get_block(x, base_y + dy, z) != BlockType::Air {
                return false;
            }
        }

        let top = base_y + trunk_height - 1;
        let last_layer = canopy_layers.saturating_sub(1);
        let max_leaf_y = top + last_layer;
        if max_leaf_y >= CHUNK_HEIGHT {
            return false;
        }

        for layer in 0..=last_layer {
            let y = top + layer;
            let shrink = layer.saturating_sub(1);
            let radius = canopy_radius.saturating_sub(shrink);
            let x_start = x.saturating_sub(radius);
            let x_end = (x + radius).min(CHUNK_SIZE - 1);
            let z_start = z.saturating_sub(radius);
            let z_end = (z + radius).min(CHUNK_SIZE - 1);

            for dx in x_start..=x_end {
                for dz in z_start..=z_end {
                    if layer == 0 && dx == x && dz == z {
                        continue;
                    }
                    if chunk.get_block(dx, y, dz) != BlockType::Air {
                        return false;
                    }
                }
            }
        }

        true
    }

    fn grow_tree(
        &self,
        chunk: &mut Chunk,
        x: usize,
        base_y: usize,
        z: usize,
        trunk_height: usize,
        canopy_radius: usize,
        canopy_layers: usize,
    ) {
        for dy in 0..trunk_height {
            chunk.set_block(x, base_y + dy, z, BlockType::Wood);
        }

        let top = base_y + trunk_height - 1;
        let last_layer = canopy_layers.saturating_sub(1);
        for layer in 0..=last_layer {
            let y = top + layer;
            let shrink = layer.saturating_sub(1);
            let radius = canopy_radius.saturating_sub(shrink);
            let x_start = x.saturating_sub(radius);
            let x_end = (x + radius).min(CHUNK_SIZE - 1);
            let z_start = z.saturating_sub(radius);
            let z_end = (z + radius).min(CHUNK_SIZE - 1);

            for dx in x_start..=x_end {
                for dz in z_start..=z_end {
                    if layer == 0 && dx == x && dz == z {
                        continue;
                    }
                    chunk.set_block(dx, y, dz, BlockType::Leaves);
                }
            }
        }
    }
}
