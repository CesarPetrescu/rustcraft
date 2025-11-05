#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockType {
    Air,
    Grass,
    Dirt,
    Stone,
    Wood,
    Sand,
    Leaves,
    CoalOre,
    IronOre,
    Water,
    FlowerRose,
    FlowerTulip,
    GlowShroom,
    CaveCrystal,
    CaveMoss,
    Terracotta,
    LilyPad,
    Snow,
    CopperWire,
    Resistor,
    VoltageSource,
    Ground,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlockFace {
    Top,
    Bottom,
    North,
    South,
    East,
    West,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub fn pair_indices(self) -> (usize, usize) {
        match self {
            Axis::X => (0, 1),
            Axis::Y => (2, 3),
            Axis::Z => (4, 5),
        }
    }

    pub fn from_connector_index(idx: usize) -> Self {
        match idx {
            0 | 1 => Axis::X,
            2 | 3 => Axis::Y,
            4 | 5 => Axis::Z,
            _ => Axis::X,
        }
    }

    pub fn to_index(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }

    pub fn all() -> [Axis; 3] {
        [Axis::X, Axis::Y, Axis::Z]
    }
    pub fn positive_face(self) -> BlockFace {
        match self {
            Axis::X => BlockFace::East,
            Axis::Y => BlockFace::Top,
            Axis::Z => BlockFace::South,
        }
    }

    pub fn negative_face(self) -> BlockFace {
        match self {
            Axis::X => BlockFace::West,
            Axis::Y => BlockFace::Bottom,
            Axis::Z => BlockFace::North,
        }
    }

    pub fn as_dir(self) -> cgmath::Vector3<f32> {
        match self {
            Axis::X => cgmath::Vector3::new(1.0, 0.0, 0.0),
            Axis::Y => cgmath::Vector3::new(0.0, 1.0, 0.0),
            Axis::Z => cgmath::Vector3::new(0.0, 0.0, 1.0),
        }
    }
}

impl BlockFace {
    pub fn axis(self) -> Axis {
        match self {
            BlockFace::Top | BlockFace::Bottom => Axis::Y,
            BlockFace::North | BlockFace::South => Axis::Z,
            BlockFace::East | BlockFace::West => Axis::X,
        }
    }

    pub fn normal(self) -> cgmath::Vector3<i32> {
        match self {
            BlockFace::Top => cgmath::Vector3::new(0, 1, 0),
            BlockFace::Bottom => cgmath::Vector3::new(0, -1, 0),
            BlockFace::North => cgmath::Vector3::new(0, 0, -1),
            BlockFace::South => cgmath::Vector3::new(0, 0, 1),
            BlockFace::East => cgmath::Vector3::new(1, 0, 0),
            BlockFace::West => cgmath::Vector3::new(-1, 0, 0),
        }
    }

    pub fn normal_f32(self) -> cgmath::Vector3<f32> {
        let n = self.normal();
        cgmath::Vector3::new(n.x as f32, n.y as f32, n.z as f32)
    }

    pub fn sign(self) -> f32 {
        match self {
            BlockFace::Top | BlockFace::South | BlockFace::East => 1.0,
            BlockFace::Bottom | BlockFace::North | BlockFace::West => -1.0,
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            BlockFace::Top => BlockFace::Bottom,
            BlockFace::Bottom => BlockFace::Top,
            BlockFace::North => BlockFace::South,
            BlockFace::South => BlockFace::North,
            BlockFace::East => BlockFace::West,
            BlockFace::West => BlockFace::East,
        }
    }

    pub fn from_normal(normal: cgmath::Vector3<i32>) -> Option<Self> {
        match (normal.x, normal.y, normal.z) {
            (0, 1, 0) => Some(BlockFace::Top),
            (0, -1, 0) => Some(BlockFace::Bottom),
            (0, 0, -1) => Some(BlockFace::North),
            (0, 0, 1) => Some(BlockFace::South),
            (1, 0, 0) => Some(BlockFace::East),
            (-1, 0, 0) => Some(BlockFace::West),
            _ => None,
        }
    }

