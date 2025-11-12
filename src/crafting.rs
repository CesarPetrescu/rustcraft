use crate::block::BlockType;
use crate::item::{ItemType, ToolType};

/// Represents a crafting recipe
#[derive(Clone, Debug)]
pub struct Recipe {
    pub pattern: RecipePattern,
    pub output: ItemType,
    pub output_count: usize,
}

/// Recipe pattern types
#[derive(Clone, Debug)]
pub enum RecipePattern {
    /// Shaped recipe - pattern matters (3x3 grid)
    Shaped(Vec<Vec<Option<RecipeIngredient>>>),
    /// Shapeless recipe - only ingredients matter, not position
    Shapeless(Vec<RecipeIngredient>),
}

/// Ingredient in a recipe
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecipeIngredient {
    Block(BlockType),
    Tool(ToolType),
    /// Matches any item type (wildcard)
    Any,
}

impl RecipeIngredient {
    pub fn matches(&self, item: &ItemType) -> bool {
        match (self, item) {
            (RecipeIngredient::Block(recipe_block), ItemType::Block(item_block)) => {
                recipe_block == item_block
            }
            (RecipeIngredient::Tool(recipe_tool), ItemType::Tool(item_tool, _)) => {
                recipe_tool == item_tool
            }
            (RecipeIngredient::Any, _) => true,
            _ => false,
        }
    }
}

pub struct CraftingSystem {
    recipes: Vec<Recipe>,
}

impl CraftingSystem {
    pub fn new() -> Self {
        let mut system = Self {
            recipes: Vec::new(),
        };
        system.register_default_recipes();
        system
    }

    /// Check if a crafting grid matches any recipe
    pub fn match_recipe(&self, grid: &[Option<ItemType>; 9]) -> Option<(ItemType, usize)> {
        for recipe in &self.recipes {
            if self.matches_pattern(&recipe.pattern, grid) {
                return Some((recipe.output, recipe.output_count));
            }
        }
        None
    }

    fn matches_pattern(&self, pattern: &RecipePattern, grid: &[Option<ItemType>; 9]) -> bool {
        match pattern {
            RecipePattern::Shaped(shape) => self.matches_shaped(shape, grid),
            RecipePattern::Shapeless(ingredients) => self.matches_shapeless(ingredients, grid),
        }
    }

    fn matches_shaped(&self, shape: &[Vec<Option<RecipeIngredient>>], grid: &[Option<ItemType>; 9]) -> bool {
        // Convert 1D grid to 2D
        let grid_2d: Vec<Vec<Option<ItemType>>> = vec![
            vec![grid[0], grid[1], grid[2]],
            vec![grid[3], grid[4], grid[5]],
            vec![grid[6], grid[7], grid[8]],
        ];

        // Check if pattern fits in grid (with offset)
        for y_offset in 0..=(3 - shape.len()) {
            for x_offset in 0..=(3 - shape[0].len()) {
                if self.check_shaped_at_offset(shape, &grid_2d, x_offset, y_offset) {
                    return true;
                }
            }
        }
        false
    }

