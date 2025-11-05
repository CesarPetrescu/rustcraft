use crate::block::BlockType;

pub const HOTBAR_SIZE: usize = 9;
pub const AVAILABLE_BLOCKS: [BlockType; 17] = [
    BlockType::Grass,
    BlockType::Dirt,
    BlockType::Stone,
    BlockType::Wood,
    BlockType::Sand,
    BlockType::Leaves,
    BlockType::CoalOre,
    BlockType::IronOre,
    BlockType::Terracotta,
    BlockType::Water,
    BlockType::FlowerRose,
    BlockType::FlowerTulip,
    BlockType::Snow,
    BlockType::CopperWire,
    BlockType::Resistor,
    BlockType::VoltageSource,
    BlockType::Ground,
];

pub struct Inventory {
    pub hotbar: [Option<BlockType>; HOTBAR_SIZE],
    pub selected_slot: usize,
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            hotbar: [
                Some(BlockType::Grass),
                Some(BlockType::Dirt),
                Some(BlockType::Stone),
                Some(BlockType::CopperWire),
                Some(BlockType::Resistor),
                Some(BlockType::VoltageSource),
                Some(BlockType::Ground),
                Some(BlockType::Water),
                Some(BlockType::FlowerRose),
            ],
            selected_slot: 0,
        }
    }

    pub fn select_slot(&mut self, slot: usize) {
        if slot < HOTBAR_SIZE {
            self.selected_slot = slot;
        }
    }

    pub fn selected_block(&self) -> Option<BlockType> {
        self.hotbar[self.selected_slot]
    }

    pub fn selected_slot_index(&self) -> usize {
        self.selected_slot
    }

    pub fn cycle_selection(&mut self, delta: i32) {
        let new_slot = (self.selected_slot as i32 + delta).rem_euclid(HOTBAR_SIZE as i32);
        self.selected_slot = new_slot as usize;
    }

    pub fn swap_slots(&mut self, a: usize, b: usize) {
        if a >= HOTBAR_SIZE || b >= HOTBAR_SIZE || a == b {
            return;
        }

        self.hotbar.swap(a, b);
        if self.selected_slot == a {
            self.selected_slot = b;
        } else if self.selected_slot == b {
            self.selected_slot = a;
        }
    }

    pub fn cycle_slot_block(&mut self, slot: usize, delta: i32) {
        if slot >= HOTBAR_SIZE {
            return;
        }

        let total = AVAILABLE_BLOCKS.len() as i32;
        if total == 0 {
            return;
        }

        let current_index = self.hotbar[slot]
            .and_then(|block| {
                AVAILABLE_BLOCKS
                    .iter()
                    .position(|candidate| *candidate == block)
            })
            .unwrap_or(0) as i32;
        let next_index = (current_index + delta).rem_euclid(total) as usize;
        self.hotbar[slot] = Some(AVAILABLE_BLOCKS[next_index]);
    }

    pub fn set_slot(&mut self, slot: usize, block: Option<BlockType>) {
        if slot < HOTBAR_SIZE {
            self.hotbar[slot] = block;
        }
    }

    pub fn clear_slot(&mut self, slot: usize) {
        self.set_slot(slot, None);
    }

    pub fn first_empty_slot(&self) -> Option<usize> {
        self.hotbar.iter().position(|slot| slot.is_none())
    }
}
