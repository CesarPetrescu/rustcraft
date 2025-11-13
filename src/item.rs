use crate::block::BlockType;

/// Represents an item that can be held in inventory
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemType {
    Block(BlockType),
    Tool(ToolType, u32), // ToolType and current durability
    Material(MaterialType),
}

/// Material items (crafting ingredients that aren't blocks)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialType {
    Plank,
    Stick,
}

impl MaterialType {
    pub fn name(&self) -> &'static str {
        match self {
            MaterialType::Plank => "Plank",
            MaterialType::Stick => "Stick",
        }
    }
}

impl ItemType {
    pub fn name(&self) -> &'static str {
        match self {
            ItemType::Block(block) => block.name(),
            ItemType::Tool(tool, _) => tool.name(),
            ItemType::Material(material) => material.name(),
        }
    }

    /// Get the maximum durability for this item (0 for blocks/unbreakable)
    pub fn max_durability(&self) -> u32 {
        match self {
            ItemType::Block(_) => 0,
            ItemType::Tool(tool, _) => tool.max_durability(),
            ItemType::Material(_) => 0,
        }
    }

    /// Get current durability (for tools)
    pub fn durability(&self) -> u32 {
        match self {
            ItemType::Block(_) => 0,
            ItemType::Tool(_, dur) => *dur,
            ItemType::Material(_) => 0,
        }
    }

    /// Check if this tool is effective against a block type
    pub fn is_effective_for(&self, block: BlockType) -> bool {
        match self {
            ItemType::Block(_) => false, // Blocks don't mine faster
            ItemType::Tool(tool, _) => tool.is_effective_for(block),
            ItemType::Material(_) => false,
        }
    }

    /// Get the mining speed multiplier
    pub fn mining_speed_multiplier(&self) -> f32 {
        match self {
            ItemType::Block(_) => 1.0, // Hand speed
            ItemType::Tool(tool, _) => tool.mining_speed_multiplier(),
            ItemType::Material(_) => 1.0,
        }
    }

    /// Damage the tool by 1 durability, returns true if tool breaks
    pub fn damage(&mut self) -> bool {
        match self {
            ItemType::Block(_) => false,
            ItemType::Tool(_, dur) => {
                if *dur > 0 {
                    *dur -= 1;
                    *dur == 0 // Tool broke
                } else {
                    true // Already broken
                }
            }
            ItemType::Material(_) => false,
        }
    }
}

/// Tool types with different purposes and tiers
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolType {
    // Pickaxes (mine stone, ores faster)
    WoodenPickaxe,
    StonePickaxe,
    IronPickaxe,
    DiamondPickaxe,

    // Axes (mine wood faster)
    WoodenAxe,
    StoneAxe,
    IronAxe,
    DiamondAxe,

    // Shovels (mine dirt, sand, gravel faster)
    WoodenShovel,
    StoneShovel,
    IronShovel,
    DiamondShovel,

    // Swords (combat, but can also break blocks)
    WoodenSword,
    StoneSword,
    IronSword,
    DiamondSword,
}

