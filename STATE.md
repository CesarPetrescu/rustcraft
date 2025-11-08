# Project State

## Overview
- **Current build**: Single-player voxel sandbox with procedural terrain (grass/dirt/stone/sand, trees), ore veins, biome-tinted skies, tall-world height pass (~y≈150 surface, 256 cap), basic block placing/breaking, hotbar inventory, sprinting with dynamic FOV, noclip toggle, textured rendering.
- **Major gaps**: No crafting/tools, no mobs, single biome, no time-of-day or weather, no UI beyond hotbar, no persistence or advanced performance features.

---

## Core Variety
- **Done**: Climate-driven terrain generator spanning plains, desert, forest, mountain, swamp, tundra, jungle, mesa, savanna, taiga, and meadow biomes with tailored height scales, sky palettes, and continental influence; carved rivers feed the limited-flow water system while layered cave networks now concentrate underground; grass/dirt/stone/sand/wood/leaves/snow/terracotta blocks, coal/iron ore variants, decorative flowers and lily pads, procedural texture atlas.
- **Missing**:
  - Additional block types (liquids, glass, light-emitting blocks, specialty resources).
  - Deeper biome identity (unique vegetation props, small structures, sky/fog tricks) and rare edge biomes.
  - Craft/placement loops for new materials.
- **Plan**:
  1. Extend block registry to support metadata (hardness, light level, variant textures).
  2. Flesh out biome sub-variants (structures, vegetation props, fog/sky tweaks) on top of the new climate lattice.
  3. Add resource nodes (ore distribution, water bodies) with appropriate mesh/physics tweaks.
- **Ideas**: Seasonal palette swaps, rare structures (villages, ruins), animated blocks (water/lava surface).

## Player Progression
- **Done**: Hotbar inventory, manual block placement/removal.
- **Missing**:
  - Tool items with durability and harvesting tiers.
  - Crafting UI/recipes, resource collection feedback.
  - Adventure hooks (loot chests, structures).
- **Plan**:
  1. Introduce item system (tools, blocks, consumables) with durability tracking.
  2. Build minimal crafting interface (2×2 or crafting table) + recipe data.
  3. Scatter simple structures containing loot tables; tie access to crafted tools.
- **Ideas**: Skill progression (mining speed upgrades), quest log, achievements.

## Mob Ecosystem
- **Done**: None.
- **Missing**:
  - Entity framework (position, physics, AI state).
  - Passive/hostile mob definitions with spawn rules.
  - Combat + drops.
- **Plan**:
  1. Implement entity system and update loop decoupled from blocks.
  2. Add simple AI behaviors (wander, avoid edges, chase/attack).
  3. Integrate spawn manager using biomes/time-of-day, plus loot tables.
- **Ideas**: Taming/breeding mechanics, boss encounters, NPC traders.

## World Interaction
- **Done**: Gravity on player (collision), block updates via player interaction only.
- **Missing**:
  - Day/night cycle, weather, plant growth, fluids, redstone-like logic.
- **Plan**:
  1. Add global time keeper + sky/lighting gradients.
  2. Implement block tick system for crops/fluids.
  3. Introduce weather effects (rain/snow) influencing crops or mob spawns.
- **Ideas**: Temperature map, erosion, electricity/logic blocks.

## Polish & UX
- **Done**: Basic HUD (selected block printouts), mouse grab toggle, smoother movement with sprint (Left Ctrl) and dynamic FOV response, crosshair overlay, pause menu with input locking, simple inventory panel overlay.
- **Missing**:
  - Pause/settings menu, crafting UI, minimap/compass, tutorials, audio, particles.
- **Plan**:
  1. Build UI layer (winit/wgpu overlay) for menus + inventory.
  2. Integrate audio pipeline (BGM, SFX) using rodio or similar.
  3. Add particle system for block interactions and ambient effects.
- **Ideas**: Photo mode, accessibility options, controller support.

## Tech & Performance
- **Done**: Chunk-based world storage, instanced rendering (per block), automatic mesh rebuilds when world changes.
- **Missing**:
  - Async chunk generation, distant chunk streaming, mesh LOD, save/load.
- **Plan**:
  1. Move chunk generation + tree placement to worker threads; defer mesh upload until ready.
  2. Implement streaming manager for distant chunks with adjustable radius and low-poly meshes.
  3. Persist world/player state (on-disk format) and add save slots.
- **Ideas**: Multi-threaded meshing, GPU compute-based meshing, networked multiplayer architecture.

---

## Immediate Next Steps
1. ✅ Formalized block metadata (hardness, light emission, texture rules) to support new materials.
2. ✅ Prototype biome sampling (temperature/moisture noise) to feed future systems.
3. ✅ Expanded chunk height to 256 and refactored world storage/meshing to match.
4. ✅ Rescaled terrain baseline (~y≈150 surface) with extended cavern depth and climate-aware biome blending; fluid polishing continues separately.
5. Flesh out crafting/pause UI wireframes and expand overlay to show actual item stacks/tooltips.

## Terrain & Vertical Overhaul Roadmap
1. ✅ Lift chunk height to 256 while keeping horizontal size at 16×16 (structural refactor complete).
2. ✅ Develop layered cave generation with surface openings and ensure rivers carve through walls.
3. ✅ Author additional biomes (swamp, tundra, jungle, mesa) with bespoke foliage and sky palettes (props/vines still TBD).
4. ⬜ Balance resource distribution and performance for the expanded vertical space (chunk streaming, lighting).
5. 🔄 Iterate on water physics/visuals to reduce edge distortions and support waterfalls/reservoirs (smoothing updated; needs further passes).

### Detailed Cave & Biome Plan (next focus)
- ✅ **Cave Blueprint**: layered noise caverns carved with surface openings; next add decorations (stalactites, dripstone, ambient fog).
- ✅ **Biome Palette Extensions**: Swamp, Tundra, Jungle, Mesa, Savanna, Taiga, and Meadow are live with bespoke palettes and tree tuning; next pass should add unique props (mangroves, vines, hoodoos) and fog tweaks.
- **Height Bands**: tune elevation ranges (80–140 lowland basins, 140–190 temperate belts, 190–240 highlands, 240+ peaks) to avoid abrupt biome mixing along cliffs.
- **Rivers**: increase river noise resolution, ensure minimum channel width of 2 blocks, drop waterfalls where elevation difference > 2.
- **Performance Tasks**: chunk streaming radius tuning, background meshing for deeper caves, vertical culling in renderer.

### Outstanding TODOs / Issues
- Water equalization still produces raised ridges at chunk edges; clamp corner heights and revisit smoothing shader.
- Rivers/lakes occasionally leave floating sheets when intersecting caves; tweak the fluid diffusion step.
- Biome props pass pending: add swamp-specific trees, tundra spruce variants with snow, jungle vines, mesa arches/fog tints.
- New river fill logic relies on local ground checks—keep an eye out for exposed voids or performance spikes and tighten thresholds if needed.
- Validate that the elevated baseline (+60) still leaves enough breathable underground layers without overwhelming surface biomes; adjust climate/continental noise if belts bunch together.
- Monitor the sparser cave mask to ensure underground exploration still has interesting pockets; relax thresholds slightly if layers feel empty.
- Confirm rivers stay level (max 3-block gradient), avoid cave networks, and never spawn above WATER_LEVEL+6; adjust clamps if artifacts appear.
