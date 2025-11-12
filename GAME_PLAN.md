# Rustcraft Complete Game Plan

This document outlines the roadmap to implement all features from the original Minecraft-like game, organized into logical phases with dependencies considered.

---

## Current State (Already Implemented)

✅ Block system (22 block types)
✅ Player movement with physics (gravity, collision, jumping, sprinting)
✅ Block placement and removal (raycasting)
✅ Inventory system (9-slot hotbar + full inventory UI)
✅ World generation (11 biomes, rivers, caves, ore veins)
✅ WGPU rendering pipeline
✅ Advanced shading and atmosphere
✅ Fluid simulation (GPU-accelerated)
✅ Electrical circuit system
✅ Accessibility features
✅ Day/night cycle framework (not fully active)
✅ Crosshair UI

---

## Phase 1: Core World & Rendering Enhancements

### 1.1 Terrain Generation Improvements
- [ ] **Infinite terrain generation** - Implement chunk streaming and generation on-demand
- [ ] **Improved tree generation** - Add proper tree structures with leaves and wood
- [ ] **Grass and flowers** - Add decorative foliage on grass blocks
- [ ] **More varied cave systems** - Enhance cave generation for better exploration

### 1.2 Lighting System
- [ ] **Skylight implementation** - Light propagates from sky down through transparent blocks
- [ ] **Block light system** - Light emitted from glowstones and torches
- [ ] **Light updates on block changes** - Dynamic light recalculation
- [ ] **Slight shadows on block faces** - North & south sides darker for depth

### 1.3 Visual Polish
- [ ] **Darker tree leaves** - Adjust leaf texture/shading
- [ ] **Night time activation** - Enable full day/night cycle
- [ ] **Stars and moon rendering** - Add celestial bodies to night sky
- [ ] **Stars gradually show/hide** - Fade based on time of day
- [ ] **Drawing clouds** - Volumetric or 2D cloud layer
- [ ] **Cloud movement** - Animate clouds across sky
- [ ] **Transparent clouds** - Alpha blending for realistic appearance
- [ ] **Increased FOV** - Change from 45° to 90° field of view
- [ ] **360° main menu background** - Rotating world preview

---

## Phase 2: Player Interaction & Visual Feedback

### 2.1 Block Interaction
- [ ] **Block hover highlight** - Show which block player is looking at
- [ ] **Block breaking animation** - Crack texture overlay during breaking
- [ ] **Block breaking system** - Progressive damage to blocks
- [ ] **Can place blocks sideways** - Place on any face, not just top

### 2.2 Hand & Item Rendering
- [ ] **Right hand model** - First-person hand visible
- [ ] **Block in right hand** - Show held item in 3D
- [ ] **Hand animation** - Idle sway animation
- [ ] **Pickaxe animation frames** - Capture swing frames
- [ ] **Pickaxe animation playback** - Animate tool usage
- [ ] **Set precise right arm position** - Fine-tune hand placement
- [ ] **Right hand reacts to light** - Apply lighting to hand model

### 2.3 UI Enhancements
- [ ] **Block icons in inventory** - Render block previews
- [ ] **Text rendering system** - Font rendering for UI
- [ ] **Item count display** - Show stack sizes
- [ ] **Scalable UI** - Resolution-independent UI
- [ ] **Hearts and food UI** - Health and hunger display
- [ ] **Tools durability bar** - Show tool wear
- [ ] **3D Steve in inventory** - Player model that follows mouse
- [ ] **High contrast mode** - Accessibility enhancement (may already exist)

---

## Phase 3: Item System & World Persistence

### 3.1 Item Drops
- [ ] **Item drop entities** - Spawn items in world
- [ ] **Rotating and hovering items** - Animated drop rendering
- [ ] **Item drops collide with terrain** - Physics for drops
- [ ] **Item drops move toward player** - Magnetic collection
- [ ] **Can throw items** - Drop from inventory with velocity

