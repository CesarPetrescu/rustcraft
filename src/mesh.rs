use cgmath::{InnerSpace, Vector3};

use crate::block::{Axis, BlockFace, BlockType, RenderKind};
use crate::chunk::{Chunk, CHUNK_SIZE};
use crate::electric::{BlockPos3, ElectricalComponent, ElectricalNode};
use crate::texture::{
    atlas_uv_bounds, TILE_FLOWER_LEAF, TILE_FLOWER_ROSE_PETAL, TILE_FLOWER_STEM,
    TILE_FLOWER_TULIP_PETAL, TILE_GROUND_SIDE_CONNECTED, TILE_GROUND_SIDE_UNCONNECTED,
    TILE_GROUND_TOP_CONNECTED, TILE_GROUND_TOP_UNCONNECTED, TILE_RESISTOR_SIDE_CONNECTED,
    TILE_RESISTOR_SIDE_UNCONNECTED, TILE_RESISTOR_TOP_CONNECTED, TILE_RESISTOR_TOP_UNCONNECTED,
    TILE_VOLTAGE_SIDE_CONNECTED, TILE_VOLTAGE_SIDE_UNCONNECTED, TILE_VOLTAGE_TOP_CONNECTED,
    TILE_VOLTAGE_TOP_UNCONNECTED, TILE_WIRE_SIDE_CONNECTED, TILE_WIRE_SIDE_UNCONNECTED,
    TILE_WIRE_TOP_CONNECTED, TILE_WIRE_TOP_UNCONNECTED,
};
use crate::world::{ChunkPos, World, MAX_FLUID_LEVEL};

const HALF_BLOCK: f32 = 0.5;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub material: f32,
    pub tint: [f32; 3],
}

pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn push_quad(&mut self, quad: [Vertex; 4]) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&quad);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn push_quad_double_sided(&mut self, quad: [Vertex; 4]) {
        let back = [
            invert_normal(quad[0]),
            invert_normal(quad[3]),
            invert_normal(quad[2]),
            invert_normal(quad[1]),
        ];

        self.push_quad(quad);
        self.push_quad(back);
    }
}

fn invert_normal(mut vertex: Vertex) -> Vertex {
    vertex.normal[0] = -vertex.normal[0];
    vertex.normal[1] = -vertex.normal[1];
    vertex.normal[2] = -vertex.normal[2];
    vertex
}

type TileUv = (f32, f32, f32, f32);

#[derive(Clone, Copy)]
struct ComponentTextures {
    base_side: (u32, u32),
    base_top: (u32, u32),
    side_connected: (u32, u32),
    side_unconnected: (u32, u32),
    top_connected: (u32, u32),
    top_unconnected: (u32, u32),
}

struct ComponentUvs {
    side_base: TileUv,
    top_base: TileUv,
    side_connected: TileUv,
    side_unconnected: TileUv,
    top_connected: TileUv,
    top_unconnected: TileUv,
}

fn face_to_index(face: BlockFace) -> usize {
    match face {
        BlockFace::East => 0,
        BlockFace::West => 1,
        BlockFace::Top => 2,
        BlockFace::Bottom => 3,
        BlockFace::South => 4,
        BlockFace::North => 5,
    }
}

fn connector_present(connectors: &[bool; 6], face: BlockFace) -> bool {
    connectors[face_to_index(face)]
}

fn connection_active(connectors: &[bool; 6], connections: &[bool; 6], face: BlockFace) -> bool {
    let idx = face_to_index(face);
    connectors[idx] && connections[idx]
}

fn connector_target(block_half: f32, connected: bool, scale: f32, inset: f32, overlap: f32) -> f32 {
    if connected {
        block_half + scaled(overlap, scale)
    } else {
        (block_half - scaled(inset, scale)).max(0.0)
    }
}

fn component_textures(component: ElectricalComponent, block: BlockType) -> ComponentTextures {
    let base_side = block.atlas_coords(BlockFace::North);
    let base_top = block.atlas_coords(BlockFace::Top);
    match component {
        ElectricalComponent::Wire => ComponentTextures {
            base_side,
            base_top,
            side_connected: TILE_WIRE_SIDE_CONNECTED,
            side_unconnected: TILE_WIRE_SIDE_UNCONNECTED,
            top_connected: TILE_WIRE_TOP_CONNECTED,
            top_unconnected: TILE_WIRE_TOP_UNCONNECTED,
        },
        ElectricalComponent::Resistor => ComponentTextures {
            base_side,
            base_top,
            side_connected: TILE_RESISTOR_SIDE_CONNECTED,
            side_unconnected: TILE_RESISTOR_SIDE_UNCONNECTED,
            top_connected: TILE_RESISTOR_TOP_CONNECTED,
            top_unconnected: TILE_RESISTOR_TOP_UNCONNECTED,
        },
        ElectricalComponent::VoltageSource => ComponentTextures {
            base_side,
            base_top,
            side_connected: TILE_VOLTAGE_SIDE_CONNECTED,
            side_unconnected: TILE_VOLTAGE_SIDE_UNCONNECTED,
            top_connected: TILE_VOLTAGE_TOP_CONNECTED,
            top_unconnected: TILE_VOLTAGE_TOP_UNCONNECTED,
        },
        ElectricalComponent::Ground => ComponentTextures {
            base_side,
            base_top,
            side_connected: TILE_GROUND_SIDE_CONNECTED,
            side_unconnected: TILE_GROUND_SIDE_UNCONNECTED,
            top_connected: TILE_GROUND_TOP_CONNECTED,
            top_unconnected: TILE_GROUND_TOP_UNCONNECTED,
        },
    }
}

fn build_component_uvs(textures: ComponentTextures) -> ComponentUvs {
    ComponentUvs {
        side_base: atlas_uv_bounds(textures.base_side.0, textures.base_side.1),
        top_base: atlas_uv_bounds(textures.base_top.0, textures.base_top.1),
        side_connected: atlas_uv_bounds(textures.side_connected.0, textures.side_connected.1),
        side_unconnected: atlas_uv_bounds(textures.side_unconnected.0, textures.side_unconnected.1),
        top_connected: atlas_uv_bounds(textures.top_connected.0, textures.top_connected.1),
        top_unconnected: atlas_uv_bounds(textures.top_unconnected.0, textures.top_unconnected.1),
    }
}

fn emit_connection_plate(
    mesh: &mut MeshData,
    block_center: Vector3<f32>,
    block_half: f32,
    dir: Vector3<f32>,
    u_axis: Vector3<f32>,
    v_axis: Vector3<f32>,
    scale: f32,
    connected: bool,
    connected_uv: TileUv,
    unconnected_uv: TileUv,
    material: f32,
) {
    if dir.magnitude2() < 1e-6 {
        return;
    }
    let uv = if connected {
        connected_uv
    } else {
        unconnected_uv
    };
    let dir_norm = dir.normalize();
    let u_dir = u_axis.normalize();
    let v_dir = v_axis.normalize();
    if u_dir.magnitude2() < 1e-6 || v_dir.magnitude2() < 1e-6 {
        return;
    }

    let offset = dir_norm * (block_half + scaled(0.02, scale));
    let c = block_center + offset;
    let half_u = u_dir * scaled(0.32, scale);
    let half_v = v_dir * scaled(0.32, scale);

    let (u_min, u_max, v_min, v_max) = uv;
    let quad = [
        Vertex {
            position: [
                c.x - half_u.x - half_v.x,
                c.y - half_u.y - half_v.y,
                c.z - half_u.z - half_v.z,
            ],
            normal: [dir_norm.x, dir_norm.y, dir_norm.z],
            uv: [u_min, v_min],
            material,
            tint: [1.0, 1.0, 1.0],
        },
        Vertex {
            position: [
                c.x + half_u.x - half_v.x,
                c.y + half_u.y - half_v.y,
                c.z + half_u.z - half_v.z,
            ],
            normal: [dir_norm.x, dir_norm.y, dir_norm.z],
            uv: [u_max, v_min],
            material,
            tint: [1.0, 1.0, 1.0],
        },
        Vertex {
            position: [
                c.x + half_u.x + half_v.x,
                c.y + half_u.y + half_v.y,
                c.z + half_u.z + half_v.z,
            ],
            normal: [dir_norm.x, dir_norm.y, dir_norm.z],
            uv: [u_max, v_max],
            material,
            tint: [1.0, 1.0, 1.0],
        },
        Vertex {
            position: [
                c.x - half_u.x + half_v.x,
                c.y - half_u.y + half_v.y,
                c.z - half_u.z + half_v.z,
            ],
            normal: [dir_norm.x, dir_norm.y, dir_norm.z],
            uv: [u_min, v_max],
            material,
            tint: [1.0, 1.0, 1.0],
        },
    ];

    mesh.push_quad_double_sided(quad);
}
#[derive(Clone, Copy, Default)]
struct AxisLead {
    forward_present: bool,
    backward_present: bool,
    forward_connected: bool,
    backward_connected: bool,
}

impl AxisLead {
    const fn new(
        forward_present: bool,
        backward_present: bool,
        forward_connected: bool,
        backward_connected: bool,
    ) -> Self {
        Self {
            forward_present,
            backward_present,
            forward_connected,
            backward_connected,
        }
    }
}

const MATERIAL_OPAQUE: f32 = 0.0;
const MATERIAL_CUTOUT: f32 = 1.0;
const MATERIAL_TRANSLUCENT: f32 = 2.0;

