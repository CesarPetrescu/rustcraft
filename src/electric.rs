use std::collections::{HashMap, HashSet, VecDeque};

use cgmath::Vector3;

use crate::{
    block::{Axis, BlockFace, BlockType, ElectricalKind},
    chunk::CHUNK_SIZE,
    world::ChunkPos,
};

/// Directions used to find Manhattan-adjacent neighbors in the grid.
const NEIGHBOR_DIRS: [Vector3<i32>; 6] = [
    Vector3::new(1, 0, 0),
    Vector3::new(-1, 0, 0),
    Vector3::new(0, 1, 0),
    Vector3::new(0, -1, 0),
    Vector3::new(0, 0, 1),
    Vector3::new(0, 0, -1),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPos3 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos3 {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn offset(self, delta: Vector3<i32>) -> Self {
        Self::new(self.x + delta.x, self.y + delta.y, self.z + delta.z)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ComponentParams {
    pub resistance_ohms: Option<f32>,
    pub voltage_volts: Option<f32>,
    pub max_current_amps: Option<f32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ComponentTelemetry {
    pub voltage: f32,
    pub current: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AttachmentKey {
    pos: BlockPos3,
    face: BlockFace,
}

impl ComponentParams {
    pub const fn wire(resistance: f32, max_current: f32) -> Self {
        Self {
            resistance_ohms: Some(resistance),
            voltage_volts: None,
            max_current_amps: Some(max_current),
        }
    }

    pub const fn resistor(resistance: f32, max_current: f32) -> Self {
        Self {
            resistance_ohms: Some(resistance),
            voltage_volts: None,
            max_current_amps: Some(max_current),
        }
    }

    pub const fn voltage_source(voltage: f32, internal_resistance: f32, max_current: f32) -> Self {
        Self {
            resistance_ohms: Some(internal_resistance),
            voltage_volts: Some(voltage),
            max_current_amps: Some(max_current),
        }
    }

    pub const fn ground() -> Self {
        Self {
            resistance_ohms: Some(0.0),
            voltage_volts: Some(0.0),
            max_current_amps: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElectricalComponent {
    Wire,
    Resistor,
    VoltageSource,
    Ground,
}

impl ElectricalComponent {
    pub fn from_block(block: BlockType) -> Option<Self> {
        match block.electrical_kind()? {
            ElectricalKind::Wire => Some(Self::Wire),
            ElectricalKind::Resistor => Some(Self::Resistor),
            ElectricalKind::VoltageSource => Some(Self::VoltageSource),
            ElectricalKind::Ground => Some(Self::Ground),
        }
    }

    pub fn connectors(self, axis: Axis, face: BlockFace) -> [bool; 6] {
        match self {
            Self::Wire | Self::Resistor => {
                let mut connectors = axis_pair_connectors(axis);
                let secondary_axis = Axis::all()
                    .into_iter()
                    .find(|candidate| *candidate != axis && *candidate != face.axis())
                    .unwrap_or(axis);
                if secondary_axis != axis {
                    let extra = axis_pair_connectors(secondary_axis);
                    for (idx, value) in extra.iter().enumerate() {
                        if *value {
                            connectors[idx] = true;
                        }
                    }
                }
                // Also enable the mount face connector
                connectors[face_index(face)] = true;
                connectors
            }
            Self::VoltageSource => {
                let mut connectors = axis_pair_connectors(axis);
                // Also enable the mount face connector
                connectors[face_index(face)] = true;
                connectors
            }
            Self::Ground => {
                // Ground connects from all sides to any adjacent components
                // It acts as a ground reference point for the circuit
                [true; 6]
            }
        }
    }

    pub fn default_axis(self) -> Axis {
        match self {
            Self::Wire | Self::Resistor | Self::VoltageSource => Axis::X,
            Self::Ground => Axis::Y,
        }
    }

    pub fn default_params(self) -> ComponentParams {
        match self {
            Self::Wire => ComponentParams::wire(0.05, 30.0),
            Self::Resistor => ComponentParams::resistor(100.0, 2.0),
            Self::VoltageSource => ComponentParams::voltage_source(12.0, 0.1, 10.0),
            Self::Ground => ComponentParams::ground(),
        }
    }

    pub fn terminal_faces(self, axis: Axis, mount_face: BlockFace) -> (BlockFace, BlockFace) {
        match self {
            // Ground has only one terminal (mount face) - the same face serves as both terminals
            ElectricalComponent::Ground => (mount_face, mount_face),
            ElectricalComponent::Wire
            | ElectricalComponent::Resistor
            | ElectricalComponent::VoltageSource => (axis.positive_face(), axis.negative_face()),
        }
    }

    pub fn block_type(self) -> BlockType {
        match self {
            Self::Wire => BlockType::CopperWire,
            Self::Resistor => BlockType::Resistor,
            Self::VoltageSource => BlockType::VoltageSource,
            Self::Ground => BlockType::Ground,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ElectricalNode {
    pub component: ElectricalComponent,
    pub chunk: ChunkPos,
    pub axis: Axis,
    pub face: BlockFace,
    pub params: ComponentParams,
    pub telemetry: ComponentTelemetry,
}

impl ElectricalNode {
    pub fn connectors(&self) -> [bool; 6] {
        self.component.connectors(self.axis, self.face)
    }

    pub fn terminal_faces(&self) -> (BlockFace, BlockFace) {
        self.component.terminal_faces(self.axis, self.face)
    }
}

#[derive(Debug, Clone)]
pub struct NetworkElement {
    pub position: BlockPos3,
    pub component: ElectricalComponent,
    pub axis: Axis,
    pub face: BlockFace,
    pub params: ComponentParams,
}

#[derive(Debug, Default, Clone)]
pub struct ElectricalNetwork {
    pub elements: Vec<NetworkElement>,
    pub has_source: bool,
    pub has_ground: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FaceNodes {
    slots: [Option<ElectricalNode>; 6],
}

impl FaceNodes {
    fn set(&mut self, face: BlockFace, node: ElectricalNode) -> Option<ElectricalNode> {
        let idx = face_index(face);
        let previous = self.slots[idx].take();
        self.slots[idx] = Some(node);
        previous
    }

    fn get(&self, face: BlockFace) -> Option<&ElectricalNode> {
        let idx = face_index(face);
        self.slots[idx].as_ref()
    }

    fn get_mut(&mut self, face: BlockFace) -> Option<&mut ElectricalNode> {
        let idx = face_index(face);
        self.slots[idx].as_mut()
    }

    fn remove(&mut self, face: BlockFace) -> Option<ElectricalNode> {
        let idx = face_index(face);
        self.slots[idx].take()
    }

    fn is_empty(&self) -> bool {
        self.slots.iter().all(|slot| slot.is_none())
    }

    pub fn iter(&self) -> impl Iterator<Item = (BlockFace, &ElectricalNode)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(idx, slot)| slot.as_ref().map(|node| (face_from_index(idx), node)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (BlockFace, &mut ElectricalNode)> {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(idx, slot)| slot.as_mut().map(move |node| (face_from_index(idx), node)))
    }
}

pub struct ElectricalSystem {
    nodes: HashMap<BlockPos3, FaceNodes>,
    networks: Vec<ElectricalNetwork>,
    dirty_blocks: HashSet<BlockPos3>,
}

impl ElectricalSystem {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            networks: Vec::new(),
            dirty_blocks: HashSet::new(),
        }
    }

    /// Called whenever a world block changes.
    pub fn update_block(
        &mut self,
        chunk: ChunkPos,
        local_pos: (usize, usize, usize),
        block: BlockType,
    ) {
        self.update_block_with(chunk, local_pos, block, None, None, None);
    }

    pub fn update_block_with(
        &mut self,
        chunk: ChunkPos,
        local_pos: (usize, usize, usize),
        block: BlockType,
        axis_hint: Option<Axis>,
        face_hint: Option<BlockFace>,
        params_override: Option<ComponentParams>,
    ) {
        let world_pos = BlockPos3::new(
            chunk.x * CHUNK_SIZE as i32 + local_pos.0 as i32,
            local_pos.1 as i32,
            chunk.z * CHUNK_SIZE as i32 + local_pos.2 as i32,
        );

        if let Some(component) = ElectricalComponent::from_block(block) {
            let default_face = if component == ElectricalComponent::Ground {
                BlockFace::Bottom
            } else {
                BlockFace::Top
            };
            let face = face_hint.unwrap_or(default_face);
            let mut axis = self.infer_axis(world_pos, face, component, axis_hint);
            axis = sanitize_axis(axis, face, component);
            let params = params_override.unwrap_or_else(|| component.default_params());
            let entry = self.nodes.entry(world_pos).or_default();
            entry.set(
                face,
                ElectricalNode {
                    component,
                    chunk,
                    axis,
                    face,
                    params,
                    telemetry: ComponentTelemetry::default(),
                },
            );
            self.dirty_blocks.insert(world_pos);
        } else {
            let removed = if let Some(face) = face_hint {
                self.remove_component(world_pos, face)
            } else {
                self.remove_all_components(world_pos)
            };
            if removed {
                self.dirty_blocks.insert(world_pos);
            }
        }
    }

    pub fn remove_component(&mut self, world_pos: BlockPos3, face: BlockFace) -> bool {
        if let Some(entry) = self.nodes.get_mut(&world_pos) {
            let removed = entry.remove(face).is_some();
            if removed {
                if entry.is_empty() {
                    self.nodes.remove(&world_pos);
                }
                self.dirty_blocks.insert(world_pos);
            }
            removed
        } else {
            false
        }
    }

    pub fn remove_all_components(&mut self, world_pos: BlockPos3) -> bool {
        if let Some(entry) = self.nodes.remove(&world_pos) {
            if !entry.is_empty() {
                self.dirty_blocks.insert(world_pos);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn set_axis(&mut self, world_pos: BlockPos3, face: BlockFace, axis: Axis) {
        if let Some(entry) = self.nodes.get_mut(&world_pos) {
            if let Some(node) = entry.get_mut(face) {
                let sanitized = sanitize_axis(axis, node.face, node.component);
                if node.axis != sanitized {
                    node.axis = sanitized;
                    self.dirty_blocks.insert(world_pos);
                }
            }
        }
    }

    pub fn set_params(&mut self, world_pos: BlockPos3, face: BlockFace, params: ComponentParams) {
        if let Some(entry) = self.nodes.get_mut(&world_pos) {
            if let Some(node) = entry.get_mut(face) {
                if node.params != params {
                    node.params = params;
                    self.dirty_blocks.insert(world_pos);
                }
            }
        }
    }

    pub fn axis_at(&self, world_pos: BlockPos3, face: BlockFace) -> Option<Axis> {
        self.nodes
            .get(&world_pos)
            .and_then(|entry| entry.get(face))
            .map(|node| node.axis)
    }

    pub fn params_at(&self, world_pos: BlockPos3, face: BlockFace) -> Option<ComponentParams> {
        self.nodes
            .get(&world_pos)
            .and_then(|entry| entry.get(face))
            .map(|node| node.params)
    }

    pub fn component_at(
        &self,
        world_pos: BlockPos3,
        face: BlockFace,
    ) -> Option<ElectricalComponent> {
        self.nodes
            .get(&world_pos)
            .and_then(|entry| entry.get(face))
            .map(|node| node.component)
    }

    pub fn telemetry_at(
        &self,
        world_pos: BlockPos3,
        face: BlockFace,
    ) -> Option<ComponentTelemetry> {
        self.nodes
            .get(&world_pos)
            .and_then(|entry| entry.get(face))
            .map(|node| node.telemetry)
    }

    pub fn powered_nodes(
        &self,
        min_current: f32,
    ) -> Vec<(BlockPos3, ElectricalComponent, ComponentTelemetry)> {
        let threshold = min_current.abs();
        let mut powered = Vec::new();
        for (pos, faces) in &self.nodes {
            let mut strongest: Option<(ElectricalComponent, ComponentTelemetry)> = None;
            for (_, node) in faces.iter() {
                let telemetry = node.telemetry;
                if telemetry.current.abs() >= threshold {
                    match &mut strongest {
                        Some((_, best)) if telemetry.current.abs() <= best.current.abs() => {}
                        _ => strongest = Some((node.component, telemetry)),
                    }
                }
            }
            if let Some(entry) = strongest {
                powered.push((*pos, entry.0, entry.1));
            }
        }
        powered
    }

    pub fn connection_mask(&self, world_pos: BlockPos3, face: BlockFace) -> Option<[bool; 6]> {
        let faces = self.nodes.get(&world_pos)?;
        let node = faces.get(face)?;
        let connectors = node.connectors();
        let mut mask = [false; 6];

        for (idx, has_connector) in connectors.iter().enumerate() {
            if !*has_connector {
                continue;
            }
            let neighbor_pos = world_pos.offset(NEIGHBOR_DIRS[idx]);
            let opposite = opposite_index(idx);
            if let Some(neighbors) = self.nodes.get(&neighbor_pos) {
                if neighbors
                    .iter()
                    .any(|(_, node)| node.connectors()[opposite])
                {
                    mask[idx] = true;
                }
            }
        }

        for (other_face, other_node) in faces.iter() {
            if other_face == face {
                continue;
            }
            let other_connectors = other_node.connectors();
            for (idx, has_connector) in connectors.iter().enumerate() {
                if *has_connector && other_connectors[idx] {
                    mask[idx] = true;
                }
            }
        }

        Some(mask)
    }

    pub(crate) fn face_nodes(&self, world_pos: BlockPos3) -> Option<&FaceNodes> {
        self.nodes.get(&world_pos)
    }

    pub fn tick(&mut self) {
        if self.dirty_blocks.is_empty() {
            return;
        }

        self.rebuild_networks();
        self.update_telemetry();
        self.dirty_blocks.clear();
    }

    #[allow(dead_code)]
    pub fn networks(&self) -> &[ElectricalNetwork] {
        &self.networks
    }

    fn infer_axis(
        &self,
        world_pos: BlockPos3,
        face: BlockFace,
        component: ElectricalComponent,
        hint: Option<Axis>,
    ) -> Axis {
        if let Some(axis) = hint {
            return axis;
        }
        if let Some(existing) = self.nodes.get(&world_pos).and_then(|entry| entry.get(face)) {
            return existing.axis;
        }

        // First check for intra-block connections (same block, different faces)
        if let Some(entry) = self.nodes.get(&world_pos) {
            for &candidate in preferred_axes(component).iter() {
                if candidate == face.axis() {
                    continue;
                }
                let candidate_connectors = axis_pair_connectors(candidate);
                let mut shares_edge = false;
                for (other_face, other_node) in entry.iter() {
                    if other_face == face {
                        continue;
                    }
                    let other_connectors = other_node.connectors();
                    if candidate_connectors
                        .iter()
                        .enumerate()
                        .any(|(idx, present)| *present && other_connectors[idx])
                    {
                        shares_edge = true;
                        break;
                    }
                }
                if shares_edge {
                    return candidate;
                }
            }
        }

        // Check all external neighbors and count potential connections for each axis
        let mut axis_scores: [(Axis, usize); 3] = [
            (Axis::X, 0),
            (Axis::Y, 0),
            (Axis::Z, 0),
        ];

        for (idx, dir) in NEIGHBOR_DIRS.iter().enumerate() {
            let neighbor_pos = world_pos.offset(*dir);
            let opposite = opposite_index(idx);

            if let Some(neighbors) = self.nodes.get(&neighbor_pos) {
                // Check if any neighbor at this position can connect
                let has_compatible_neighbor = neighbors
                    .iter()
                    .any(|(_, node)| node.connectors()[opposite]);

                if has_compatible_neighbor {
                    // Determine which axis this direction belongs to
                    let axis_for_dir = Axis::from_connector_index(idx);

                    // Increment score for this axis
                    for (axis, score) in axis_scores.iter_mut() {
                        if *axis == axis_for_dir {
                            *score += 1;
                            break;
                        }
                    }
                }
            }
        }

        // Filter out the face's axis and sort by score (highest first), then by preference
        let face_axis = face.axis();
        let preferred = preferred_axes(component);

        axis_scores.sort_by(|a, b| {
            // First, exclude face axis
            if a.0 == face_axis && b.0 != face_axis {
                return std::cmp::Ordering::Greater;
            }
            if b.0 == face_axis && a.0 != face_axis {
                return std::cmp::Ordering::Less;
            }

            // Then sort by score (descending)
            match b.1.cmp(&a.1) {
                std::cmp::Ordering::Equal => {
                    // If scores are equal, use preference order
                    let a_pref = preferred.iter().position(|&x| x == a.0).unwrap_or(999);
                    let b_pref = preferred.iter().position(|&x| x == b.0).unwrap_or(999);
                    a_pref.cmp(&b_pref)
                }
                other => other,
            }
        });

        // Return the best axis if it has at least one connection, otherwise use default
        if axis_scores[0].0 != face_axis && axis_scores[0].1 > 0 {
            axis_scores[0].0
        } else {
            // No neighbors found, use default axis (but not the face axis)
            for &candidate in preferred.iter() {
                if candidate != face_axis {
                    return candidate;
                }
            }
            component.default_axis()
        }
    }

    fn rebuild_networks(&mut self) {
        self.networks.clear();
        let mut visited: HashSet<AttachmentKey> = HashSet::new();

        for (&pos, faces) in &self.nodes {
            for (face, _) in faces.iter() {
                let start = AttachmentKey { pos, face };
                if visited.contains(&start) {
                    continue;
                }

                let mut queue = VecDeque::new();
                queue.push_back(start);

                let mut network = ElectricalNetwork::default();

                while let Some(current) = queue.pop_front() {
                    if !visited.insert(current) {
                        continue;
                    }

                    let Some(current_node) = self.node_ref(current) else {
                        continue;
                    };

                    match current_node.component {
                        ElectricalComponent::VoltageSource => network.has_source = true,
                        ElectricalComponent::Ground => network.has_ground = true,
                        ElectricalComponent::Wire | ElectricalComponent::Resistor => {}
                    }

                    network.elements.push(NetworkElement {
                        position: current.pos,
                        component: current_node.component,
                        axis: current_node.axis,
                        face: current.face,
                        params: current_node.params,
                    });

                    let connectors = current_node.connectors();
                    for (idx, dir) in NEIGHBOR_DIRS.iter().enumerate() {
                        if !connectors[idx] {
                            continue;
                        }
                        let neighbor_pos = current.pos.offset(*dir);
                        let opposite = opposite_index(idx);
                        if let Some(neighbors) = self.nodes.get(&neighbor_pos) {
                            for (neighbor_face, neighbor_node) in neighbors.iter() {
                                if !neighbor_node.connectors()[opposite] {
                                    continue;
                                }
                                let neighbor_key = AttachmentKey {
                                    pos: neighbor_pos,
                                    face: neighbor_face,
                                };
                                if visited.contains(&neighbor_key) {
                                    continue;
                                }
                                queue.push_back(neighbor_key);
                            }
                        }
                    }

                    if let Some(entry) = self.nodes.get(&current.pos) {
                        for (other_face, other_node) in entry.iter() {
                            if other_face == current.face {
                                continue;
                            }
                            let other_connectors = other_node.connectors();
                            let mut shared = false;
                            for (idx, has_connector) in connectors.iter().enumerate() {
                                if *has_connector && other_connectors[idx] {
                                    shared = true;
                                    break;
                                }
                            }
                            if shared {
                                let neighbor_key = AttachmentKey {
                                    pos: current.pos,
                                    face: other_face,
                                };
                                if !visited.contains(&neighbor_key) {
                                    queue.push_back(neighbor_key);
                                }
                            }
                        }
                    }
                }

                if !network.elements.is_empty() {
                    self.networks.push(network);
                }
            }
        }
    }

    fn node_ref(&self, key: AttachmentKey) -> Option<&ElectricalNode> {
        self.nodes
            .get(&key.pos)
            .and_then(|entry| entry.get(key.face))
    }

    fn node_mut(&mut self, key: AttachmentKey) -> Option<&mut ElectricalNode> {
        self.nodes
            .get_mut(&key.pos)
            .and_then(|entry| entry.get_mut(key.face))
    }

    fn update_telemetry(&mut self) {
        for faces in self.nodes.values_mut() {
            for (_, node) in faces.iter_mut() {
                node.telemetry = ComponentTelemetry::default();
            }
        }

        let mut telemetry_updates = Vec::new();

        for network in &self.networks {
            let has_loop = network.has_source && network.has_ground;

            // Count voltage sources for validation
            let voltage_sources: Vec<_> = network
                .elements
                .iter()
                .filter(|el| el.component == ElectricalComponent::VoltageSource)
                .collect();

            // Get source voltage (if multiple sources, sum them - series connection)
            let source_voltage = voltage_sources
                .iter()
                .filter_map(|el| el.params.voltage_volts)
                .sum::<f32>();

            // Calculate total resistance
            let total_resistance = network
                .elements
                .iter()
                .filter_map(|el| el.params.resistance_ohms)
                .sum::<f32>();

            // Ensure minimum resistance to avoid division by zero or unrealistic currents
            let effective_resistance = total_resistance.max(0.01);

            // Calculate theoretical current - only flows if we have a complete loop (source AND ground)
            let mut current = if has_loop {
                source_voltage / effective_resistance
            } else {
                0.0
            };

            // Short circuit detection: Check if current exceeds any component's max_current
            // Find the most restrictive current limit in the network
            let mut is_short_circuit = false;
            if current > 0.0 {
                let min_max_current = network
                    .elements
                    .iter()
                    .filter_map(|el| el.params.max_current_amps)
                    .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

                if let Some(max_current) = min_max_current {
                    if current > max_current {
                        // Short circuit detected! Limit current to max or cut it off entirely
                        // For realistic behavior, we'll cut the current to simulate a blown fuse/breaker
                        is_short_circuit = true;
                        current = 0.0; // Circuit breaker trips, no current flows
                    }
                }

                // Additional check: if resistance is extremely low (< 0.1 ohms) and current is very high
                // This catches cases where max_current might not be set properly
                if total_resistance < 0.1 && current > 100.0 {
                    is_short_circuit = true;
                    current = 0.0;
                }
            }

            // Update telemetry for each element in the network
            for element in &network.elements {
                let key = AttachmentKey {
                    pos: element.position,
                    face: element.face,
                };

                let voltage = if is_short_circuit {
                    // In a short circuit, voltage drops to near zero
                    0.0
                } else if element.component == ElectricalComponent::VoltageSource {
                    // Voltage source shows its source voltage
                    source_voltage
                } else if let Some(resistance) = element.params.resistance_ohms {
                    // Other components show voltage drop across them (V = I * R)
                    current * resistance
                } else {
                    0.0
                };

                telemetry_updates.push((key, ComponentTelemetry { current, voltage }));
            }
        }

        for (key, telemetry) in telemetry_updates {
            if let Some(node) = self.node_mut(key) {
                node.telemetry = telemetry;
            }
        }
    }
}

fn axis_pair_connectors(axis: Axis) -> [bool; 6] {
    let mut connectors = [false; 6];
    let (a, b) = axis.pair_indices();
    connectors[a] = true;
    connectors[b] = true;
    connectors
}

fn preferred_axes(component: ElectricalComponent) -> [Axis; 3] {
    match component {
        ElectricalComponent::Wire
        | ElectricalComponent::Resistor
        | ElectricalComponent::VoltageSource => [Axis::X, Axis::Z, Axis::Y],
        ElectricalComponent::Ground => [Axis::Y, Axis::X, Axis::Z],
    }
}

fn sanitize_axis(mut axis: Axis, face: BlockFace, component: ElectricalComponent) -> Axis {
    if axis != face.axis() {
        return axis;
    }
    for candidate in preferred_axes(component) {
        if candidate != face.axis() {
            axis = candidate;
            break;
        }
    }
    if axis == face.axis() {
        axis = match face.axis() {
            Axis::X => Axis::Y,
            Axis::Y => Axis::X,
            Axis::Z => Axis::Y,
        };
    }
    axis
}

fn face_from_index(idx: usize) -> BlockFace {
    match idx {
        0 => BlockFace::East,
        1 => BlockFace::West,
        2 => BlockFace::Top,
        3 => BlockFace::Bottom,
        4 => BlockFace::South,
        5 => BlockFace::North,
        _ => BlockFace::Top,
    }
}

fn face_index(face: BlockFace) -> usize {
    match face {
        BlockFace::East => 0,
        BlockFace::West => 1,
        BlockFace::Top => 2,
        BlockFace::Bottom => 3,
        BlockFace::South => 4,
        BlockFace::North => 5,
    }
}

fn opposite_index(idx: usize) -> usize {
    match idx {
        0 => 1,
        1 => 0,
        2 => 3,
        3 => 2,
        4 => 5,
        5 => 4,
        _ => unreachable!(),
    }
}