### 3.2 Item & Block Types
- [ ] **Working glowstones** - Light-emitting blocks
- [ ] **Torches** - Placeable light sources
- [ ] **Torches attached to blocks** - Wall-mounted torches
- [ ] **Light passes through transparent blocks** - Glass, water, etc.
- [ ] **Add glass blocks** - Transparent building material
- [ ] **Coal ore generation** - Mineable resource
- [ ] **Iron ore generation** - Mineable resource
- [ ] **Diamond ore generation** - Rare valuable resource
- [ ] **Custom item drops** - Different blocks drop different items

### 3.3 World Persistence
- [ ] **Save whole world** - Serialize chunks to disk
- [ ] **Load whole world** - Deserialize and restore world state
- [ ] **Auto-save system** - Periodic saves

---

## Phase 4: Tools & Crafting

### 4.1 Tool System
- [ ] **Add tools** - Pickaxe, axe, shovel, sword
- [ ] **Generate 3D mesh from pixel art** - Tool for creating item models
- [ ] **Tool durability** - Tools wear down with use
- [ ] **Different breaking speeds** - Tools affect break time
- [ ] **Stone pickaxe for iron ore** - Tool tier requirements
- [ ] **Iron pickaxe for diamond** - Progressive mining tiers

### 4.2 Crafting System
- [ ] **Crafting recipes** - Define item combinations
- [ ] **Crafting UI** - 2x2 or 3x3 grid interface
- [ ] **Crafting table block** - Placeable 3x3 crafting station
- [ ] **Crafting table functionality** - Advanced crafting access

---

## Phase 5: Entities & AI

### 5.1 Entity Framework
- [ ] **Entity system architecture** - Base entity class/trait
- [ ] **Entity rendering** - Draw entities in world
- [ ] **Entity physics** - Collision, gravity for entities
- [ ] **Entities react to light** - Apply lighting to entities

### 5.2 Zombie AI
- [ ] **Add zombies** - Basic zombie entity
- [ ] **Zombie walk animation** - Animated movement
- [ ] **Zombie idle behavior** - Wander randomly by default
- [ ] **Zombie pathfinding** - Find best path to target
- [ ] **Pathfinding in mazes** - Handle complex terrain
- [ ] **Zombie follows path** - Execute navigation
- [ ] **Zombie can chase player** - Aggressive behavior
- [ ] **Zombie can walk diagonally** - Smooth movement
- [ ] **Zombie spawn system** - Spawn in darkness/at night
- [ ] **One thousand simulated zombies** - Performance optimization

### 5.3 Combat System
- [ ] **Can one-hit zombies** - Basic melee attack
- [ ] **Zombies pushed on hit** - Knockback effect
- [ ] **Red overlay on hit** - Damage feedback
- [ ] **Can get hit by zombie** - Player takes damage
- [ ] **Health system** - Player HP tracking
- [ ] **Death system** - Game over on 0 HP
- [ ] **Game end screen** - Death UI
- [ ] **Respawn system** - Return to spawn point
- [ ] **All items drop on respawn** - Death penalty
- [ ] **Fall damage** - Height-based damage
- [ ] **Health regen** - Restore HP if no damage for 5 minutes
- [ ] **Menu pauses game** - Freeze game state

---

## Phase 6: Polish & Gameplay

### 6.1 Visual Effects
- [ ] **Particle system** - Generic particle emitter
- [ ] **Block break particles** - Destroy animation
- [ ] **Block place particles** - Placement feedback

### 6.2 Collision & Physics
- [ ] **Front collision** - Prevent walking through blocks
- [ ] **Side collision** - All-direction collision (may already exist)

### 6.3 Quality of Life
- [ ] **Removed food icons** - Simplified UI (if food system exists)

---

## Implementation Priority & Dependencies

### Critical Path
1. **Infinite terrain** (Phase 1.1) - Foundation for exploration
2. **Lighting system** (Phase 1.2) - Required for torches, day/night
3. **Item drops** (Phase 3.1) - Needed for crafting materials
4. **Tools** (Phase 4.1) - Required for progression
5. **Crafting** (Phase 4.2) - Core gameplay loop
6. **Entities** (Phase 5.1) - Foundation for mobs
7. **Combat** (Phase 5.3) - Survival mechanics