impl ToolType {
    pub fn name(&self) -> &'static str {
        match self {
            ToolType::WoodenPickaxe => "Wooden Pickaxe",
            ToolType::StonePickaxe => "Stone Pickaxe",
            ToolType::IronPickaxe => "Iron Pickaxe",
            ToolType::DiamondPickaxe => "Diamond Pickaxe",
            ToolType::WoodenAxe => "Wooden Axe",
            ToolType::StoneAxe => "Stone Axe",
            ToolType::IronAxe => "Iron Axe",
            ToolType::DiamondAxe => "Diamond Axe",
            ToolType::WoodenShovel => "Wooden Shovel",
            ToolType::StoneShovel => "Stone Shovel",
            ToolType::IronShovel => "Iron Shovel",
            ToolType::DiamondShovel => "Diamond Shovel",
            ToolType::WoodenSword => "Wooden Sword",
            ToolType::StoneSword => "Stone Sword",
            ToolType::IronSword => "Iron Sword",
            ToolType::DiamondSword => "Diamond Sword",
        }
    }

    pub fn max_durability(&self) -> u32 {
        match self {
            // Wooden tools: 59
            ToolType::WoodenPickaxe | ToolType::WoodenAxe
            | ToolType::WoodenShovel | ToolType::WoodenSword => 59,

            // Stone tools: 131
            ToolType::StonePickaxe | ToolType::StoneAxe
            | ToolType::StoneShovel | ToolType::StoneSword => 131,

            // Iron tools: 250
            ToolType::IronPickaxe | ToolType::IronAxe
            | ToolType::IronShovel | ToolType::IronSword => 250,

            // Diamond tools: 1561
            ToolType::DiamondPickaxe | ToolType::DiamondAxe
            | ToolType::DiamondShovel | ToolType::DiamondSword => 1561,
        }
    }

    pub fn mining_speed_multiplier(&self) -> f32 {
        match self {
            // Pickaxes
            ToolType::WoodenPickaxe => 2.0,
            ToolType::StonePickaxe => 4.0,
            ToolType::IronPickaxe => 6.0,
            ToolType::DiamondPickaxe => 8.0,

            // Axes
            ToolType::WoodenAxe => 2.0,
            ToolType::StoneAxe => 4.0,
            ToolType::IronAxe => 6.0,
            ToolType::DiamondAxe => 8.0,

            // Shovels
            ToolType::WoodenShovel => 2.0,
            ToolType::StoneShovel => 4.0,
            ToolType::IronShovel => 6.0,
            ToolType::DiamondShovel => 8.0,

            // Swords (not great for mining, but usable)
            ToolType::WoodenSword => 1.5,
            ToolType::StoneSword => 1.5,
            ToolType::IronSword => 1.5,
            ToolType::DiamondSword => 1.5,
        }
    }

    pub fn is_effective_for(&self, block: BlockType) -> bool {
        match self {
            // Pickaxes are effective for stone and ores
            ToolType::WoodenPickaxe | ToolType::StonePickaxe
            | ToolType::IronPickaxe | ToolType::DiamondPickaxe => {
                matches!(
                    block,
                    BlockType::Stone
                        | BlockType::CoalOre
                        | BlockType::IronOre
                        | BlockType::Terracotta
                        | BlockType::Snow
                )
            }

            // Axes are effective for wood
            ToolType::WoodenAxe | ToolType::StoneAxe
            | ToolType::IronAxe | ToolType::DiamondAxe => {
                matches!(block, BlockType::Wood)
            }

            // Shovels are effective for dirt, sand, gravel
            ToolType::WoodenShovel | ToolType::StoneShovel
            | ToolType::IronShovel | ToolType::DiamondShovel => {
                matches!(block, BlockType::Dirt | BlockType::Sand | BlockType::Grass)
            }

            // Swords are not particularly effective for any block
            ToolType::WoodenSword | ToolType::StoneSword
            | ToolType::IronSword | ToolType::DiamondSword => false,
        }
    }

    /// Get the tool category
    pub fn category(&self) -> ToolCategory {
        match self {
            ToolType::WoodenPickaxe | ToolType::StonePickaxe
            | ToolType::IronPickaxe | ToolType::DiamondPickaxe => ToolCategory::Pickaxe,

            ToolType::WoodenAxe | ToolType::StoneAxe
            | ToolType::IronAxe | ToolType::DiamondAxe => ToolCategory::Axe,

            ToolType::WoodenShovel | ToolType::StoneShovel
            | ToolType::IronShovel | ToolType::DiamondShovel => ToolCategory::Shovel,

            ToolType::WoodenSword | ToolType::StoneSword
            | ToolType::IronSword | ToolType::DiamondSword => ToolCategory::Sword,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolCategory {
    Pickaxe,
    Axe,
    Shovel,
    Sword,
}
