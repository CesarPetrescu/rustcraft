# Electrical System Analysis

## Component Overview

### 1. Wire (CopperWire)
- **Default Params**: 0.05Ω resistance, 30A max current
- **Default Axis**: X
- **Connector Logic**:
  - Connects along main axis (2 directions)
  - Connects along secondary axis (2 directions, perpendicular to main and mount face)
  - Connects through mount face (1 direction)
  - **Total: Up to 5 directions**

### 2. Resistor
- **Default Params**: 220Ω resistance, 2A max current (configurable)
- **Default Axis**: X
- **Connector Logic**: Same as Wire
  - Main axis pair + secondary axis pair + mount face
  - **Total: Up to 5 directions**

### 3. Voltage Source
- **Default Params**: 12V voltage, 0.2Ω internal resistance, 5A max current (configurable)
- **Default Axis**: X
- **Connector Logic**:
  - Connects along main axis only (2 directions)
  - Connects through mount face (1 direction)
  - **Total: 3 directions**

### 4. Ground
- **Default Params**: 0V voltage, 0Ω resistance
- **Default Axis**: Y
- **Connector Logic**:
  - Connects through mount face (1 direction)
  - Connects through opposite face (1 direction)
  - **Total: 2 directions (through the block)**

## Connection Logic Flow

### Connector Determination
```
For each component at (position, face):
1. Determine axis (inferred from neighbors or default)
2. Calculate connectors based on component type + axis + face
3. Store as ElectricalNode
```

### Network Building (BFS Traversal)
```
For each unvisited node:
1. Start BFS from this node
2. Check 6 directions for external connections:
   - If node has connector[i] AND neighbor has connector[opposite(i)]
   - Add neighbor to network
3. Check other faces on same block for intra-block connections:
   - If nodes share ANY connector direction
   - Add to network
4. Mark network as powered if it has BOTH voltage source AND ground
```

### Telemetry Calculation
```
For each network:
1. Sum total resistance of all components
2. Get source voltage from voltage source
3. Calculate current: I = V / R (if network has loop)
4. Calculate voltage drop across each component: V_drop = I * R_component
5. Update each node's telemetry
```

## Connection Examples

### Example 1: Two Wires on Adjacent Blocks
```
Block A (0,0,0) - Top face, axis X:
  Connectors: [E✓, W✓, T✓, -, S✓, N✓]
  Indices:    [0,  1,  2,  3, 4,  5 ]

Block B (1,0,0) - Top face, axis X:
  Connectors: [E✓, W✓, T✓, -, S✓, N✓]
  Indices:    [0,  1,  2,  3, 4,  5 ]

Connection check:
- Wire A has connector[0] (East) = true
- Wire B is at offset (+1,0,0) from A
- opposite(0) = 1 (West)
- Wire B has connector[1] (West) = true
- ✓ CONNECTION ESTABLISHED
```

### Example 2: Two Wires on Same Block, Different Faces
```
Wire A on Top face, axis X:
  Connectors: [E✓, W✓, T✓, -, S✓, N✓]

Wire B on Bottom face, axis X:
  Connectors: [E✓, W✓, -, B✓, S✓, N✓]

Intra-block connection check:
- Share indices: 0(E), 1(W), 4(S), 5(N)
- ✓ CONNECTION ESTABLISHED (via shared horizontal connectors)
- Note: Do NOT connect via Top-Bottom directly!
```

### Example 3: Wire to Voltage Source
```
Wire on Top face, axis X:
  Connectors: [E✓, W✓, T✓, -, S✓, N✓]

Voltage Source on Top face, axis X:
  Connectors: [E✓, W✓, T✓, -, -, -]

Intra-block connection check:
- Share indices: 0(E), 1(W), 2(T)
- ✓ CONNECTION ESTABLISHED
```

### Example 4: Ground Connection
```
Ground on Top face:
  Connectors: [-, -, T✓, B✓, -, -]
  (Connects through block vertically)

Wire on Bottom face, axis X:
  Connectors: [E✓, W✓, -, B✓, S✓, N✓]

Intra-block connection check:
- Share index: 3(B)
- ✓ CONNECTION ESTABLISHED
```

## Identified Issues

### Issue 1: Axis Inference May Not Consider All Neighbors
**Location**: `electric.rs:495-550` (`infer_axis` function)

**Problem**: The axis inference only checks the first matching neighbor and returns immediately. If there are multiple neighbors with different orientations, it may not choose the optimal axis.

**Current Logic**:
```rust
for (idx, dir) in NEIGHBOR_DIRS.iter().enumerate() {
    let neighbor_pos = world_pos.offset(*dir);
    let opposite = opposite_index(idx);
    if let Some(neighbors) = self.nodes.get(&neighbor_pos) {
        if neighbors.iter().any(|(_, node)| node.connectors()[opposite]) {
            return Axis::from_connector_index(idx);  // Returns immediately!
        }
    }
}
```

**Impact**: When placing a component that could connect to multiple neighbors in different directions, the axis may be chosen arbitrarily based on iteration order, not based on the best connectivity.

### Issue 2: No Validation of Network Topology
**Location**: `electric.rs:552-646` (`rebuild_networks` function)

**Problem**: The network building doesn't validate if the topology makes electrical sense. For example, multiple voltage sources in the same network should be checked for conflicts.

**Impact**: Invalid circuits are allowed, potentially causing confusing behavior.