fn material_for_block(block: BlockType) -> f32 {
    match block {
        BlockType::Water => MATERIAL_TRANSLUCENT,
        BlockType::Leaves
        | BlockType::FlowerRose
        | BlockType::FlowerTulip
        | BlockType::GlowShroom
        | BlockType::LilyPad => MATERIAL_CUTOUT,
        _ => MATERIAL_OPAQUE,
    }
}

pub fn generate_world_mesh(world: &World) -> MeshData {
    let mut combined = MeshData::new();
    let mut chunk_positions: Vec<ChunkPos> = world.chunks().keys().copied().collect();
    chunk_positions.sort_by_key(|pos| (pos.x, pos.z));

    for pos in chunk_positions {
        if let Some(chunk) = world.chunks().get(&pos) {
            let chunk_mesh = generate_chunk_mesh(world, pos, chunk);
            let base = combined.vertices.len() as u32;
            combined.vertices.extend_from_slice(&chunk_mesh.vertices);
            combined
                .indices
                .extend(chunk_mesh.indices.iter().map(|i| i + base));
        }
    }

    combined
}

pub fn generate_block_mesh(block: BlockType, origin: Vector3<f32>, scale: f32) -> MeshData {
    let mut mesh = MeshData::new();
    let half_extent = HALF_BLOCK * scale;
    let origin_array = [origin.x, origin.y, origin.z];

    match block.render_kind() {
        RenderKind::Solid => {
            let faces = [
                (BlockFace::Top, [0.0, 1.0, 0.0]),
                (BlockFace::Bottom, [0.0, -1.0, 0.0]),
                (BlockFace::North, [0.0, 0.0, -1.0]),
                (BlockFace::South, [0.0, 0.0, 1.0]),
                (BlockFace::East, [1.0, 0.0, 0.0]),
                (BlockFace::West, [-1.0, 0.0, 0.0]),
            ];

            for (face, normal) in faces {
                let quad = build_face(face, normal, block, origin_array, half_extent);
                mesh.push_quad(quad);
            }
        }
        RenderKind::Cross => {
            append_cross_billboard(&mut mesh, block, origin, half_extent);
        }
        RenderKind::Flower => {
            if matches!(block, BlockType::FlowerRose | BlockType::FlowerTulip) {
                append_flower_billboard(&mut mesh, block, origin, scale);
            } else {
                append_cross_billboard(&mut mesh, block, origin, half_extent);
            }
        }
        RenderKind::Flat => {
            append_flat_billboard(&mut mesh, block, origin, half_extent);
        }
        RenderKind::Electrical(_) => {
            append_electrical_preview(&mut mesh, block, origin, half_extent);
        }
    }

    mesh
}

pub fn generate_chunk_mesh(world: &World, chunk_pos: ChunkPos, chunk: &Chunk) -> MeshData {
    let mut mesh = MeshData::new();

    for (x, y, z, block) in chunk.iter() {
        let world_x = chunk_pos.x * CHUNK_SIZE as i32 + x as i32;
        let world_y = y as i32;
        let world_z = chunk_pos.z * CHUNK_SIZE as i32 + z as i32;
        let world_pos = BlockPos3::new(world_x, world_y, world_z);

        match block.render_kind() {
            RenderKind::Solid => {
                append_solid_block(&mut mesh, world, world_x, world_y, world_z, block)
            }
            RenderKind::Cross => append_cross_block(&mut mesh, world_x, world_y, world_z, block),
            RenderKind::Flower => {
                if matches!(block, BlockType::FlowerRose | BlockType::FlowerTulip) {
                    append_flower_block(&mut mesh, world_x, world_y, world_z, block);
                } else {
                    append_cross_block(&mut mesh, world_x, world_y, world_z, block)
                }
            }
            RenderKind::Flat => append_flat_block(&mut mesh, world_x, world_y, world_z, block),
            RenderKind::Electrical(_) => {}
        }

        if let Some(faces) = world.electrical().face_nodes(world_pos) {
            for (face, node) in faces.iter() {
                append_electrical_attachment(&mut mesh, world, world_pos, face, node);
            }
        }
    }

    for (x, y, z, amount) in chunk.fluids_iter() {
        let world_x = chunk_pos.x * CHUNK_SIZE as i32 + x as i32;
        let world_y = y as i32;
        let world_z = chunk_pos.z * CHUNK_SIZE as i32 + z as i32;
        append_fluid_block(&mut mesh, world, world_x, world_y, world_z, amount);
    }

    mesh
}

fn append_solid_block(
    mesh: &mut MeshData,
    world: &World,
    x: i32,
    y: i32,
    z: i32,
    block: BlockType,
) {
    let neighbors = [
        (BlockFace::Top, (x, y + 1, z), [0.0, 1.0, 0.0]),
        (BlockFace::Bottom, (x, y - 1, z), [0.0, -1.0, 0.0]),
        (BlockFace::North, (x, y, z - 1), [0.0, 0.0, -1.0]),
        (BlockFace::South, (x, y, z + 1), [0.0, 0.0, 1.0]),
        (BlockFace::East, (x + 1, y, z), [1.0, 0.0, 0.0]),
        (BlockFace::West, (x - 1, y, z), [-1.0, 0.0, 0.0]),
    ];

    for (face, (nx, ny, nz), normal) in neighbors.iter() {
        if !world.get_block(*nx, *ny, *nz).occludes() {
            let quad = build_face(
                *face,
                *normal,
                block,
                [x as f32, y as f32, z as f32],
                HALF_BLOCK,
            );
            mesh.push_quad(quad);
        }
    }
}

fn build_face(
    face: BlockFace,
    normal: [f32; 3],
    block: BlockType,
    origin: [f32; 3],
    half_extent: f32,
) -> [Vertex; 4] {
    let (tile_x, tile_y) = block.atlas_coords(face);
    let (u_min, u_max, v_min, v_max) = atlas_uv_bounds(tile_x, tile_y);
    let material = material_for_block(block);
    let (ox, oy, oz) = (origin[0], origin[1], origin[2]);

    let (p0, p1, p2, p3) = match face {
        BlockFace::Top => (
            [ox - half_extent, oy + half_extent, oz - half_extent],
            [ox - half_extent, oy + half_extent, oz + half_extent],
            [ox + half_extent, oy + half_extent, oz + half_extent],
            [ox + half_extent, oy + half_extent, oz - half_extent],
        ),
        BlockFace::Bottom => (
            [ox - half_extent, oy - half_extent, oz - half_extent],
            [ox + half_extent, oy - half_extent, oz - half_extent],
            [ox + half_extent, oy - half_extent, oz + half_extent],
            [ox - half_extent, oy - half_extent, oz + half_extent],
        ),
        BlockFace::North => (
            [ox - half_extent, oy - half_extent, oz - half_extent],
            [ox - half_extent, oy + half_extent, oz - half_extent],
            [ox + half_extent, oy + half_extent, oz - half_extent],
            [ox + half_extent, oy - half_extent, oz - half_extent],
        ),
        BlockFace::South => (
            [ox + half_extent, oy - half_extent, oz + half_extent],
            [ox + half_extent, oy + half_extent, oz + half_extent],
            [ox - half_extent, oy + half_extent, oz + half_extent],
            [ox - half_extent, oy - half_extent, oz + half_extent],
        ),
        BlockFace::East => (
            [ox + half_extent, oy - half_extent, oz - half_extent],
            [ox + half_extent, oy + half_extent, oz - half_extent],
            [ox + half_extent, oy + half_extent, oz + half_extent],
            [ox + half_extent, oy - half_extent, oz + half_extent],
        ),
        BlockFace::West => (
            [ox - half_extent, oy - half_extent, oz + half_extent],
            [ox - half_extent, oy + half_extent, oz + half_extent],
            [ox - half_extent, oy + half_extent, oz - half_extent],
            [ox - half_extent, oy - half_extent, oz - half_extent],
        ),
    };

    [
        Vertex {
            position: p0,
            normal,
            uv: [u_min, v_min],
            material,
            tint: [1.0, 1.0, 1.0],
        },
        Vertex {
            position: p1,
            normal,
            uv: [u_max, v_min],
            material,
            tint: [1.0, 1.0, 1.0],
        },
        Vertex {
            position: p2,
            normal,
            uv: [u_max, v_max],
            material,
            tint: [1.0, 1.0, 1.0],
        },
        Vertex {
            position: p3,
            normal,
            uv: [u_min, v_max],
            material,
            tint: [1.0, 1.0, 1.0],
        },
    ]
}

