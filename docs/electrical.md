# Electrical System Plan

## 1. Simulation Core
- Adopt a per-tick Modified Nodal Analysis solver operating on real electrical units (volts, amps, ohms, farads, henries).
- Define common node/branch data structures plus sparse matrix routines that can run on CPU or GPU.
- Build a component library covering resistors, capacitors, inductors, ideal/current sources, controlled sources, DC-DC converters.
- Specify tick scheduling, convergence tolerances, and fallback strategies when circuits fail to converge.

## 2. World Representation
- Extend chunk save format to store per-face electrical components; allow surface-mounted and multi-block devices.
- Represent cables with tier metadata, resistance, and thermal limits; cache impedance/state for fast lookup.
- Partition circuit graphs by chunk/region, maintaining caches for inactive networks to reduce solver work.

## 3. Voltage Tier Progression
- Establish canonical voltage tiers (e.g., 48 V, 200 V, 800 V) and enforce compatible wire/device combinations.
- Implement DC-DC converters/transformers for stepping between tiers, including efficiency and heat models.
- Gate machinery and player progression by tier requirements to mirror Electrical Age gameplay loop.

## 4. Power Sources & Storage
- Create generator blocks (fuel, steam, wind, solar, hydro) with defined IV curves and tick-based output.
- Add battery chemistries and capacitors with charge/discharge behavior (½ C V²) and internal resistance.
- Provide hooks for mechanical integration (shafts, turbines) to pair electrical and kinetic systems.

## 4a. Core Blocks & Feature Concepts
- **Wire & Bus Blocks**: tiered cables, junction boxes, patch panels; each face supports connectors, insulators, and measurement taps.
- **Power Sources**: modular generators (fuel, steam, kinetic), photovoltaic panels, wind rotors; each exposes rated voltage/current and dynamic efficiency curves.
- **Energy Storage**: batteries (different chemistries, cycles, leakage), capacitors banks for smoothing, flywheels/inductor storage for transient loads.
- **Converters & Transformers**: DC-DC converters, step-up/down transformers, rectifiers/inverters (future AC support); configurable via UI with efficiency and heat outputs.
- **Protection Hardware**: fuses, breakers, surge arresters, relays, ground-fault interrupters; all mount in switchgear enclosures and integrate with logic.
- **Control & Logic Modules**: analog gates, amplifiers, op-amps, PID controllers, oscillators, comparators, wireless telemetry nodes.
- **Instrumentation**: multimeter probe, clamp meter, panel meters (V/A/W), oscilloscopes, data loggers; provide real-time readings and history graphs.
- **Integration Blocks**: redstone-voltage interface, automation bus, computer probe (API access), energy exporter/importer to other systems.
- **Gameplay Loops**: progress through research tiers unlocking higher-rated conductors, advanced sources, automation modules; manage heat, insulation, and maintenance to keep grids stable.

## 4b. Component Placement & Orientation
- Persist an `Axis` (X/Y/Z) for every electrical block so connectors snap to real faces inside the voxel grid instead of assuming a flat plane.
- Bundle per-block electrical parameters inside `ComponentParams` (`resistance_ohms`, `voltage_volts`, `max_current_amps`) to keep simulation constants next to the component definition.
- On placement, infer orientation from the surface normal and player heading, then queue it through `ElectricalSystem::set_axis` so world data, meshes, and the solver stay aligned.
- Mesh generation consults the stored axis before drawing sub-block geometry, preventing wires or leads from clipping when rotated in 3D.

```rust
let pos = BlockPos3::new(world_x, world_y, world_z);
let axis = world
    .electrical()
    .axis_at(pos)
    .unwrap_or_else(|| block_type.default_axis());
let connectors = block_type.electrical_connectors(axis);
```

- Network rebuilds emit `NetworkElement` entries (component, axis, parameters) ready for Modified Nodal Analysis stamping without extra lookups.
- Default gameplay tuning lives in `ElectricalComponent::default_params` (e.g., 0.05 ohm copper wire, 220 ohm resistor, 12 V source) so balancing and UI readouts stay consistent.

## 5. Protection & Safety
- Add fuses, breakers, relays with configurable trip curves; integrate with wire thermal model for overloads.
- Model overvoltage/overcurrent failure (wire melt, device damage) and enforce safe shutdown behavior.
- Implement grounding: explicit 0 V node, equipotential bonding, ground fault sensing.

## 6. Measurement & Control
- Supply instruments (multimeter, clamp meter, oscilloscope) that show live voltage/current/power data.
- Include analog and digital logic blocks (gates, op-amp, PID, oscillators, filters) for automation.
- Support wireless telemetry/data logging and expose APIs for future scripting integrations.