    fn check_shaped_at_offset(
        &self,
        shape: &[Vec<Option<RecipeIngredient>>],
        grid: &[Vec<Option<ItemType>>],
        x_offset: usize,
        y_offset: usize,
    ) -> bool {
        // Check all cells in the grid
        for y in 0..3 {
            for x in 0..3 {
                let in_pattern = y >= y_offset
                    && y < y_offset + shape.len()
                    && x >= x_offset
                    && x < x_offset + shape[0].len();

                if in_pattern {
                    let pattern_y = y - y_offset;
                    let pattern_x = x - x_offset;
                    let required = &shape[pattern_y][pattern_x];
                    let actual = &grid[y][x];

                    match (required, actual) {
                        (Some(ingredient), Some(item)) => {
                            if !ingredient.matches(item) {
                                return false;
                            }
                        }
                        (Some(_), None) => return false,
                        (None, Some(_)) => return false,
                        (None, None) => {}
                    }
                } else {
                    // Outside pattern - must be empty
                    if grid[y][x].is_some() {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn matches_shapeless(
        &self,
        ingredients: &[RecipeIngredient],
        grid: &[Option<ItemType>; 9],
    ) -> bool {
        let required = ingredients.to_vec();
        let mut available: Vec<ItemType> = grid.iter().filter_map(|x| *x).collect();

        if required.len() != available.len() {
            return false;
        }

        // Try to match each required ingredient
        for ingredient in &required {
            let found = available.iter().position(|item| ingredient.matches(item));
            if let Some(idx) = found {
                available.remove(idx);
            } else {
                return false;
            }
        }

        available.is_empty()
    }

    fn register_default_recipes(&mut self) {
        use BlockType::*;
        use RecipeIngredient::Block as B;
        use ToolType::*;

        // Wood → 4 Planks (shapeless)
        self.recipes.push(Recipe {
            pattern: RecipePattern::Shapeless(vec![B(Wood)]),
            output: ItemType::Block(Wood), // TODO: Add Plank block type
            output_count: 4,
        });

        // 2 Planks → 4 Sticks (vertical pattern)
        self.recipes.push(Recipe {
            pattern: RecipePattern::Shaped(vec![
                vec![Some(B(Wood))],
                vec![Some(B(Wood))],
            ]),
            output: ItemType::Block(Wood), // TODO: Add Stick item
            output_count: 4,
        });

        // Wooden Pickaxe: 3 planks + 2 sticks
        self.recipes.push(Recipe {
            pattern: RecipePattern::Shaped(vec![
                vec![Some(B(Wood)), Some(B(Wood)), Some(B(Wood))],
                vec![None, Some(B(Wood)), None],
                vec![None, Some(B(Wood)), None],
            ]),
            output: ItemType::Tool(WoodenPickaxe, WoodenPickaxe.max_durability()),
            output_count: 1,
        });

        // Stone Pickaxe: 3 stone + 2 sticks
        self.recipes.push(Recipe {
            pattern: RecipePattern::Shaped(vec![
                vec![Some(B(Stone)), Some(B(Stone)), Some(B(Stone))],
                vec![None, Some(B(Wood)), None],
                vec![None, Some(B(Wood)), None],
            ]),
            output: ItemType::Tool(StonePickaxe, StonePickaxe.max_durability()),
            output_count: 1,
        });

        // Iron Pickaxe: 3 iron ore + 2 sticks
        self.recipes.push(Recipe {
            pattern: RecipePattern::Shaped(vec![
                vec![Some(B(IronOre)), Some(B(IronOre)), Some(B(IronOre))],
                vec![None, Some(B(Wood)), None],
                vec![None, Some(B(Wood)), None],
            ]),
            output: ItemType::Tool(IronPickaxe, IronPickaxe.max_durability()),
            output_count: 1,
        });

        // Wooden Axe
        self.recipes.push(Recipe {
            pattern: RecipePattern::Shaped(vec![
                vec![Some(B(Wood)), Some(B(Wood))],
                vec![Some(B(Wood)), None],
                vec![None, Some(B(Wood))],
            ]),
            output: ItemType::Tool(WoodenAxe, WoodenAxe.max_durability()),
            output_count: 1,
        });

        // Stone Axe
        self.recipes.push(Recipe {
            pattern: RecipePattern::Shaped(vec![
                vec![Some(B(Stone)), Some(B(Stone))],
                vec![Some(B(Wood)), None],
                vec![Some(B(Wood)), None],
            ]),
            output: ItemType::Tool(StoneAxe, StoneAxe.max_durability()),
            output_count: 1,
        });

        // Wooden Shovel: 1 plank + 2 sticks
        self.recipes.push(Recipe {
            pattern: RecipePattern::Shaped(vec![
                vec![Some(B(Wood))],
                vec![Some(B(Wood))],
                vec![Some(B(Wood))],
            ]),
            output: ItemType::Tool(WoodenShovel, WoodenShovel.max_durability()),
            output_count: 1,
        });

        // Stone Shovel
        self.recipes.push(Recipe {
            pattern: RecipePattern::Shaped(vec![
                vec![Some(B(Stone))],
                vec![Some(B(Wood))],
                vec![Some(B(Wood))],
            ]),
            output: ItemType::Tool(StoneShovel, StoneShovel.max_durability()),
            output_count: 1,
        });

        // Torch: 1 coal + 1 stick → 4 torches
        self.recipes.push(Recipe {
            pattern: RecipePattern::Shaped(vec![
                vec![Some(B(CoalOre))],
                vec![Some(B(Wood))],
            ]),
            output: ItemType::Block(Torch),
            output_count: 4,
        });
    }

    pub fn recipe_count(&self) -> usize {
        self.recipes.len()
    }
}