fn append_cross_block(mesh: &mut MeshData, x: i32, y: i32, z: i32, block: BlockType) {
    let material = material_for_block(block);
    let (tile_x, tile_y) = block.atlas_coords(BlockFace::Top);
    let (u_min, u_max, v_min, v_max) = atlas_uv_bounds(tile_x, tile_y);
    let cx = x as f32;
    let cy = y as f32;
    let cz = z as f32;

    let quads = [
        (
            [
                [cx - HALF_BLOCK, cy - HALF_BLOCK, cz - HALF_BLOCK],
                [cx + HALF_BLOCK, cy - HALF_BLOCK, cz + HALF_BLOCK],
                [cx + HALF_BLOCK, cy + HALF_BLOCK, cz + HALF_BLOCK],
                [cx - HALF_BLOCK, cy + HALF_BLOCK, cz - HALF_BLOCK],
            ],
            [HALF_BLOCK, 0.0, HALF_BLOCK],
        ),
        (
            [
                [cx - HALF_BLOCK, cy - HALF_BLOCK, cz + HALF_BLOCK],
                [cx + HALF_BLOCK, cy - HALF_BLOCK, cz - HALF_BLOCK],
                [cx + HALF_BLOCK, cy + HALF_BLOCK, cz - HALF_BLOCK],
                [cx - HALF_BLOCK, cy + HALF_BLOCK, cz + HALF_BLOCK],
            ],
            [HALF_BLOCK, 0.0, -HALF_BLOCK],
        ),
    ];

    for (positions, normal) in quads {
        mesh.push_quad_double_sided([
            Vertex {
                position: positions[0],
                normal,
                uv: [u_min, v_max],
                material,

                tint: [1.0, 1.0, 1.0],
            },
            Vertex {
                position: positions[1],
                normal,
                uv: [u_max, v_max],
                material,

                tint: [1.0, 1.0, 1.0],
            },
            Vertex {
                position: positions[2],
                normal,
                uv: [u_max, v_min],
                material,

                tint: [1.0, 1.0, 1.0],
            },
            Vertex {
                position: positions[3],
                normal,
                uv: [u_min, v_min],
                material,

                tint: [1.0, 1.0, 1.0],
            },
        ]);
    }
}

fn append_flower_block(mesh: &mut MeshData, x: i32, y: i32, z: i32, block: BlockType) {
    let center = Vector3::new(x as f32, y as f32, z as f32);
    emit_flower_geometry(mesh, center, 1.0, block);
}

fn append_flower_billboard(
    mesh: &mut MeshData,
    block: BlockType,
    origin: Vector3<f32>,
    scale: f32,
) {
    emit_flower_geometry(mesh, origin, scale, block);
}

fn emit_flower_geometry(mesh: &mut MeshData, center: Vector3<f32>, scale: f32, block: BlockType) {
    let material = material_for_block(block);
    match block {
        BlockType::FlowerRose => emit_rose_geometry(mesh, center, scale, material),
        BlockType::FlowerTulip => emit_tulip_geometry(mesh, center, scale, material),
        _ => {}
    }
}