### Parallel Tracks
- **Visual polish** (Phase 1.3) can be done anytime
- **UI enhancements** (Phase 2.3) can be done independently
- **World persistence** (Phase 3.3) can be done after basic gameplay works
- **Zombie AI** (Phase 5.2) requires entity framework first

---

## Technical Architecture Notes

### Infinite Terrain
- Implement chunk loading/unloading based on player position
- Add chunk serialization for save/load
- Need async chunk generation to avoid frame drops
- Consider view distance setting (e.g., 8-16 chunks)

### Lighting System
- Two light types: skylight (0-15) and blocklight (0-15)
- 4-bit per light type per block = 1 byte per block
- BFS light propagation algorithm
- Light update queue for dynamic changes
- Shader needs to sample light values

### Entity System
- ECS (Entity Component System) or trait-based approach
- Entity ID management
- Spatial partitioning for collision (chunk-based)
- Interpolation for smooth rendering
- Animation system with keyframes

### Item System
- Item ID + metadata (durability, enchantments, etc.)
- Item stack (type + count)
- Item entity separate from item stack
- Loot tables for block drops

### Crafting
- Recipe registry (input pattern → output)
- Shaped vs shapeless recipes
- Crafting grid state management
- Inventory transaction system

### Pathfinding
- A* or Dijkstra on voxel grid
- Cache paths, recalculate when invalidated
- Jump/fall cost calculations
- Path smoothing for natural movement

### Combat
- Hitbox/hurtbox system
- Damage types (melee, fall, etc.)
- Invulnerability frames
- Knockback vector calculation

---

## Estimated Complexity (Story Points)

| Phase | Complexity | Reason |
|-------|-----------|--------|
| Phase 1 | 21 pts | Lighting system is complex (8), terrain is moderate (8), visuals are easy (5) |
| Phase 2 | 13 pts | Animations and rendering (8), UI work (5) |
| Phase 3 | 13 pts | Item physics (5), block types (3), persistence (5) |
| Phase 4 | 13 pts | Tool system (5), crafting logic (8) |
| Phase 5 | 34 pts | Entity framework (8), AI pathfinding (13), combat (13) |
| Phase 6 | 5 pts | Mostly polish and small fixes |
| **Total** | **99 pts** | ~3-6 months for solo dev |

---

## Development Milestones

### Milestone 1: "Playable World" (Phases 1-2)
- Infinite terrain you can explore
- Day/night cycle with lighting
- Breaking and placing blocks feels good
- **Goal**: Fun to walk around and build

### Milestone 2: "Survival Lite" (Phases 3-4)
- Gather resources from the world
- Craft tools to progress
- Mine better ores with better tools
- **Goal**: Progression loop established

### Milestone 3: "Full Survival" (Phase 5)
- Hostile mobs spawn and attack
- Combat and health system
- Death and respawn
- **Goal**: Challenge and risk added

### Milestone 4: "Polish" (Phase 6)
- Particles, effects, QoL
- Performance optimization
- Bug fixes
- **Goal**: Game feels complete

---

## Next Steps

1. Review this plan and prioritize features
2. Start with Phase 1.1 (infinite terrain) or Phase 1.2 (lighting)
3. Set up project tracking (GitHub issues/projects)
4. Begin implementation following the dependency graph
5. Test each feature thoroughly before moving on
6. Iterate based on playtesting feedback

---

## Notes

- This plan assumes a solo developer working part-time
- Some features may be simpler if you reuse existing systems (e.g., electrical system concepts for lighting)
- Consider using external crates for complex systems (e.g., `specs` for ECS, `rapier` for physics)
- Prioritize getting a minimal version working before adding all features
- The original game likely took months to develop - be patient!

---

## References

- Current codebase: `/home/user/rustcraft/src/`
- Existing documentation: `STATE.md`, `electrical.md`
- Dependencies: `wgpu`, `winit`, `cgmath`, `noise`
- Inspiration: Original Minecraft development blog