    pub fn from_normal_f32(normal: cgmath::Vector3<f32>) -> Option<Self> {
        let x = normal.x.round() as i32;
        let y = normal.y.round() as i32;
        let z = normal.z.round() as i32;
        Self::from_normal(cgmath::Vector3::new(x, y, z))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElectricalKind {
    Wire,
    Resistor,
    VoltageSource,
    Ground,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderKind {
    Solid,
    #[allow(dead_code)]
    Cross,
    Flat,
    Flower,
    Electrical(ElectricalKind),
}

#[derive(Clone, Copy, Debug)]
pub struct TextureRule {
    default: (u32, u32),
    top: Option<(u32, u32)>,
    bottom: Option<(u32, u32)>,
}

impl TextureRule {
    pub const fn uniform(default: (u32, u32)) -> Self {
        Self {
            default,
            top: None,
            bottom: None,
        }
    }

    pub const fn with_top_bottom(default: (u32, u32), top: (u32, u32), bottom: (u32, u32)) -> Self {
        Self {
            default,
            top: Some(top),
            bottom: Some(bottom),
        }
    }

    pub fn face(&self, face: BlockFace) -> (u32, u32) {
        match face {
            BlockFace::Top => self.top.unwrap_or(self.default),
            BlockFace::Bottom => self.bottom.unwrap_or(self.default),
            _ => self.default,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BlockInfo {
    pub name: &'static str,
    pub is_solid: bool,
    pub occludes: bool,
    pub hardness: f32,
    pub light_emission: f32,
    pub textures: TextureRule,
    pub render_kind: RenderKind,
}

pub const VARIANT_COUNT: usize = 22;

const BLOCK_INFOS: [BlockInfo; VARIANT_COUNT] = [
    BlockInfo {
        name: "Air",
        is_solid: false,
        occludes: false,
        hardness: 0.0,
        light_emission: 0.0,
        textures: TextureRule::uniform((0, 0)),
        render_kind: RenderKind::Solid,
    },
    BlockInfo {
        name: "Grass",
        is_solid: true,
        occludes: true,
        hardness: 0.6,
        light_emission: 0.0,
        textures: TextureRule::with_top_bottom((1, 0), (0, 0), (2, 0)),
        render_kind: RenderKind::Solid,
    },
    BlockInfo {
        name: "Dirt",
        is_solid: true,
        occludes: true,
        hardness: 0.5,
        light_emission: 0.0,
        textures: TextureRule::uniform((2, 0)),
        render_kind: RenderKind::Solid,
    },
    BlockInfo {
        name: "Stone",
        is_solid: true,
        occludes: true,
        hardness: 1.5,
        light_emission: 0.0,
        textures: TextureRule::uniform((3, 0)),
        render_kind: RenderKind::Solid,
    },
    BlockInfo {
        name: "Wood",
        is_solid: true,
        occludes: true,
        hardness: 1.0,
        light_emission: 0.0,
        textures: TextureRule::with_top_bottom((4, 0), (5, 0), (5, 0)),
        render_kind: RenderKind::Solid,
    },
    BlockInfo {
        name: "Sand",
        is_solid: true,
        occludes: true,
        hardness: 0.5,
        light_emission: 0.0,
        textures: TextureRule::uniform((6, 0)),
        render_kind: RenderKind::Solid,
    },
    BlockInfo {
        name: "Leaves",
        is_solid: true,
        occludes: false,
        hardness: 0.2,
        light_emission: 0.0,
        textures: TextureRule::uniform((7, 0)),
        render_kind: RenderKind::Solid,
    },
    BlockInfo {
        name: "Coal Ore",
        is_solid: true,
        occludes: true,
        hardness: 1.2,
        light_emission: 0.0,
        textures: TextureRule::uniform((8, 0)),
        render_kind: RenderKind::Solid,
    },
    BlockInfo {
        name: "Iron Ore",
        is_solid: true,
        occludes: true,
        hardness: 1.4,
        light_emission: 0.0,
        textures: TextureRule::uniform((9, 0)),
        render_kind: RenderKind::Solid,
    },
    BlockInfo {
        name: "Water",
        is_solid: false,
        occludes: false,
        hardness: 0.0,
        light_emission: 0.0,
        textures: TextureRule::uniform((10, 0)),
        render_kind: RenderKind::Solid,
    },
    BlockInfo {
        name: "Rose",
        is_solid: false,
        occludes: false,
        hardness: 0.0,
        light_emission: 0.0,
        textures: TextureRule::uniform((11, 0)),
        render_kind: RenderKind::Flower,
    },
    BlockInfo {
        name: "Tulip",
        is_solid: false,
        occludes: false,
        hardness: 0.0,
        light_emission: 0.0,
        textures: TextureRule::uniform((12, 0)),
        render_kind: RenderKind::Flower,
    },
    BlockInfo {
        name: "Glow Shroom",
        is_solid: false,
        occludes: false,
        hardness: 0.0,
        light_emission: 0.1,
        textures: TextureRule::uniform((38, 0)),
        render_kind: RenderKind::Flower,
    },
    BlockInfo {
        name: "Cave Crystal",
        is_solid: true,
        occludes: true,
        hardness: 0.6,
        light_emission: 0.2,
        textures: TextureRule::uniform((15, 0)),
        render_kind: RenderKind::Cross,
    },
    BlockInfo {
        name: "Cave Moss",
        is_solid: true,
        occludes: true,
        hardness: 0.25,
        light_emission: 0.0,
        textures: TextureRule::uniform((6, 0)),
        render_kind: RenderKind::Solid,
    },
    BlockInfo {
        name: "Terracotta",
        is_solid: true,
        occludes: true,
        hardness: 1.4,
        light_emission: 0.0,
        textures: TextureRule::uniform((13, 0)),
        render_kind: RenderKind::Solid,
    },
    BlockInfo {
        name: "Lily Pad",
        is_solid: false,
        occludes: false,
        hardness: 0.0,
        light_emission: 0.0,
        textures: TextureRule::uniform((14, 0)),
        render_kind: RenderKind::Flat,
    },
    BlockInfo {
        name: "Snow",
        is_solid: true,
        occludes: true,
        hardness: 0.2,
        light_emission: 0.0,
        textures: TextureRule::uniform((15, 0)),
        render_kind: RenderKind::Solid,
    },
    BlockInfo {
        name: "Copper Wire",
        is_solid: false,
        occludes: false,
        hardness: 0.0,
        light_emission: 0.0,
        textures: TextureRule::uniform((16, 0)),
        render_kind: RenderKind::Electrical(ElectricalKind::Wire),
    },
    BlockInfo {
        name: "Resistor",
        is_solid: false,
        occludes: false,
        hardness: 0.0,
        light_emission: 0.0,
        textures: TextureRule::uniform((17, 0)),
        render_kind: RenderKind::Electrical(ElectricalKind::Resistor),
    },
    BlockInfo {
        name: "Voltage Source",
        is_solid: false,
        occludes: false,
        hardness: 0.0,
        light_emission: 0.0,
        textures: TextureRule::uniform((18, 0)),
        render_kind: RenderKind::Electrical(ElectricalKind::VoltageSource),
    },
    BlockInfo {
        name: "Ground Node",
        is_solid: false,
        occludes: false,
        hardness: 0.0,
        light_emission: 0.0,
        textures: TextureRule::uniform((19, 0)),
        render_kind: RenderKind::Electrical(ElectricalKind::Ground),
    },
];

impl BlockType {
    fn info(self) -> &'static BlockInfo {
        &BLOCK_INFOS[self as usize]
    }

    pub fn is_solid(self) -> bool {
        self.info().is_solid
    }

    pub fn occludes(self) -> bool {
        self.info().occludes
    }

    pub fn name(self) -> &'static str {
        self.info().name
    }

    #[allow(dead_code)]
    pub fn hardness(self) -> f32 {
        self.info().hardness
    }

    #[allow(dead_code)]
    pub fn light_emission(self) -> f32 {
        self.info().light_emission
    }

    pub fn atlas_coords(self, face: BlockFace) -> (u32, u32) {
        self.info().textures.face(face)
    }

    pub fn render_kind(self) -> RenderKind {
        self.info().render_kind
    }

    pub fn electrical_kind(self) -> Option<ElectricalKind> {
        match self {
            BlockType::CopperWire => Some(ElectricalKind::Wire),
            BlockType::Resistor => Some(ElectricalKind::Resistor),
            BlockType::VoltageSource => Some(ElectricalKind::VoltageSource),
            BlockType::Ground => Some(ElectricalKind::Ground),
            _ => None,
        }
    }

    pub fn is_electrical(self) -> bool {
        self.electrical_kind().is_some()
    }

    pub fn default_axis(self) -> Axis {
        match self.electrical_kind() {
            Some(ElectricalKind::VoltageSource) => Axis::X,
            Some(ElectricalKind::Ground) => Axis::Y,
            Some(ElectricalKind::Wire) | Some(ElectricalKind::Resistor) => Axis::X,
            None => Axis::X,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub block_type: BlockType,
}

impl Block {
    pub const fn new(block_type: BlockType) -> Self {
        Self { block_type }
    }
}

impl Default for Block {
    fn default() -> Self {
        Block::new(BlockType::Air)
    }
}
