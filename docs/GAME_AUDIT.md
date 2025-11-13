# Gameplay Opportunity & Flaw Audit

This audit highlights the most impactful feature ideas plus concrete flaws spotted while reviewing the runtime systems, UI flow, and support docs.

## High-Impact Additions

1. **Finish the crafting progression loop**  
   The recipe table currently reuses `BlockType::Wood` as the output for planks and sticks, and then references those placeholders inside the pickaxe/axe recipes. The comments explicitly note the missing block/item definitions, so formalizing `Plank` and `Stick` items (or block variants) would immediately unlock a functional tiered progression and allow you to add more recipes safely.  
   _Reference: `CraftingSystem::register_default_recipes` in `src/crafting.rs` keeps TODO placeholders for both outputs._

2. **Expose the dormant settings/accessibility systems in UI**  
   `settings.rs` already defines accessibility modes, colorblind matrices, and keybindings, but nothing in the current codebase reads or mutates these structs. Wiring them into a pause/settings menu (and saving them per profile) would boost usability and gives you a clear UI feature to build.  
   _Reference: `AccessibilitySettings`, `GraphicsSettings`, and `KeyBindings` are only defined in `src/settings.rs` without any call sites._

3. **Expand the entity system beyond dropped items**  
   The entire `entity.rs` module models only `ItemEntity`, meaning there is no infrastructure for living mobs, projectiles, or even basic particle anchors. Generalizing this into a world entity registry with update hooks would open the door for animals, enemies, and interactable props requested in the roadmap.  
   _Reference: `src/entity.rs` solely defines `ItemEntity` with simple gravity/collision logic._

4. **Add global time-of-day & weather hooks**  
   The roadmap already calls out the absence of a day/night cycle, weather, or growth ticks. Implementing a `WorldClock` that drives sky gradients, lighting falloff, and biome-specific weather events would make the world feel alive and provide levers for mob spawning or electrical systems.  
   _Reference: `STATE.md` ("World Interaction" section) lists day/night, weather, and block tick systems as missing features._

5. **Deliver on electrical gameplay feedback**  
   The electrical component model (connectors, terminals, telemetry) is quite deep, yet the UI only renders attachment handles in the inspector. Adding oscilloscopes, spark VFX, or tutorial circuits would surface the system to players and justify the detailed logic already present in `electric.rs`.  
   _Reference: `ElectricalComponent::connectors`/`default_params` in `src/electric.rs` show the simulation depth awaiting visualization._

## Flaws & Quick Wins

1. **Crafting outputs are semantically wrong**  
   Because planks and sticks both still point to `BlockType::Wood`, players would get wood logs back when trying to craft, which makes every tool recipe impossible to satisfy legitimately. Introduce dedicated `ItemType::Material` variants or new block IDs before shipping the crafting UI.  
   _Reference: `src/crafting.rs` lines 166-205 keep returning `ItemType::Block(Wood)` while TODOs admit the missing assets._

2. **Dropped-item "random" velocities are deterministic**  
   `ItemEntity::new` derives its lateral velocity from deterministic sine hashes of the spawn coordinates, so breaking several blocks at the same (x, z) produces identical throw arcs every time. Switching to a seeded RNG (or even frame counter noise) would break that repetition.  
   _Reference: `src/entity.rs` lines 18-25 compute "random" vectors solely from position values._

3. **Lighting updates are excessively coarse**  
   `LightingSystem::update_light_at` recomputes skylight and blocklight for the entire chunk (and its neighbors) on each edit instead of performing incremental flood updates. This leads to O(chunk) work per placement and becomes a bottleneck for fast builders. Refactoring toward localized BFS queues per changed block would prevent unnecessary propagation.  
   _Reference: `src/lighting.rs` lines 10-80 run full chunk recalculations each time `update_light_at` is called._

4. **Dead settings/accessibility code adds maintenance overhead**  
   Because `AccessibilitySettings`, `GraphicsSettings`, and keybinding toggles are never loaded or persisted, they silently rot when you rename actions or add inputs. Either remove them until a settings menu ships, or hook them into `main.rs` so they control heatmaps, diagnostics overlays, and colorblind matrices.  
   _Reference: `src/settings.rs` lines 1-120 define these structs, but `rg` finds zero usages elsewhere in the tree._

5. **Fluid roadmap issues remain unresolved**  
   The outstanding TODO list still reports raised ridges, floating sheets, and smoothing instability in the GPU fluids. Tackling those shader-level issues (plus exposing an in-game diagnostic overlay tied to `FluidSystem`) would close one of the loudest visual glitches players currently see.  
   _Reference: `STATE.md` lines 98-105 enumerate the known water/ridge artifacts awaiting fixes._

## Suggested Next Steps

1. Land dedicated data for planks/sticks, validate the crafting grid logic against those new items, and extend recipe coverage.  
2. Implement a pause/settings overlay that mutates the structs in `settings.rs`, persists them, and applies the colorblind matrices in the renderer.  
3. Introduce a lightweight entity registry so future mobs, projectiles, and interactable props can reuse motion/collision helpers.  
4. Add a `WorldClock` resource that feeds both lighting gradients and weather toggles before layering mobs or crops on top.  
5. Instrument the fluid GPU path with a debug heatmap (hooked to the existing diagnostics overlay keybind) so visual issues are easier to reproduce and fix.
