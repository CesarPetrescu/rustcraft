use crate::block::{Block, BlockType, RenderKind};

pub const CHUNK_SIZE: usize = 16;
pub const CHUNK_HEIGHT: usize = 256;
pub const CHUNK_AREA: usize = CHUNK_SIZE * CHUNK_SIZE;
pub const CHUNK_VOLUME: usize = CHUNK_AREA * CHUNK_HEIGHT;

pub const fn index(x: usize, y: usize, z: usize) -> usize {
    x + CHUNK_SIZE * (z + CHUNK_SIZE * y)
}

#[derive(Clone)]
pub struct Chunk {
    blocks: [Block; CHUNK_VOLUME],
    fluids: [u8; CHUNK_VOLUME],
    cell_state: Vec<i16>,
}

impl Chunk {
    pub fn new() -> Self {
        let mut chunk = Self {
            blocks: [Block::default(); CHUNK_VOLUME],
            fluids: [0; CHUNK_VOLUME],
            cell_state: vec![0; CHUNK_VOLUME],
        };
        chunk.rebuild_cell_state();
        chunk
    }

    pub fn set_block(&mut self, x: usize, y: usize, z: usize, block: BlockType) {
        if x < CHUNK_SIZE && y < CHUNK_HEIGHT && z < CHUNK_SIZE {
            let idx = index(x, y, z);
            self.blocks[idx] = Block::new(block);
            if block != BlockType::Air {
                self.fluids[idx] = 0;
            }
            self.update_cell_state(idx);
        }
    }

    pub fn get_block(&self, x: usize, y: usize, z: usize) -> BlockType {
        if x < CHUNK_SIZE && y < CHUNK_HEIGHT && z < CHUNK_SIZE {
            self.blocks[index(x, y, z)].block_type
        } else {
            BlockType::Air
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, usize, usize, BlockType)> + '_ {
        self.blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| match block.block_type {
                BlockType::Air => false,
                _ => matches!(
                    block.block_type.render_kind(),
                    RenderKind::Solid
                        | RenderKind::Cross
                        | RenderKind::Flat
                        | RenderKind::Flower
                        | RenderKind::Electrical(_)
                ),
            })
            .map(|(i, block)| {
                let y = i / CHUNK_AREA;
                let rem = i % CHUNK_AREA;
                let z = rem / CHUNK_SIZE;
                let x = rem % CHUNK_SIZE;
                (x, y, z, block.block_type)
            })
    }

    pub fn get_fluid(&self, x: usize, y: usize, z: usize) -> u8 {
        if x < CHUNK_SIZE && y < CHUNK_HEIGHT && z < CHUNK_SIZE {
            self.fluids[index(x, y, z)]
        } else {
            0
        }
    }

    pub fn set_fluid(&mut self, x: usize, y: usize, z: usize, amount: u8) {
        if x < CHUNK_SIZE && y < CHUNK_HEIGHT && z < CHUNK_SIZE {
            let idx = index(x, y, z);
            self.fluids[idx] = amount;
            if amount > 0 {
                self.blocks[idx] = Block::new(BlockType::Air);
            }
            self.update_cell_state(idx);
        }
    }

    pub fn fluids_iter(&self) -> impl Iterator<Item = (usize, usize, usize, u8)> + '_ {
        self.fluids
            .iter()
            .enumerate()
            .filter(|(_, amount)| **amount > 0)
            .map(|(i, amount)| {
                let y = i / CHUNK_AREA;
                let rem = i % CHUNK_AREA;
                let z = rem / CHUNK_SIZE;
                let x = rem % CHUNK_SIZE;
                (x, y, z, *amount)
            })
    }

    #[allow(dead_code)]
    pub fn fluids(&self) -> &[u8] {
        &self.fluids
    }

    pub fn apply_fluids(&mut self, new_fluids: &[u8]) {
        if new_fluids.len() != CHUNK_VOLUME {
            return;
        }
        self.fluids.copy_from_slice(new_fluids);
        for idx in 0..CHUNK_VOLUME {
            if self.fluids[idx] > 0 {
                self.blocks[idx] = Block::new(BlockType::Air);
            }
            self.update_cell_state(idx);
        }
    }

    pub fn cell_state(&self) -> &[i16] {
        &self.cell_state
    }

    fn update_cell_state(&mut self, idx: usize) {
        let block = self.blocks[idx].block_type;
        let fluid = self.fluids[idx];
        self.cell_state[idx] = if fluid > 0 {
            fluid as i16
        } else if block.occludes() {
            -1
        } else {
            0
        };
    }

    fn rebuild_cell_state(&mut self) {
        for idx in 0..self.cell_state.len() {
            self.update_cell_state(idx);
        }
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}