### Issue 3: Terminal Faces Not Used in Connection Logic
**Location**: `electric.rs:156-163` (`terminal_faces` function)

**Problem**: The `terminal_faces` method calculates which faces are the electrical terminals, but this information is NOT used in the connection logic. Connectors are determined independently without considering which ends are the actual terminals.

**Current Logic**:
- `terminal_faces` returns the positive and negative terminals
- But `connectors` method doesn't use this information
- Components can connect from non-terminal directions

**Impact**: Components may connect in ways that don't match their physical/electrical design.

### Issue 4: Ground Component Connects Bidirectionally When It Shouldn't
**Location**: `electric.rs:128-133` (Ground connector logic)

**Problem**: Ground is set to connect through both mount face AND opposite face, making it bidirectional. But physically, a ground should typically only connect in ONE direction (from the circuit to ground).

**Current Logic**:
```rust
Self::Ground => {
    let mut connectors = [false; 6];
    connectors[face_index(face)] = true;
    connectors[face_index(face.opposite())] = true;  // Bidirectional!
    connectors
}
```

**Impact**: Ground components can "pass through" blocks, potentially creating unintended connections.

### Issue 5: Intra-Block Connections Too Permissive
**Location**: `electric.rs:615-638` and `electric.rs:461-471`

**Problem**: The intra-block connection logic connects components if they share ANY connector direction, not specifically if they should electrically connect.

**Example**: Two wires on opposite faces (Top/Bottom) with horizontal orientation (axis X) will connect via their shared East/West/North/South connectors, even though physically they're stacked vertically and shouldn't connect horizontally.

**Impact**: Components on the same block connect in visually confusing ways.

### Issue 6: Secondary Axis Logic May Create Unintended T-Junctions
**Location**: `electric.rs:113-124` (Wire/Resistor connector logic)

**Problem**: Wires and resistors automatically get a secondary axis, allowing them to form T-junctions. While this may be intended, it can create unexpected connections.

**Example**:
```
Wire on Top face, axis X:
  Connectors: East, West (main axis)
           + North, South (secondary axis Z)
           + Top (mount face)
```

This wire can connect to 4 horizontal neighbors + the mount point, forming a 4-way junction automatically.

**Impact**: Wires may connect to more neighbors than the user expects.

### Issue 7: No Visual Feedback for Axis Orientation
**Problem**: When placing components, users may not be able to easily see or control the axis orientation, leading to unexpected connector directions.

**Impact**: "Textures seem connected but logic doesn't work" - the visual may not clearly show which direction the component is oriented.

### Issue 8: Connection Mask Doesn't Distinguish Intra vs External Connections
**Location**: `electric.rs:439-474` (`connection_mask` function)

**Problem**: The connection mask used for rendering treats intra-block connections the same as external connections, marking all shared connector directions as "connected". This doesn't provide accurate visual feedback.

**Impact**: Rendering may show connections in directions that don't represent the actual connection path.

## Recommended Fixes

### Priority 1: Fix Terminal Face Logic
- Components should primarily connect through their terminal faces
- Wire/Resistor: terminals are axis positive and negative faces
- Voltage Source: terminals are axis positive and negative faces
- Ground: terminal is only mount face (NOT opposite)

### Priority 2: Improve Axis Inference
- Consider ALL neighbors, not just first match
- Weight towards axes that would create most connections
- Provide visual feedback for axis orientation

### Priority 3: Refine Intra-Block Connection Logic
- Components on same block should only connect if:
  a) They share a connector direction that makes physical sense, OR
  b) One component's terminal aligns with another's connector

### Priority 4: Add Network Validation
- Warn about multiple voltage sources in same network
- Check for short circuits (voltage source directly to ground with no resistance)
- Validate that ground is actually grounded

### Priority 5: Improve Visual Feedback
- Show axis orientation clearly
- Distinguish between potential connectors and actual connections
- Color-code by network to show which components are connected

## Network Flow Diagram

```
Component Placement
       ↓
update_block_with()
       ↓
  ┌─────────────────┐
  │ Determine Axis  │ ← May have issues (Issue 1)
  │ (infer_axis)    │
  └────────┬────────┘
           ↓
  ┌─────────────────┐
  │ Create Node     │
  │ with Connectors │ ← Connector logic (Issues 3,4,5,6)
  └────────┬────────┘
           ↓
  ┌─────────────────┐
  │ Mark Dirty      │
  └────────┬────────┘
           ↓
    (On next tick)
           ↓
  ┌─────────────────┐
  │ rebuild_networks│ ← BFS traversal (Issue 2)
  └────────┬────────┘
           ↓
  ┌─────────────────┐
  │update_telemetry │ ← Calculate V & I
  └────────┬────────┘
           ↓
  ┌─────────────────┐
  │ Render with     │ ← Visual feedback (Issues 7,8)
  │ connection_mask │
  └─────────────────┘
```

## Conclusion

The electrical system has a solid foundation but suffers from several logic issues:

1. **Over-connectivity**: Components connect too easily via shared connectors
2. **Axis ambiguity**: Axis inference doesn't consider all neighbors
3. **Terminal mismatch**: Terminal faces aren't enforced in connection logic
4. **Ground bidirectionality**: Grounds connect both directions through blocks
5. **Visual disconnect**: Rendering doesn't accurately show connection paths

These issues combine to create the user's reported problem: "textures seem connected but logic doesn't work" - components may LOOK aligned but have wrong axes, or they may show connections in directions that don't represent the actual electrical path.