fn emit_rose_geometry(mesh: &mut MeshData, center: Vector3<f32>, scale: f32, material: f32) {
    let half = HALF_BLOCK * scale;
    let base_y = center.y - half;

    let stem_uv = atlas_uv_bounds(TILE_FLOWER_STEM.0, TILE_FLOWER_STEM.1);
    let leaf_uv = atlas_uv_bounds(TILE_FLOWER_LEAF.0, TILE_FLOWER_LEAF.1);
    let petal_uv = atlas_uv_bounds(TILE_FLOWER_ROSE_PETAL.0, TILE_FLOWER_ROSE_PETAL.1);

    let stem_height = scale * 0.78;
    let stem_half = [scale * 0.05, stem_height * 0.5, scale * 0.05];
    let stem_center = Vector3::new(center.x, base_y + stem_half[1] + scale * 0.05, center.z);
    push_oriented_box(
        mesh,
        stem_center,
        Vector3::new(1.0, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        Vector3::new(0.0, 0.0, 1.0),
        stem_half,
        stem_uv,
        material,
        [0.78, 0.92, 0.70],
    );

    let upper_half = [scale * 0.032, scale * 0.10, scale * 0.032];
    let upper_center = Vector3::new(center.x, base_y + stem_height + upper_half[1], center.z);
    push_oriented_box(
        mesh,
        upper_center,
        Vector3::new(1.0, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        Vector3::new(0.0, 0.0, 1.0),
        upper_half,
        stem_uv,
        material,
        [0.82, 0.96, 0.74],
    );

    let lower_leaf_height = base_y + scale * 0.36;
    push_plant_panel(
        mesh,
        center,
        lower_leaf_height,
        28.0,
        18.0,
        scale * 0.18,
        0.0,
        scale * 0.19,
        scale * 0.08,
        scale * 0.02,
        leaf_uv,
        material,
        [0.84, 0.98, 0.74],
    );
    push_plant_panel(
        mesh,
        center,
        lower_leaf_height + scale * 0.06,
        -32.0,
        16.0,
        scale * 0.17,
        0.0,
        scale * 0.18,
        scale * 0.08,
        scale * 0.019,
        leaf_uv,
        material,
        [0.78, 0.94, 0.68],
    );
    let upper_leaf_height = base_y + scale * 0.48;
    push_plant_panel(
        mesh,
        center,
        upper_leaf_height,
        62.0,
        22.0,
        scale * 0.12,
        -scale * 0.01,
        scale * 0.12,
        scale * 0.07,
        scale * 0.018,
        leaf_uv,
        material,
        [0.88, 0.98, 0.80],
    );

    let sepal_height = base_y + stem_height + scale * 0.03;
    for angle in [0.0_f32, 90.0, 180.0, 270.0] {
        push_plant_panel(
            mesh,
            center,
            sepal_height,
            angle + 15.0,
            26.0,
            scale * 0.08,
            -scale * 0.004,
            scale * 0.08,
            scale * 0.05,
            scale * 0.014,
            leaf_uv,
            material,
            [0.88, 0.98, 0.74],
        );
    }

    let bud_center = Vector3::new(center.x, base_y + stem_height + scale * 0.12, center.z);
    let bud_half = [scale * 0.085, scale * 0.09, scale * 0.085];
    push_oriented_box(
        mesh,
        bud_center,
        Vector3::new(1.0, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        Vector3::new(0.0, 0.0, 1.0),
        bud_half,
        petal_uv,
        material,
        [1.0, 0.98, 0.96],
    );

    let petal_height = base_y + stem_height + scale * 0.07;
    let petal_half_tangent = scale * 0.12;
    let petal_half_bitangent = scale * 0.16;
    let petal_half_normal = scale * 0.02;
    for (idx, angle) in [0.0_f32, 72.0, 144.0, 216.0, 288.0].iter().enumerate() {
        let tilt = if idx % 2 == 0 { 18.0 } else { 24.0 };
        let normal_offset = if idx % 2 == 0 {
            scale * 0.01
        } else {
            scale * 0.015
        };
        push_plant_panel(
            mesh,
            center,
            petal_height,
            *angle,
            tilt,
            scale * 0.12,
            normal_offset,
            petal_half_tangent,
            petal_half_bitangent,
            petal_half_normal,
            petal_uv,
            material,
            [1.0, 1.0, 1.0],
        );
    }

    let inner_height = petal_height + scale * 0.018;
    let inner_half_tangent = petal_half_tangent * 0.65;
    let inner_half_bitangent = petal_half_bitangent * 0.7;
    let inner_half_normal = petal_half_normal * 0.9;
    for angle in [36.0_f32, 108.0, 180.0, 252.0, 324.0] {
        push_plant_panel(
            mesh,
            center,
            inner_height,
            angle,
            20.0,
            scale * 0.09,
            scale * 0.012,
            inner_half_tangent,
            inner_half_bitangent,
            inner_half_normal,
            petal_uv,
            material,
            [1.0, 1.0, 1.0],
        );
    }

    let crown_center = Vector3::new(center.x, base_y + stem_height + scale * 0.17, center.z);
    let crown_half = [scale * 0.045, scale * 0.045, scale * 0.045];
    push_oriented_box(
        mesh,
        crown_center,
        Vector3::new(1.0, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        Vector3::new(0.0, 0.0, 1.0),
        crown_half,
        petal_uv,
        material,
        [1.0, 0.96, 0.98],
    );
}

fn emit_tulip_geometry(mesh: &mut MeshData, center: Vector3<f32>, scale: f32, material: f32) {
    let half = HALF_BLOCK * scale;
    let base_y = center.y - half;

    let stem_uv = atlas_uv_bounds(TILE_FLOWER_STEM.0, TILE_FLOWER_STEM.1);
    let leaf_uv = atlas_uv_bounds(TILE_FLOWER_LEAF.0, TILE_FLOWER_LEAF.1);
    let petal_uv = atlas_uv_bounds(TILE_FLOWER_TULIP_PETAL.0, TILE_FLOWER_TULIP_PETAL.1);

    let stem_height = scale * 0.82;
    let stem_half = [scale * 0.045, stem_height * 0.5, scale * 0.045];
    let stem_center = Vector3::new(center.x, base_y + stem_half[1] + scale * 0.05, center.z);
    push_oriented_box(
        mesh,
        stem_center,
        Vector3::new(1.0, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        Vector3::new(0.0, 0.0, 1.0),
        stem_half,
        stem_uv,
        material,
        [0.76, 0.92, 0.68],
    );

    let sheath_height = base_y + scale * 0.2;
    push_plant_panel(
        mesh,
        center,
        sheath_height,
        12.0,
        22.0,
        scale * 0.09,
        -scale * 0.01,
        scale * 0.10,
        scale * 0.05,
        scale * 0.015,
        leaf_uv,
        material,
        [0.82, 0.98, 0.74],
    );
    push_plant_panel(
        mesh,
        center,
        sheath_height,
        -18.0,
        18.0,
        scale * 0.08,
        -scale * 0.008,
        scale * 0.10,
        scale * 0.05,
        scale * 0.015,
        leaf_uv,
        material,
        [0.78, 0.94, 0.70],
    );

    let leaf_height = base_y + scale * 0.44;
    push_plant_panel(
        mesh,
        center,
        leaf_height,
        34.0,
        25.0,
        scale * 0.22,
        -scale * 0.01,
        scale * 0.24,
        scale * 0.14,
        scale * 0.022,
        leaf_uv,
        material,
        [0.84, 0.98, 0.76],
    );
    push_plant_panel(
        mesh,
        center,
        leaf_height + scale * 0.04,
        -38.0,
        27.0,
        scale * 0.20,
        -scale * 0.012,
        scale * 0.22,
        scale * 0.13,
        scale * 0.021,
        leaf_uv,
        material,
        [0.80, 0.95, 0.72],
    );

    let outer_height = base_y + stem_height + scale * 0.04;
    let outer_half_tangent = scale * 0.10;
    let outer_half_bitangent = scale * 0.19;
    let outer_half_normal = scale * 0.025;
    for angle in [0.0_f32, 90.0, 180.0, 270.0] {
        push_plant_panel(
            mesh,
            center,
            outer_height,
            angle,
            12.0,
            scale * 0.11,
            scale * 0.012,
            outer_half_tangent,
            outer_half_bitangent,
            outer_half_normal,
            petal_uv,
            material,
            [1.0, 1.0, 1.0],
        );
    }

    let inner_height = outer_height + scale * 0.012;
    let inner_half_tangent = outer_half_tangent * 0.7;
    let inner_half_bitangent = outer_half_bitangent * 0.75;
    let inner_half_normal = outer_half_normal * 0.9;
    for angle in [45.0_f32, 135.0, 225.0, 315.0] {
        push_plant_panel(
            mesh,
            center,
            inner_height,
            angle,
            9.0,
            scale * 0.07,
            scale * 0.008,
            inner_half_tangent,
            inner_half_bitangent,
            inner_half_normal,
            petal_uv,
            material,
            [1.0, 0.99, 0.98],
        );
    }

    let cap_center = Vector3::new(center.x, base_y + stem_height + scale * 0.16, center.z);
    let cap_half = [scale * 0.055, scale * 0.06, scale * 0.055];
    push_oriented_box(
        mesh,
        cap_center,
        Vector3::new(1.0, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        Vector3::new(0.0, 0.0, 1.0),
        cap_half,
        petal_uv,
        material,
        [1.0, 0.98, 0.95],
    );
}

fn push_plant_panel(
    mesh: &mut MeshData,
    block_center: Vector3<f32>,
    center_height: f32,
    angle_deg: f32,
    tilt_deg: f32,
    forward: f32,
    normal_offset: f32,
    half_tangent: f32,
    half_bitangent: f32,
    half_normal: f32,
    uv: TileUv,
    material: f32,
    tint: [f32; 3],
) {
    let (tangent, bitangent, normal) = axis_basis(angle_deg, tilt_deg);
    let base = Vector3::new(block_center.x, center_height, block_center.z);
    let panel_center = base + tangent * forward + normal * normal_offset;
    push_oriented_box(
        mesh,
        panel_center,
        tangent,
        bitangent,
        normal,
        [half_tangent, half_bitangent, half_normal],
        uv,
        material,
        tint,
    );
}

fn axis_basis(angle_deg: f32, tilt_deg: f32) -> (Vector3<f32>, Vector3<f32>, Vector3<f32>) {
    let angle = angle_deg.to_radians();
    let tilt = tilt_deg.to_radians();
    let mut tangent = Vector3::new(angle.cos(), 0.0, angle.sin());
    if tangent.magnitude2() < 1e-6 {
        tangent = Vector3::new(1.0, 0.0, 0.0);
    }
    tangent = tangent.normalize();

    let up = Vector3::new(0.0, 1.0, 0.0);
    let mut bitangent = up * tilt.cos() + tangent * tilt.sin();
    if bitangent.magnitude2() < 1e-6 {
        bitangent = up;
    }
    bitangent = bitangent.normalize();

    let mut normal = tangent.cross(bitangent);
    if normal.magnitude2() < 1e-6 {
        normal = tangent.cross(up);
    }
    normal = normal.normalize();

    (tangent, bitangent, normal)
}

fn append_flat_block(mesh: &mut MeshData, x: i32, y: i32, z: i32, block: BlockType) {
    let material = material_for_block(block);
    let (tile_x, tile_y) = block.atlas_coords(BlockFace::Top);
    let (u_min, u_max, v_min, v_max) = atlas_uv_bounds(tile_x, tile_y);
    let y = y as f32 + 0.48;
    let cx = x as f32;
    let cz = z as f32;

    mesh.push_quad_double_sided([
        Vertex {
            position: [cx - HALF_BLOCK, y, cz - HALF_BLOCK],
            normal: [0.0, 1.0, 0.0],
            uv: [u_min, v_min],
            material,
            tint: [1.0, 1.0, 1.0],
        },
        Vertex {
            position: [cx - HALF_BLOCK, y, cz + HALF_BLOCK],
            normal: [0.0, 1.0, 0.0],
            uv: [u_min, v_max],
            material,
            tint: [1.0, 1.0, 1.0],
        },
        Vertex {
            position: [cx + HALF_BLOCK, y, cz + HALF_BLOCK],
            normal: [0.0, 1.0, 0.0],
            uv: [u_max, v_max],
            material,
            tint: [1.0, 1.0, 1.0],
        },
        Vertex {
            position: [cx + HALF_BLOCK, y, cz - HALF_BLOCK],
            normal: [0.0, 1.0, 0.0],
            uv: [u_max, v_min],
            material,
            tint: [1.0, 1.0, 1.0],
        },
    ]);
}

fn append_fluid_block(mesh: &mut MeshData, world: &World, x: i32, y: i32, z: i32, amount: u8) {
    if amount == 0 {
        return;
    }

    let fill_ratio = amount as f32 / MAX_FLUID_LEVEL as f32;
    if fill_ratio <= f32::EPSILON {
        return;
    }

    let material = MATERIAL_TRANSLUCENT;
    let tint = [1.0, 1.0, 1.0];
    let cx = x as f32;
    let cy = y as f32;
    let cz = z as f32;
    let bottom = cy - HALF_BLOCK;
    let mut top = bottom + fill_ratio;
    if top > cy + HALF_BLOCK {
        top = cy + HALF_BLOCK;
    }
    if top <= bottom + 0.001 {
        top = bottom + 0.001;
    }

    let (top_tile_x, top_tile_y) = BlockType::Water.atlas_coords(BlockFace::Top);
    let (top_u_min, top_u_max, top_v_min, top_v_max) = atlas_uv_bounds(top_tile_x, top_tile_y);

    let above_amount = world.get_fluid_amount(x, y + 1, z);
    let above_block = world.get_block(x, y + 1, z);
    // Only draw the surface if it is actually exposed.
    if above_amount == 0 && !above_block.occludes() {
        mesh.push_quad_double_sided([
            Vertex {
                position: [cx - HALF_BLOCK, top, cz - HALF_BLOCK],
                normal: [0.0, 1.0, 0.0],
                uv: [top_u_min, top_v_min],
                material,
                tint,
            },
            Vertex {
                position: [cx + HALF_BLOCK, top, cz - HALF_BLOCK],
                normal: [0.0, 1.0, 0.0],
                uv: [top_u_max, top_v_min],
                material,
                tint,
            },
            Vertex {
                position: [cx + HALF_BLOCK, top, cz + HALF_BLOCK],
                normal: [0.0, 1.0, 0.0],
                uv: [top_u_max, top_v_max],
                material,
                tint,
            },
            Vertex {
                position: [cx - HALF_BLOCK, top, cz + HALF_BLOCK],
                normal: [0.0, 1.0, 0.0],
                uv: [top_u_min, top_v_max],
                material,
                tint,
            },
        ]);
    }

    let neighbors = [
        BlockFace::North,
        BlockFace::South,
        BlockFace::East,
        BlockFace::West,
    ];

    // Emit side quads any time the neighbouring cell does not hide this face.
    for face in neighbors {
        let offset = face.normal();
        let nx = x + offset.x;
        let nz = z + offset.z;
        let neighbor_amount = world.get_fluid_amount(nx, y, nz);
        let neighbor_block = world.get_block(nx, y, nz);

        if neighbor_amount >= amount && neighbor_amount > 0 {
            continue;
        }

        if neighbor_amount == 0 && neighbor_block.occludes() {
            continue;
        }

        let neighbor_ratio = neighbor_amount as f32 / MAX_FLUID_LEVEL as f32;
        let mut side_bottom = if neighbor_amount > 0 {
            bottom + neighbor_ratio
        } else {
            bottom
        };

        if side_bottom >= top - 0.001 {
            continue;
        }

        side_bottom = side_bottom.clamp(bottom, top);

        let normal_vec = face.normal();
        let normal = [
            normal_vec.x as f32,
            normal_vec.y as f32,
            normal_vec.z as f32,
        ];
        let (tile_x, tile_y) = BlockType::Water.atlas_coords(face);
        let (u_min, u_max, v_min, v_max) = atlas_uv_bounds(tile_x, tile_y);

        let (p0, p1, p2, p3) = match face {
            BlockFace::North => (
                [cx - HALF_BLOCK, side_bottom, cz - HALF_BLOCK],
                [cx - HALF_BLOCK, top, cz - HALF_BLOCK],
                [cx + HALF_BLOCK, top, cz - HALF_BLOCK],
                [cx + HALF_BLOCK, side_bottom, cz - HALF_BLOCK],
            ),
            BlockFace::South => (
                [cx + HALF_BLOCK, side_bottom, cz + HALF_BLOCK],
                [cx + HALF_BLOCK, top, cz + HALF_BLOCK],
                [cx - HALF_BLOCK, top, cz + HALF_BLOCK],
                [cx - HALF_BLOCK, side_bottom, cz + HALF_BLOCK],
            ),
            BlockFace::East => (
                [cx + HALF_BLOCK, side_bottom, cz - HALF_BLOCK],
                [cx + HALF_BLOCK, top, cz - HALF_BLOCK],
                [cx + HALF_BLOCK, top, cz + HALF_BLOCK],
                [cx + HALF_BLOCK, side_bottom, cz + HALF_BLOCK],
            ),
            BlockFace::West => (
                [cx - HALF_BLOCK, side_bottom, cz + HALF_BLOCK],
                [cx - HALF_BLOCK, top, cz + HALF_BLOCK],
                [cx - HALF_BLOCK, top, cz - HALF_BLOCK],
                [cx - HALF_BLOCK, side_bottom, cz - HALF_BLOCK],
            ),
            _ => continue,
        };

        let quad = [
            Vertex {
                position: p0,
                normal,
                uv: [u_min, v_min],
                material,
                tint,
            },
            Vertex {
                position: p1,
                normal,
                uv: [u_max, v_min],
                material,
                tint,
            },
            Vertex {
                position: p2,
                normal,
                uv: [u_max, v_max],
                material,
                tint,
            },
            Vertex {
                position: p3,
                normal,
                uv: [u_min, v_max],
                material,
                tint,
            },
        ];

        mesh.push_quad_double_sided(quad);
    }
}

fn append_cross_billboard(
    mesh: &mut MeshData,
    block: BlockType,
    origin: Vector3<f32>,
    half_extent: f32,
) {
    let material = material_for_block(block);
    let (tile_x, tile_y) = block.atlas_coords(BlockFace::North);
    let (u_min, u_max, v_min, v_max) = atlas_uv_bounds(tile_x, tile_y);
    let cx = origin.x;
    let cy = origin.y;
    let cz = origin.z;

    let quads = [
        (
            [
                [cx - half_extent, cy - half_extent, cz - half_extent],
                [cx + half_extent, cy - half_extent, cz + half_extent],
                [cx + half_extent, cy + half_extent, cz + half_extent],
                [cx - half_extent, cy + half_extent, cz - half_extent],
            ],
            [0.0, 0.0, 1.0],
        ),
        (
            [
                [cx - half_extent, cy - half_extent, cz + half_extent],
                [cx + half_extent, cy - half_extent, cz - half_extent],
                [cx + half_extent, cy + half_extent, cz - half_extent],
                [cx - half_extent, cy + half_extent, cz + half_extent],
            ],
            [0.0, 0.0, -1.0],
        ),
    ];

    for (positions, normal) in quads {
        mesh.push_quad_double_sided([
            Vertex {
                position: positions[0],
                normal,
                uv: [u_min, v_max],
                material,

                tint: [1.0, 1.0, 1.0],
            },
            Vertex {
                position: positions[1],
                normal,
                uv: [u_max, v_max],
                material,

                tint: [1.0, 1.0, 1.0],
            },
            Vertex {
                position: positions[2],
                normal,
                uv: [u_max, v_min],
                material,

                tint: [1.0, 1.0, 1.0],
            },
            Vertex {
                position: positions[3],
                normal,
                uv: [u_min, v_min],
                material,

                tint: [1.0, 1.0, 1.0],
            },
        ]);
    }
}

fn append_flat_billboard(
    mesh: &mut MeshData,
    block: BlockType,
    origin: Vector3<f32>,
    half_extent: f32,
) {
    let material = material_for_block(block);
    let (tile_x, tile_y) = block.atlas_coords(BlockFace::Top);
    let (u_min, u_max, v_min, v_max) = atlas_uv_bounds(tile_x, tile_y);
    let y = origin.y;
    let cx = origin.x;
    let cz = origin.z;

    mesh.push_quad_double_sided([
        Vertex {
            position: [cx - half_extent, y, cz - half_extent],
            normal: [0.0, 1.0, 0.0],
            uv: [u_min, v_min],
            material,
            tint: [1.0, 1.0, 1.0],
        },
        Vertex {
            position: [cx + half_extent, y, cz - half_extent],
            normal: [0.0, 1.0, 0.0],
            uv: [u_max, v_min],
            material,
            tint: [1.0, 1.0, 1.0],
        },
        Vertex {
            position: [cx + half_extent, y, cz + half_extent],
            normal: [0.0, 1.0, 0.0],
            uv: [u_max, v_max],
            material,
            tint: [1.0, 1.0, 1.0],
        },
        Vertex {
            position: [cx - half_extent, y, cz + half_extent],
            normal: [0.0, 1.0, 0.0],
            uv: [u_min, v_max],
            material,
            tint: [1.0, 1.0, 1.0],
        },
    ]);
}

fn append_electrical_preview(
    mesh: &mut MeshData,
    block: BlockType,
    origin: Vector3<f32>,
    half_extent: f32,
) {
    let Some(component) = ElectricalComponent::from_block(block) else {
        return;
    };
    let scale = half_extent / HALF_BLOCK;
    let face = BlockFace::Top;
    let axis = component.default_axis();
    let connectors = component.connectors(axis, face);
    append_component_mesh(
        mesh, block, component, origin, face, axis, scale, connectors, [false; 6],
    );
}

fn append_electrical_attachment(
    mesh: &mut MeshData,
    world: &World,
    pos: BlockPos3,
    face: BlockFace,
    node: &ElectricalNode,
) {
    let component = node.component;
    let block = component.block_type();
    let center = Vector3::new(pos.x as f32, pos.y as f32, pos.z as f32);
    let connectors = node.connectors();
    let connections = world
        .electrical()
        .connection_mask(pos, face)
        .unwrap_or([false; 6]);
    append_component_mesh(
        mesh,
        block,
        component,
        center,
        face,
        node.axis,
        1.0,
        connectors,
        connections,
    );
}

fn append_component_mesh(
    mesh: &mut MeshData,
    block: BlockType,
    component: ElectricalComponent,
    block_center: Vector3<f32>,
    face: BlockFace,
    axis: Axis,
    scale: f32,
    connectors: [bool; 6],
    connections: [bool; 6],
) {
    if scale <= 0.0 {
        return;
    }
    let (normal, tangent, bitangent) = component_basis(axis, face);
    let material = material_for_block(block);
    let textures = component_textures(component, block);
    let uvs = build_component_uvs(textures);
    let block_half = HALF_BLOCK * scale;
    let positive_face = axis.positive_face();
    let negative_face = axis.negative_face();
    let positive_present = connector_present(&connectors, positive_face);
    let negative_present = connector_present(&connectors, negative_face);
    let positive_connected = connection_active(&connectors, &connections, positive_face);
    let negative_connected = connection_active(&connectors, &connections, negative_face);
    let mount_face = face;
    let mount_present = connector_present(&connectors, mount_face);
    let mount_connected = connection_active(&connectors, &connections, mount_face);
    let opposite_face = face.opposite();
    let opposite_present = connector_present(&connectors, opposite_face);
    let opposite_connected = connection_active(&connectors, &connections, opposite_face);
    let axis_dir = axis.as_dir();
    let secondary_axis = Axis::all()
        .into_iter()
        .find(|candidate| *candidate != axis && *candidate != face.axis())
        .unwrap_or(axis);
    let secondary_positive = secondary_axis.positive_face();
    let secondary_negative = secondary_axis.negative_face();
    let secondary_lead = if secondary_axis != axis {
        AxisLead::new(
            connector_present(&connectors, secondary_positive),
            connector_present(&connectors, secondary_negative),
            connection_active(&connectors, &connections, secondary_positive),
            connection_active(&connectors, &connections, secondary_negative),
        )
    } else {
        AxisLead::default()
    };
    let primary_lead = AxisLead::new(
        positive_present,
        negative_present,
        positive_connected,
        negative_connected,
    );
    let primary_sign = if tangent.dot(axis_dir) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let secondary_sign = if secondary_axis != axis {
        let secondary_dir = secondary_axis.as_dir();
        if bitangent.dot(secondary_dir) >= 0.0 {
            1.0
        } else {
            -1.0
        }
    } else {
        1.0
    };

    match component {
        ElectricalComponent::Wire => append_wire_mesh(
            mesh,
            material,
            block_center,
            block_half,
            normal,
            tangent,
            bitangent,
            &uvs,
            scale,
            primary_lead,
            secondary_lead,
            primary_sign,
            secondary_sign,
        ),
        ElectricalComponent::Resistor => append_resistor_mesh(
            mesh,
            material,
            block_center,
            block_half,
            normal,
            tangent,
            bitangent,
            &uvs,
            scale,
            primary_lead,
            secondary_lead,
            primary_sign,
            secondary_sign,
        ),
        ElectricalComponent::VoltageSource => append_voltage_source_mesh(
            mesh,
            material,
            block_center,
            block_half,
            normal,
            tangent,
            bitangent,
            &uvs,
            scale,
            primary_lead,
            secondary_lead,
            primary_sign,
            secondary_sign,
        ),
        ElectricalComponent::Ground => append_ground_mesh(
            mesh,
            material,
            block_center,
            block_half,
            normal,
            tangent,
            bitangent,
            &uvs,
            scale,
            mount_present,
            mount_connected,
            opposite_present,
            opposite_connected,
        ),
    }

    if primary_lead.forward_present {
        emit_connection_plate(
            mesh,
            block_center,
            block_half,
            tangent * primary_sign,
            normal,
            bitangent,
            scale,
            primary_lead.forward_connected,
            uvs.side_connected,
            uvs.side_unconnected,
            material,
        );
    }
    if primary_lead.backward_present {
        emit_connection_plate(
            mesh,
            block_center,
            block_half,
            tangent * -primary_sign,
            normal,
            bitangent,
            scale,
            primary_lead.backward_connected,
            uvs.side_connected,
            uvs.side_unconnected,
            material,
        );
    }
    if secondary_lead.forward_present {
        emit_connection_plate(
            mesh,
            block_center,
            block_half,
            bitangent * secondary_sign,
            normal,
            tangent,
            scale,
            secondary_lead.forward_connected,
            uvs.side_connected,
            uvs.side_unconnected,
            material,
        );
    }
    if secondary_lead.backward_present {
        emit_connection_plate(
            mesh,
            block_center,
            block_half,
            bitangent * -secondary_sign,
            normal,
            tangent,
            scale,
            secondary_lead.backward_connected,
            uvs.side_connected,
            uvs.side_unconnected,
            material,
        );
    }
    if mount_present {
        emit_connection_plate(
            mesh,
            block_center,
            block_half,
            normal,
            tangent,
            bitangent,
            scale,
            mount_connected,
            uvs.top_connected,
            uvs.top_unconnected,
            material,
        );
    }
    if opposite_present {
        emit_connection_plate(
            mesh,
            block_center,
            block_half,
            -normal,
            tangent,
            bitangent,
            scale,
            opposite_connected,
            uvs.top_connected,
            uvs.top_unconnected,
            material,
        );
    }
}

fn append_wire_mesh(
    mesh: &mut MeshData,
    material: f32,
    block_center: Vector3<f32>,
    block_half: f32,
    normal: Vector3<f32>,
    tangent: Vector3<f32>,
    bitangent: Vector3<f32>,
    uvs: &ComponentUvs,
    scale: f32,
    primary: AxisLead,
    secondary: AxisLead,
    primary_sign: f32,
    secondary_sign: f32,
) {
    let body_half = [
        scaled(0.32, scale),
        scaled(0.08, scale),
        scaled(0.05, scale),
    ];
    let body_center = block_center + normal * (block_half + body_half[2] + scaled(0.012, scale));
    push_component_box(
        mesh,
        body_center,
        tangent,
        bitangent,
        normal,
        body_half,
        uvs.side_base,
        uvs.top_base,
        material,
        [1.0, 1.0, 1.0],
    );

    let insulator_half = [body_half[0], body_half[1] * 0.58, body_half[2] * 0.4];
    let insulator_center = body_center + normal * (body_half[2] - insulator_half[2] * 0.6);
    push_oriented_box(
        mesh,
        insulator_center,
        tangent,
        bitangent,
        normal,
        insulator_half,
        uvs.top_base,
        material,
        [0.3, 0.26, 0.36],
    );

    let lead_radius = scaled(0.045, scale);
    let lead_depth = scaled(0.04, scale);

    if primary.forward_present {
        let target = connector_target(block_half, primary.forward_connected, scale, 0.045, 0.012);
        if target > body_half[0] + 0.004 {
            let lead_length = (target - body_half[0]).max(0.01);
            let lead_half = [lead_length * 0.5, lead_radius, lead_depth];
            let lead_offset = body_half[0] + lead_half[0];
            let lead_uv = if primary.forward_connected {
                uvs.side_connected
            } else {
                uvs.side_unconnected
            };
            push_oriented_box(
                mesh,
                body_center + tangent * (primary_sign * lead_offset),
                tangent,
                bitangent,
                normal,
                lead_half,
                lead_uv,
                material,
                [1.0, 0.68, 0.32],
            );
        }
    }

    if primary.backward_present {
        let target = connector_target(block_half, primary.backward_connected, scale, 0.045, 0.012);
        if target > body_half[0] + 0.004 {
            let lead_length = (target - body_half[0]).max(0.01);
            let lead_half = [lead_length * 0.5, lead_radius, lead_depth];
            let lead_offset = body_half[0] + lead_half[0];
            let lead_uv = if primary.backward_connected {
                uvs.side_connected
            } else {
                uvs.side_unconnected
            };
            push_oriented_box(
                mesh,
                body_center + tangent * (-primary_sign * lead_offset),
                tangent,
                bitangent,
                normal,
                lead_half,
                lead_uv,
                material,
                [0.86, 0.54, 0.28],
            );
        }
    }

    if secondary.forward_present {
        let target = connector_target(block_half, secondary.forward_connected, scale, 0.045, 0.012);
        if target > body_half[1] + 0.004 {
            let lead_length = (target - body_half[1]).max(0.01);
            let lead_half = [lead_radius, lead_length * 0.5, lead_depth];
            let lead_offset = body_half[1] + lead_half[1];
            let lead_uv = if secondary.forward_connected {
                uvs.side_connected
            } else {
                uvs.side_unconnected
            };
            push_oriented_box(
                mesh,
                body_center + bitangent * (secondary_sign * lead_offset),
                tangent,
                bitangent,
                normal,
                lead_half,
                lead_uv,
                material,
                [1.0, 0.66, 0.44],
            );
        }
    }

    if secondary.backward_present {
        let target = connector_target(
            block_half,
            secondary.backward_connected,
            scale,
            0.045,
            0.012,
        );
        if target > body_half[1] + 0.004 {
            let lead_length = (target - body_half[1]).max(0.01);
            let lead_half = [lead_radius, lead_length * 0.5, lead_depth];
            let lead_offset = body_half[1] + lead_half[1];
            let lead_uv = if secondary.backward_connected {
                uvs.side_connected
            } else {
                uvs.side_unconnected
            };
            push_oriented_box(
                mesh,
                body_center + bitangent * (-secondary_sign * lead_offset),
                tangent,
                bitangent,
                normal,
                lead_half,
                lead_uv,
                material,
                [0.78, 0.48, 0.32],
            );
        }
    }
}

fn append_resistor_mesh(
    mesh: &mut MeshData,
    material: f32,
    block_center: Vector3<f32>,
    block_half: f32,
    normal: Vector3<f32>,
    tangent: Vector3<f32>,
    bitangent: Vector3<f32>,
    uvs: &ComponentUvs,
    scale: f32,
    primary: AxisLead,
    secondary: AxisLead,
    primary_sign: f32,
    secondary_sign: f32,
) {
    let body_half = [
        scaled(0.26, scale),
        scaled(0.12, scale),
        scaled(0.11, scale),
    ];
    let body_center = block_center + normal * (block_half + body_half[2] + scaled(0.015, scale));
    push_component_box(
        mesh,
        body_center,
        tangent,
        bitangent,
        normal,
        body_half,
        uvs.side_base,
        uvs.top_base,
        material,
        [1.0, 1.0, 1.0],
    );

    let collar_half = [
        body_half[0] * 0.32,
        body_half[1] * 0.65,
        body_half[2] * 0.85,
    ];
    let collar_center = body_center + normal * (body_half[2] - collar_half[2]);
    push_oriented_box(
        mesh,
        collar_center,
        tangent,
        bitangent,
        normal,
        collar_half,
        uvs.top_base,
        material,
        [0.72, 0.42, 0.28],
    );

    let cap_half = [
        scaled(0.05, scale),
        body_half[1] * 0.92,
        body_half[2] * 0.96,
    ];
    let cap_offset = body_half[0] - cap_half[0] + scaled(0.008, scale);
    let cap_tint = [0.86, 0.78, 0.62];
    push_oriented_box(
        mesh,
        body_center + tangent * cap_offset,
        tangent,
        bitangent,
        normal,
        cap_half,
        uvs.top_base,
        material,
        cap_tint,
    );
    push_oriented_box(
        mesh,
        body_center - tangent * cap_offset,
        tangent,
        bitangent,
        normal,
        cap_half,
        uvs.top_base,
        material,
        cap_tint,
    );

    let lead_radius = scaled(0.042, scale);
    let lead_depth = scaled(0.035, scale);

    if primary.forward_present {
        let target = connector_target(block_half, primary.forward_connected, scale, 0.05, 0.014);
        if target > body_half[0] + 0.004 {
            let lead_length = (target - body_half[0]).max(0.01);
            let lead_half = [lead_length * 0.5, lead_radius, lead_depth];
            let lead_offset = body_half[0] + lead_half[0];
            let lead_uv = if primary.forward_connected {
                uvs.side_connected
            } else {
                uvs.side_unconnected
            };
            push_oriented_box(
                mesh,
                body_center + tangent * (primary_sign * lead_offset),
                tangent,
                bitangent,
                normal,
                lead_half,
                lead_uv,
                material,
                [0.82, 0.82, 0.82],
            );
        }
    }

    if primary.backward_present {
        let target = connector_target(block_half, primary.backward_connected, scale, 0.05, 0.014);
        if target > body_half[0] + 0.004 {
            let lead_length = (target - body_half[0]).max(0.01);
            let lead_half = [lead_length * 0.5, lead_radius, lead_depth];
            let lead_offset = body_half[0] + lead_half[0];
            let lead_uv = if primary.backward_connected {
                uvs.side_connected
            } else {
                uvs.side_unconnected
            };
            push_oriented_box(
                mesh,
                body_center + tangent * (-primary_sign * lead_offset),
                tangent,
                bitangent,
                normal,
                lead_half,
                lead_uv,
                material,
                [0.74, 0.74, 0.74],
            );
        }
    }

    if secondary.forward_present {
        let target = connector_target(block_half, secondary.forward_connected, scale, 0.05, 0.014);
        if target > body_half[1] + 0.004 {
            let lead_length = (target - body_half[1]).max(0.01);
            let lead_half = [lead_radius, lead_length * 0.5, lead_depth];
            let lead_offset = body_half[1] + lead_half[1];
            let lead_uv = if secondary.forward_connected {
                uvs.side_connected
            } else {
                uvs.side_unconnected
            };
            push_oriented_box(
                mesh,
                body_center + bitangent * (secondary_sign * lead_offset),
                tangent,
                bitangent,
                normal,
                lead_half,
                lead_uv,
                material,
                [0.78, 0.82, 0.82],
            );
        }
    }

    if secondary.backward_present {
        let target = connector_target(block_half, secondary.backward_connected, scale, 0.05, 0.014);
        if target > body_half[1] + 0.004 {
            let lead_length = (target - body_half[1]).max(0.01);
            let lead_half = [lead_radius, lead_length * 0.5, lead_depth];
            let lead_offset = body_half[1] + lead_half[1];
            let lead_uv = if secondary.backward_connected {
                uvs.side_connected
            } else {
                uvs.side_unconnected
            };
            push_oriented_box(
                mesh,
                body_center + bitangent * (-secondary_sign * lead_offset),
                tangent,
                bitangent,
                normal,
                lead_half,
                lead_uv,
                material,
                [0.68, 0.72, 0.72],
            );
        }
    }
}

fn append_voltage_source_mesh(
    mesh: &mut MeshData,
    material: f32,
    block_center: Vector3<f32>,
    block_half: f32,
    normal: Vector3<f32>,
    tangent: Vector3<f32>,
    bitangent: Vector3<f32>,
    uvs: &ComponentUvs,
    scale: f32,
    primary: AxisLead,
    secondary: AxisLead,
    primary_sign: f32,
    secondary_sign: f32,
) {
    let body_half = [
        scaled(0.24, scale),
        scaled(0.16, scale),
        scaled(0.18, scale),
    ];
    let body_center = block_center + normal * (block_half + body_half[2] + scaled(0.02, scale));
    push_component_box(
        mesh,
        body_center,
        tangent,
        bitangent,
        normal,
        body_half,
        uvs.side_base,
        uvs.top_base,
        material,
        [1.0, 1.0, 1.0],
    );

    let band_half = [
        body_half[0] * 0.96,
        scaled(0.028, scale),
        body_half[2] * 0.6,
    ];
    let band_center =
        body_center + bitangent * (body_half[1] - band_half[1] - scaled(0.006, scale));
    push_oriented_box(
        mesh,
        band_center,
        tangent,
        bitangent,
        normal,
        band_half,
        uvs.top_base,
        material,
        [0.94, 0.95, 0.98],
    );

    let base_half = [body_half[0] * 0.96, scaled(0.03, scale), body_half[2] * 0.5];
    let base_center =
        body_center - bitangent * (body_half[1] - base_half[1] - scaled(0.004, scale));
    push_oriented_box(
        mesh,
        base_center,
        tangent,
        bitangent,
        normal,
        base_half,
        uvs.top_base,
        material,
        [0.16, 0.2, 0.28],
    );

    let terminal_half = [
        scaled(0.05, scale),
        scaled(0.05, scale),
        scaled(0.05, scale),
    ];
    let terminal_offset = body_half[0] + terminal_half[0] + scaled(0.01, scale);
    let positive_center = body_center + tangent * (primary_sign * terminal_offset);
    let negative_center = body_center + tangent * (-primary_sign * terminal_offset);

    if primary.forward_present {
        let terminal_uv = if primary.forward_connected {
            uvs.top_connected
        } else {
            uvs.top_unconnected
        };
        push_oriented_box(
            mesh,
            positive_center,
            tangent,
            bitangent,
            normal,
            terminal_half,
            terminal_uv,
            material,
            [1.0, 0.3, 0.34],
        );
    }

    if primary.backward_present {
        let terminal_uv = if primary.backward_connected {
            uvs.top_connected
        } else {
            uvs.top_unconnected
        };
        push_oriented_box(
            mesh,
            negative_center,
            tangent,
            bitangent,
            normal,
            terminal_half,
            terminal_uv,
            material,
            [0.16, 0.18, 0.22],
        );
    }

    let base_offset = body_half[0] + terminal_half[0];
    let lead_radius = scaled(0.04, scale);
    let lead_depth = scaled(0.036, scale);

    if primary.forward_present {
        let target = connector_target(block_half, primary.forward_connected, scale, 0.055, 0.016);
        if target > base_offset + 0.004 {
            let lead_length = (target - base_offset).max(0.01);
            let lead_half = [lead_length * 0.5, lead_radius, lead_depth];
            let lead_offset = base_offset + lead_half[0];
            let lead_uv = if primary.forward_connected {
                uvs.side_connected
            } else {
                uvs.side_unconnected
            };
            push_oriented_box(
                mesh,
                body_center + tangent * (primary_sign * lead_offset),
                tangent,
                bitangent,
                normal,
                lead_half,
                lead_uv,
                material,
                [1.0, 0.38, 0.38],
            );
        }
    }

    if primary.backward_present {
        let target = connector_target(block_half, primary.backward_connected, scale, 0.055, 0.016);
        if target > base_offset + 0.004 {
            let lead_length = (target - base_offset).max(0.01);
            let lead_half = [lead_length * 0.5, lead_radius, lead_depth];
            let lead_offset = base_offset + lead_half[0];
            let lead_uv = if primary.backward_connected {
                uvs.side_connected
            } else {
                uvs.side_unconnected
            };
            push_oriented_box(
                mesh,
                body_center + tangent * (-primary_sign * lead_offset),
                tangent,
                bitangent,
                normal,
                lead_half,
                lead_uv,
                material,
                [0.2, 0.22, 0.26],
            );
        }
    }

    let plus_center =
        positive_center + normal * (body_half[2] + terminal_half[2] + scaled(0.04, scale));
    let minus_center =
        negative_center + normal * (body_half[2] + terminal_half[2] + scaled(0.04, scale));
    let plus_horizontal = [
        scaled(0.062, scale),
        scaled(0.01, scale),
        scaled(0.006, scale),
    ];
    let plus_vertical = [
        scaled(0.01, scale),
        scaled(0.062, scale),
        scaled(0.006, scale),
    ];
    let plus_uv = if primary.forward_present && primary.forward_connected {
        uvs.top_connected
    } else {
        uvs.top_unconnected
    };
    push_oriented_box(
        mesh,
        plus_center,
        tangent,
        bitangent,
        normal,
        plus_horizontal,
        plus_uv,
        material,
        [1.0, 0.34, 0.34],
    );
    push_oriented_box(
        mesh,
        plus_center,
        tangent,
        bitangent,
        normal,
        plus_vertical,
        plus_uv,
        material,
        [1.0, 0.34, 0.34],
    );

    let minus_uv = if primary.backward_present && primary.backward_connected {
        uvs.top_connected
    } else {
        uvs.top_unconnected
    };
    let minus_half = [
        scaled(0.06, scale),
        scaled(0.01, scale),
        scaled(0.006, scale),
    ];
    push_oriented_box(
        mesh,
        minus_center,
        tangent,
        bitangent,
        normal,
        minus_half,
        minus_uv,
        material,
        [0.1, 0.1, 0.11],
    );

    if secondary.forward_present {
        let target = connector_target(block_half, secondary.forward_connected, scale, 0.055, 0.016);
        if target > body_half[1] + 0.004 {
            let lead_length = (target - body_half[1]).max(0.01);
            let lead_half = [lead_radius, lead_length * 0.5, lead_depth];
            let lead_offset = body_half[1] + lead_half[1];
            let lead_uv = if secondary.forward_connected {
                uvs.side_connected
            } else {
                uvs.side_unconnected
            };
            push_oriented_box(
                mesh,
                body_center + bitangent * (secondary_sign * lead_offset),
                tangent,
                bitangent,
                normal,
                lead_half,
                lead_uv,
                material,
                [0.86, 0.44, 0.44],
            );
        }
    }

    if secondary.backward_present {
        let target = connector_target(
            block_half,
            secondary.backward_connected,
            scale,
            0.055,
            0.016,
        );
        if target > body_half[1] + 0.004 {
            let lead_length = (target - body_half[1]).max(0.01);
            let lead_half = [lead_radius, lead_length * 0.5, lead_depth];
            let lead_offset = body_half[1] + lead_half[1];
            let lead_uv = if secondary.backward_connected {
                uvs.side_connected
            } else {
                uvs.side_unconnected
            };
            push_oriented_box(
                mesh,
                body_center + bitangent * (-secondary_sign * lead_offset),
                tangent,
                bitangent,
                normal,
                lead_half,
                lead_uv,
                material,
                [0.28, 0.32, 0.34],
            );
        }
    }
}

fn append_ground_mesh(
    mesh: &mut MeshData,
    material: f32,
    block_center: Vector3<f32>,
    block_half: f32,
    normal: Vector3<f32>,
    tangent: Vector3<f32>,
    bitangent: Vector3<f32>,
    uvs: &ComponentUvs,
    scale: f32,
    mount_present: bool,
    mount_connected: bool,
    opposite_present: bool,
    opposite_connected: bool,
) {
    let plate_thickness = 0.02 + if mount_present { 0.003 } else { 0.0 };
    let plate_half = [
        scaled(0.24, scale),
        scaled(0.24, scale),
        scaled(plate_thickness, scale),
    ];
    let plate_center = block_center + normal * (block_half + plate_half[2] + scaled(0.004, scale));
    let (plate_top_uv, plate_side_uv) = if mount_present {
        if mount_connected {
            (uvs.top_connected, uvs.side_connected)
        } else {
            (uvs.top_unconnected, uvs.side_unconnected)
        }
    } else {
        (uvs.top_base, uvs.side_base)
    };
    push_component_box(
        mesh,
        plate_center,
        tangent,
        bitangent,
        normal,
        plate_half,
        plate_side_uv,
        plate_top_uv,
        material,
        [0.24, 0.26, 0.32],
    );

    let mast_height = 0.18 + if mount_connected { 0.02 } else { 0.0 };
    let mast_half = [
        scaled(0.05, scale),
        scaled(0.05, scale),
        scaled(mast_height, scale),
    ];
    let mast_center = plate_center + normal * (plate_half[2] + mast_half[2] + scaled(0.01, scale));
    push_oriented_box(
        mesh,
        mast_center,
        tangent,
        bitangent,
        normal,
        mast_half,
        uvs.side_base,
        material,
        [0.94, 0.92, 0.58],
    );

    let anchor_extension = if opposite_connected {
        scaled(0.12, scale)
    } else if opposite_present {
        scaled(0.04, scale)
    } else {
        0.0
    };
    let anchor_length = (block_half * 0.8 + anchor_extension).max(0.04);
    let anchor_half = [
        scaled(0.035, scale),
        scaled(0.035, scale),
        anchor_length * 0.5,
    ];
    let anchor_center = block_center - normal * (anchor_half[2] + scaled(0.01, scale));
    let anchor_uv = if opposite_present {
        if opposite_connected {
            uvs.side_connected
        } else {
            uvs.side_unconnected
        }
    } else {
        uvs.side_base
    };
    let anchor_tint = if opposite_connected {
        [0.36, 0.58, 0.42]
    } else {
        [0.52, 0.56, 0.36]
    };
    push_oriented_box(
        mesh,
        anchor_center,
        tangent,
        bitangent,
        normal,
        anchor_half,
        anchor_uv,
        material,
        anchor_tint,
    );

    let mast_tip = mast_center + normal * mast_half[2];
    let bar_uv = if mount_connected {
        uvs.top_connected
    } else if mount_present {
        uvs.top_unconnected
    } else {
        uvs.top_base
    };
    let widths = [0.22, 0.16, 0.08];
    for (idx, width) in widths.iter().enumerate() {
        let offset = 0.02 + idx as f32 * 0.02;
        let bar_half = [
            scaled(*width, scale),
            scaled(0.014, scale),
            scaled(0.006, scale),
        ];
        let bar_center = mast_tip + normal * scaled(offset, scale);
        push_oriented_box(
            mesh,
            bar_center,
            tangent,
            bitangent,
            normal,
            bar_half,
            bar_uv,
            material,
            [0.96, 0.94, 0.62],
        );
    }
}

fn push_oriented_box(
    mesh: &mut MeshData,
    center: Vector3<f32>,
    tangent: Vector3<f32>,
    bitangent: Vector3<f32>,
    normal: Vector3<f32>,
    half: [f32; 3],
    uv: TileUv,
    material: f32,
    tint: [f32; 3],
) {
    push_oriented_box_faces(
        mesh, center, tangent, bitangent, normal, half, [uv; 6], material, tint,
    );
}

fn push_component_box(
    mesh: &mut MeshData,
    center: Vector3<f32>,
    tangent: Vector3<f32>,
    bitangent: Vector3<f32>,
    normal: Vector3<f32>,
    half: [f32; 3],
    side_uv: TileUv,
    top_uv: TileUv,
    material: f32,
    tint: [f32; 3],
) {
    let mut face_uvs = [side_uv; 6];
    face_uvs[1] = top_uv;
    push_oriented_box_faces(
        mesh, center, tangent, bitangent, normal, half, face_uvs, material, tint,
    );
}

fn push_oriented_box_faces(
    mesh: &mut MeshData,
    center: Vector3<f32>,
    tangent: Vector3<f32>,
    bitangent: Vector3<f32>,
    normal: Vector3<f32>,
    half: [f32; 3],
    face_uvs: [TileUv; 6],
    material: f32,
    tint: [f32; 3],
) {
    let hx = half[0].max(1e-4);
    let hy = half[1].max(1e-4);
    let hz = half[2].max(1e-4);

    let corners_local = [
        Vector3::new(-hx, -hy, -hz),
        Vector3::new(hx, -hy, -hz),
        Vector3::new(hx, hy, -hz),
        Vector3::new(-hx, hy, -hz),
        Vector3::new(-hx, -hy, hz),
        Vector3::new(hx, -hy, hz),
        Vector3::new(hx, hy, hz),
        Vector3::new(-hx, hy, hz),
    ];
    let corners_world = corners_local
        .map(|local| center + tangent * local.x + bitangent * local.y + normal * local.z);

    let face_indices = [
        [1usize, 0usize, 3usize, 2usize],
        [4usize, 5usize, 6usize, 7usize],
        [5usize, 1usize, 2usize, 6usize],
        [0usize, 4usize, 7usize, 3usize],
        [3usize, 7usize, 6usize, 2usize],
        [0usize, 1usize, 5usize, 4usize],
    ];
    let normals = [-normal, normal, tangent, -tangent, bitangent, -bitangent];

    let compute_uv = |face_idx: usize, local: Vector3<f32>| -> (f32, f32) {
        match face_idx {
            0 | 1 => (safe_ratio(local.x, hx), safe_ratio(local.y, hy)),
            2 | 3 => (safe_ratio(local.z, hz), safe_ratio(local.y, hy)),
            _ => (safe_ratio(local.x, hx), safe_ratio(local.z, hz)),
        }
    };

    for (face_idx, indices) in face_indices.iter().enumerate() {
        let (u_min, u_max, v_min, v_max) = face_uvs[face_idx];
        let du = u_max - u_min;
        let dv = v_max - v_min;
        let normal_vec = normals[face_idx];
        let normal_arr = [normal_vec.x, normal_vec.y, normal_vec.z];

        let local0 = corners_local[indices[0]];
        let local1 = corners_local[indices[1]];
        let local2 = corners_local[indices[2]];
        let local3 = corners_local[indices[3]];

        let world0 = corners_world[indices[0]];
        let world1 = corners_world[indices[1]];
        let world2 = corners_world[indices[2]];
        let world3 = corners_world[indices[3]];

        let (u0, v0) = compute_uv(face_idx, local0);
        let (u1, v1) = compute_uv(face_idx, local1);
        let (u2, v2) = compute_uv(face_idx, local2);
        let (u3, v3) = compute_uv(face_idx, local3);

        let vertex0 = Vertex {
            position: [world0.x, world0.y, world0.z],
            normal: normal_arr,
            uv: [u_min + u0 * du, v_min + v0 * dv],
            material,
            tint,
        };
        let vertex1 = Vertex {
            position: [world1.x, world1.y, world1.z],
            normal: normal_arr,
            uv: [u_min + u1 * du, v_min + v1 * dv],
            material,
            tint,
        };
        let vertex2 = Vertex {
            position: [world2.x, world2.y, world2.z],
            normal: normal_arr,
            uv: [u_min + u2 * du, v_min + v2 * dv],
            material,
            tint,
        };
        let vertex3 = Vertex {
            position: [world3.x, world3.y, world3.z],
            normal: normal_arr,
            uv: [u_min + u3 * du, v_min + v3 * dv],
            material,
            tint,
        };

        mesh.push_quad([vertex0, vertex1, vertex2, vertex3]);
    }
}

fn component_basis(axis: Axis, face: BlockFace) -> (Vector3<f32>, Vector3<f32>, Vector3<f32>) {
    let mut normal = face.normal_f32();
    normal = normal.normalize();
    let mut tangent = axis.as_dir();
    tangent -= normal * tangent.dot(normal);
    if tangent.magnitude2() < 1e-5 {
        tangent = if normal.x.abs() < 0.5 {
            Vector3::new(1.0, 0.0, 0.0)
        } else {
            Vector3::new(0.0, 0.0, 1.0)
        };
        tangent -= normal * tangent.dot(normal);
    }
    if tangent.magnitude2() < 1e-5 {
        tangent = normal.cross(Vector3::new(0.0, 1.0, 0.0));
        if tangent.magnitude2() < 1e-5 {
            tangent = normal.cross(Vector3::new(1.0, 0.0, 0.0));
        }
    }
    tangent = tangent.normalize();
    let mut bitangent = normal.cross(tangent);
    if bitangent.magnitude2() < 1e-5 {
        bitangent = tangent.cross(normal);
    }
    bitangent = bitangent.normalize();
    (normal, tangent, bitangent)
}

fn scaled(value: f32, scale: f32) -> f32 {
    (value * scale).max(0.002)
}

fn safe_ratio(value: f32, extent: f32) -> f32 {
    if extent.abs() < 1e-5 {
        0.5
    } else {
        (value / extent + 1.0) * 0.5
    }
}