## 7. Gameplay & UI
- Expand HUD/tooltips to display voltage tier, load, temperature, breaker status on relevant blocks.
- Provide tutorials/advancements that teach Ohm’s law, Kirchhoff’s laws, proper wire sizing, and transformer usage.
- Offer placement previews and routing aids for dense per-face component builds.

## 8. Interoperability
- Create redstone ↔ voltage interface blocks with adjustable thresholds or linear mapping.
- Design energy exporter/importer blocks for interoperability with other modded power systems.
- Add a programmable probe/interface so external computers can query circuit values.

## 9. Performance & Memory
- Store network data in structure-of-arrays layouts for cache efficiency and coalesced GPU access; reuse solver buffers.
- Update networks incrementally using dirty-region tracking, background solver threads, and GPU offload with CPU fallback.
- Integrate profiling/diagnostics for network complexity, solver iterations, and memory footprint.

## 10. Testing & Tooling
- Create unit tests for the solver (RLC benchmarks, converter efficiency) and integration tests for protection scenarios.
- Ship debug visualizations (voltage heatmaps, current vectors, tier overlays) to aid development.
- Document component specifications, governing equations, and gameplay loops (this document).

## 11. Implementation Notes (Rust & 3D Graph Solver)
- Model each electrical network as a graph where nodes correspond to block-face terminals or wire junctions in 3D space; edges carry component metadata (R/L/C, sources, converters).

### 11.1 Core Data Structures

```rust
/// Local identifier for an electrical node (potential).
type NodeId = u32;

/// Unique identifier for a branch current variable (e.g. inductors, voltage sources).
type BranchId = u32;

/// Discrete component placed between two nodes.
#[derive(Debug)]
enum ElementKind {
    Resistor { resistance: f64 },
    Capacitor { capacitance: f64 },
    Inductor { inductance: f64, branch: BranchId },
    VoltageSource { voltage: f64, branch: BranchId },
    CurrentSource { current: f64 },
    // Extend with controlled sources, converters, etc.
}

#[derive(Debug)]
struct Element {
    positive: NodeId,
    negative: NodeId,
    kind: ElementKind,
}

/// A node maps back to the world for heat, tier, and block association.
#[derive(Debug)]
struct NodeData {
    position: glam::IVec3,
    chunk: ChunkPos,
    tier: VoltageTier,
    grounded: bool,
}

/// Complete circuit region extracted from connected chunks.
struct CircuitRegion {
    nodes: Vec<NodeData>,
    elements: Vec<Element>,
    // Optional adjacency for debugging/visualization.
}
```

- Organize nodes/elements inside chunk-scoped arenas (e.g., `Vec<NodeData>`, `Vec<Element>`) with stable indices; maintain a `UnionFind<NodeKey>` to regroup connected chunks into `CircuitRegion`s whenever topology changes.

### 11.2 Mapping World Geometry to Nodes

1. Each electrical block exposes up to six terminals (one per face). A helper maps `(chunk_pos, block_pos, face)` into a unique `NodeId`.
2. Wires travelling along voxel edges create intermediate junction nodes. During placement we rasterize the path with a 3D Bresenham algorithm:

```rust
fn ensure_wire_path(
    graph: &mut GraphBuilder,
    start: NodeHandle,
    end: NodeHandle,
    params: &WireParams,
) {
    let mut current = start;
    for step in rasterize_voxel_path(start.position, end.position) {
        let next = graph.ensure_node(step.to_node_key());
        graph.add_element(current, next, ElementKind::Resistor {
            resistance: params.resistance_per_meter * params.segment_length,
        });
        current = next;
    }
    graph.add_element(current, end, ElementKind::Resistor {
        resistance: params.resistance_per_meter * params.segment_length,
    });
}
```

- Nodes cache the coordinates to support heat simulation, tier validation, and debug rendering.

### 11.3 Building Solver Matrices (Modified Nodal Analysis)

1. For each dirty region we assemble the MNA system: `A * x = z`, where `x` contains node voltages and branch currents.
2. The matrix `A` is sparse; we populate it in coordinate form before converting to CSR:

```rust
fn assemble_region(region: &CircuitRegion) -> (sprs::CsMat<f64>, Vec<f64>) {
    let mut builder = sprs::TriMat::with_capacity((region.node_count(), region.node_count()), region.elements.len() * 4);
    let mut rhs = vec![0.0; region.node_count() + region.branch_count()];

    for element in &region.elements {
        match element.kind {
            ElementKind::Resistor { resistance } => {
                let g = 1.0 / resistance;
                stamp_conductance(&mut builder, element.positive, element.negative, g);
            }
            ElementKind::CurrentSource { current } => {
                rhs[element.positive as usize] += current;
                rhs[element.negative as usize] -= current;
            }
            ElementKind::VoltageSource { voltage, branch } => {
                stamp_voltage_source(&mut builder, &mut rhs, element.positive, element.negative, branch, voltage);
            }
            ElementKind::Capacitor { capacitance } => {
                // Use backward Euler / trapezoidal integration; requires previous state.
            }
            ElementKind::Inductor { inductance, branch } => {
                // Similar treatment, introducing branch current variable.
            }
        }
    }

    (builder.to_csr(), rhs)
}
```

- Iterative solvers (CG/GMRES) from `sprs` or `nalgebra-sparse` handle the linear system. For larger matrices (hundreds of nodes), optionally upload the CSR structure to a `wgpu` compute pipeline running conjugate gradient.

### 11.4 Tick Pipeline

1. **Detect Dirty Networks**: When players add/remove components or machines change mode, mark the affected nodes. Use union-find to derive connected `CircuitRegion`s needing recomputation.
2. **Assemble & Solve**: For each region, call `assemble_region`, solve for `x`. Cache the solution with timestamp; skip recomputation for regions without changes.
3. **Apply Results**:
   - For each element, compute current `I` using solved node voltages, update device state (e.g., generator output, machine power).
   - Update wire temperature using `I²R` and spread heat to surrounding blocks.
   - Check protection devices (fuse/breaker). If triggered, remove or disable corresponding element and mark region dirty for next tick.
4. **Persist**: Write results back into chunk data structures for saving/loading.

### 11.5 GPU Offload Strategy

- Maintain reusable GPU buffers:
  - CSR matrix values/indices.
  - Vectors for unknowns and RHS.
  - Workgroup dispatch tuned for up to 512 nodes.
- When region size exceeds CPU threshold (e.g., >256 nodes), copy matrix to GPU and run iterative solver kernels (implemented in WGSL).
- Provide CPU fallback for hardware lacking compute-capable devices or when GPU is saturated (reusing the adaptive scheduler built for fluids).

### 11.6 Performance Considerations

- Store circuit data in a structure-of-arrays layout for cache-friendly traversal (`Vec<NodeData>` + `Vec<Element>`).
- Cap region sizes (e.g., 512 nodes, 768 elements). Encourage players to place transformers or DC-DC converters to isolate grids; enforce limits in placement logic.
- Apply incremental stamping: if only a few elements change, update matrix entries in place rather than rebuilding from scratch.
- Parallelize across regions using Rayon; each region solve is independent.

### 11.7 Handling Non-linear Devices

- For components with non-linear IV curves (diodes, converters), use a Newton-Raphson iteration:
  1. Linearize device around previous operating point, produce equivalent conductance/source.
  2. Assemble/solve MNA.
  3. Update operating point and repeat until convergence or max iterations.
- Keep iteration counts low (2–3) for real-time performance; clamp voltage/current to safe ranges to avoid runaway loops.

### 11.8 World Integration & Event Flow

- On block placement/removal:
  1. Map block faces to node handles via `face_node(chunk, block_pos, face)`.
  2. Connect or disconnect elements; update union-find.
  3. Mark corresponding region dirty and schedule solver run next tick.
- On chunk load/unload:
  - Deserialize circuit state.
  - Register nodes/elements with region manager; rebuild union-find for boundary connections.
  - For unloaded neighbouring chunks, freeze cross-boundary edges until both sides are present.

### 11.9 Debug Instrumentation

- Provide developer overlay showing:
  - Node voltages (colored cubes).
  - Branch currents (arrows along wires).
  - Region boundaries and node counts.
- Log solver metrics per tick (region count, matrix size, iteration counts, time spent CPU vs GPU).

### 11.10 Example Usage inside Tick Loop

```rust
pub fn tick_electrical(world: &mut World, tick: u64) {
    let dirty_regions = world.circuit_manager.collect_dirty_regions();

    dirty_regions.par_iter().for_each(|region_id| {
        let region = world.circuit_manager.build_region(*region_id);
        let (matrix, rhs) = assemble_region(&region);
        let solution = solve_region(&matrix, &rhs);
        world.circuit_manager.apply_solution(*region_id, solution, tick);
    });
}
```

- `solve_region` chooses CPU/GPU backend based on size and current load.
- `apply_solution` updates machines, heat, protection devices, and schedules follow-up events if breakers trip.

This structure keeps the electrical simulation grounded in real physics while scaling to 3D voxel worlds. Rust’s ownership guarantees and explicit graph representation help maintain performance and correctness even with large player-built networks.
