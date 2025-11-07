mod block;
mod camera;
mod chunk;
mod electric;
mod fluid_gpu;
mod fluid_system;
mod inventory;
mod mesh;
mod npu;
mod profiler;
mod raycast;
mod renderer;
mod texture;
mod world;

use std::collections::HashSet;
use std::time::Instant;

use anyhow::Context;
use camera::{
    Camera, CameraController, Projection, PLAYER_EYE_HEIGHT, PLAYER_HEIGHT, PLAYER_RADIUS,
};
use cgmath::{point3, Rad, Vector3};
use fluid_system::FluidSystem;
use inventory::{Inventory, AVAILABLE_BLOCKS, HOTBAR_SIZE};
use renderer::{Renderer, UiVertex};
use winit::{
    event::*,
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowBuilder},
};
use world::{ChunkPos, World, MAX_FLUID_LEVEL};

use crate::block::{Axis, BlockFace, BlockType};
use crate::chunk::{CHUNK_HEIGHT, CHUNK_SIZE};
use crate::electric::{BlockPos3, ComponentParams, ComponentTelemetry, ElectricalComponent};
use crate::raycast::{raycast, RaycastHit};
use crate::texture::atlas_uv_bounds;

const INVENTORY_COLS: usize = 3;
const INVENTORY_ROWS: usize = 3;
const INVENTORY_SLOT_COUNT: usize = INVENTORY_COLS * INVENTORY_ROWS;
const INVENTORY_SLOT_SIZE: f32 = 0.072;
const INVENTORY_SLOT_GAP: f32 = 0.018;
const INVENTORY_START_X: f32 = 0.22;
const INVENTORY_START_Y: f32 = 0.34;
const INVENTORY_ICON_PAD: f32 = 0.006;
const PALETTE_COLS: usize = 6;
const PALETTE_SLOT_SIZE: f32 = 0.048;
const PALETTE_SLOT_GAP: f32 = 0.016;
const PALETTE_ICON_PAD: f32 = 0.006;
#[allow(dead_code)]
const DRAG_ICON_SIZE: f32 = 0.05;
const UI_REFERENCE_ASPECT: f32 = 16.0 / 9.0;
const FILTER_CHIP_HEIGHT: f32 = 0.034;
const FILTER_CHIP_GAP: f32 = 0.012;
const FILTER_AREA_PADDING_X: f32 = 0.02;
const FILTER_AREA_PADDING_Y: f32 = 0.02;
const SEARCH_FIELD_HEIGHT: f32 = 0.038;
const SEARCH_FIELD_PADDING: f32 = 0.012;

struct PaletteCategory {
    name: &'static str,
    blocks: &'static [BlockType],
}

const CATEGORY_TERRAIN: &[BlockType] = &[
    BlockType::Grass,
    BlockType::Dirt,
    BlockType::Stone,
    BlockType::Sand,
    BlockType::Terracotta,
    BlockType::Snow,
];

const CATEGORY_FOLIAGE: &[BlockType] = &[
    BlockType::Leaves,
    BlockType::FlowerRose,
    BlockType::FlowerTulip,
    BlockType::LilyPad,
    BlockType::Wood,
];

const CATEGORY_ORES: &[BlockType] = &[BlockType::CoalOre, BlockType::IronOre];

const CATEGORY_FLUIDS: &[BlockType] = &[BlockType::Water];

const CATEGORY_ELECTRICAL: &[BlockType] = &[
    BlockType::CopperWire,
    BlockType::Resistor,
    BlockType::VoltageSource,
    BlockType::Ground,
];

const PALETTE_CATEGORIES: &[PaletteCategory] = &[
    PaletteCategory {
        name: "All",
        blocks: &AVAILABLE_BLOCKS,
    },
    PaletteCategory {
        name: "Terrain",
        blocks: CATEGORY_TERRAIN,
    },
    PaletteCategory {
        name: "Foliage",
        blocks: CATEGORY_FOLIAGE,
    },
    PaletteCategory {
        name: "Ores",
        blocks: CATEGORY_ORES,
    },
    PaletteCategory {
        name: "Fluids",
        blocks: CATEGORY_FLUIDS,
    },
    PaletteCategory {
        name: "Electrical",
        blocks: CATEGORY_ELECTRICAL,
    },
];

type Rect = ((f32, f32), (f32, f32));

struct InventoryLayout {
    panel: Rect,
    header: Rect,
    hotbar_panel: Rect,
    palette_panel: Rect,
    instructions_panel: Rect,
    search_rect: Rect,
    search_clear_rect: Rect,
    chip_rects: Vec<Rect>,
    palette_content_origin: (f32, f32),
    palette_view_height: f32,
}

const FIXED_TICK_RATE: f32 = 60.0;
const FIXED_TICK_STEP: f32 = 1.0 / FIXED_TICK_RATE;
const MAX_TICKS_PER_FRAME: usize = 6;
const WATER_UPDATE_INTERVAL: u32 = 10; // Water updates every 10 ticks (6 times per second)

fn ui_width(value: f32) -> f32 {
    value / UI_REFERENCE_ASPECT
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AttachmentTarget {
    pos: BlockPos3,
    face: BlockFace,
}

fn block_face_name(face: BlockFace) -> &'static str {
    match face {
        BlockFace::Top => "Up (+Y)",
        BlockFace::Bottom => "Down (-Y)",
        BlockFace::North => "North (-Z)",
        BlockFace::South => "South (+Z)",
        BlockFace::East => "East (+X)",
        BlockFace::West => "West (-X)",
    }
}

fn axis_name(axis: Axis) -> &'static str {
    match axis {
        Axis::X => "X-axis",
        Axis::Y => "Y-axis",
        Axis::Z => "Z-axis",
    }
}

#[derive(Clone, PartialEq)]
struct InspectInfo {
    handle: AttachmentTarget,
    label: String,
    component: ElectricalComponent,
    axis: Axis,
    positive_face: BlockFace,
    negative_face: BlockFace,
    params: ComponentParams,
    telemetry: ComponentTelemetry,
}

#[derive(Clone)]
struct ConfigEditor {
    handle: AttachmentTarget,
    label: String,
    component: ElectricalComponent,
    params: ComponentParams,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsTab {
    Display,
    Audio,
    Controls,
}

impl SettingsTab {
    const ALL: [Self; 3] = [Self::Display, Self::Audio, Self::Controls];

    fn label(self) -> &'static str {
        match self {
            Self::Display => "DISPLAY",
            Self::Audio => "AUDIO",
            Self::Controls => "CONTROLS",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Display => 0,
            Self::Audio => 1,
            Self::Controls => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HotbarState {
    Normal,
    Noclip,
    Underwater,
}

struct HotbarStatusData {
    label: &'static str,
    detail: Option<&'static str>,
    chip_fill: [f32; 4],
    chip_text: [f32; 4],
}

struct HotbarTheme {
    panel_border: [f32; 4],
    panel_fill: [f32; 4],
    panel_highlight: [f32; 4],
    slot_default: [f32; 4],
    slot_selected: [f32; 4],
    status: Option<HotbarStatusData>,
}

struct State<'window> {
    window: &'window Window,
    renderer: Renderer<'window>,
    fluid_system: FluidSystem,
    world: World,
    camera: Camera,
    projection: Projection,
    controller: CameraController,
    modifiers: Modifiers,
    inventory: Inventory,
    inventory_cursor: usize,
    inventory_hover_slot: Option<usize>,
    inventory_palette_hover: Option<usize>,
    inventory_cursor_pos: Option<(f32, f32)>,
    inventory_drag_origin: Option<usize>,
    inventory_drag_block: Option<BlockType>,
    inventory_swap_slot: Option<usize>,
    inventory_last_hover_slot: Option<usize>,
    inventory_last_hover_palette: Option<usize>,
    inventory_filter_chip_hover: Option<usize>,
    inventory_active_category: usize,
    inventory_search_query: String,
    inventory_search_active: bool,
    inventory_palette_scroll: f32,
    inventory_palette_filtered: Vec<BlockType>,
    highlight_target: Option<AttachmentTarget>,
    inspect_info: Option<InspectInfo>,
    config_editor: Option<ConfigEditor>,
    last_frame: Instant,
    tick_accumulator: f32,
    animation_time: f32,
    debug_tick_counter: u32,
    water_tick_counter: u32,
    mouse_grabbed: bool,
    world_dirty: bool,
    dirty_chunks: HashSet<ChunkPos>,
    force_full_remesh: bool,
    debug_mode: bool,
    paused: bool,
    inventory_open: bool,
    menu_restore_mouse: bool,
    ui_dirty: bool,
    ui_scaler: UiScaler,
    settings_open: bool,
    settings_selected_tab: SettingsTab,
    settings_focus_index: usize,
    settings_fov_deg: f32,
    settings_sensitivity: f32,
    settings_volume: f32,
}

impl<'window> State<'window> {
    fn is_in_menu(&self) -> bool {
        self.paused || self.inventory_open || self.config_editor.is_some() || self.settings_open
    }

    fn mark_ui_dirty(&mut self) {
        self.ui_dirty = true;
    }

    fn rebuild_ui(&mut self) {
        let geometry = self.build_ui_geometry();
        self.renderer
            .update_ui(&geometry.vertices, &geometry.indices);
        self.ui_dirty = false;
    }

    fn enter_menu_mode(&mut self) {
        if !self.is_in_menu() {
            self.menu_restore_mouse = self.mouse_grabbed;
            if self.mouse_grabbed {
                self.set_mouse_grab(false);
            }
        }
    }

    fn exit_menu_mode_if_needed(&mut self) {
        if !self.is_in_menu() && self.menu_restore_mouse {
            self.set_mouse_grab(true);
            self.menu_restore_mouse = false;
        }
    }

    fn open_pause(&mut self) {
        if self.paused {
            return;
        }
        if self.inventory_open {
            self.inventory_open = false;
        }
        self.enter_menu_mode();
        self.paused = true;
        self.settings_open = false;
        self.settings_selected_tab = SettingsTab::Display;
        self.settings_focus_index = 0;
        self.mark_ui_dirty();
        println!("--- Paused ---\nPress Esc to resume. Press S for settings.");
    }

    fn close_pause(&mut self) {
        if !self.paused {
            return;
        }
        self.paused = false;
        self.settings_open = false;
        self.exit_menu_mode_if_needed();
        self.mark_ui_dirty();
        println!("Resumed.");
    }

    fn open_inventory(&mut self) {
        if self.inventory_open {
            return;
        }
        if self.paused {
            self.close_pause();
        }
        self.enter_menu_mode();
        self.inventory_open = true;
        self.inventory_cursor = self.inventory.selected_slot_index().min(HOTBAR_SIZE - 1);
        self.inventory_swap_slot = None;
        self.inventory_hover_slot = None;
        self.inventory_palette_hover = None;
        self.inventory_cursor_pos = None;
        self.inventory_drag_origin = None;
        self.inventory_drag_block = None;
        self.inventory_last_hover_slot = None;
        self.inventory_last_hover_palette = None;
        self.inventory_filter_chip_hover = None;
        self.inventory_search_active = false;
        self.inventory_search_query.clear();
        self.inventory_active_category = 0;
        self.inventory_palette_scroll = 0.0;
        self.refresh_palette_filter();
        self.mark_ui_dirty();
        println!("Inventory opened (press E to close).");
    }

    fn close_inventory(&mut self) {
        if !self.inventory_open {
            return;
        }
        self.cancel_inventory_drag();
        self.inventory_open = false;
        self.inventory_swap_slot = None;
        self.inventory_hover_slot = None;
        self.inventory_palette_hover = None;
        self.inventory_filter_chip_hover = None;
        self.inventory_cursor_pos = None;
        self.inventory_drag_origin = None;
        self.inventory_drag_block = None;
        self.inventory_last_hover_slot = None;
        self.inventory_last_hover_palette = None;
        self.inventory_search_active = false;
        self.exit_menu_mode_if_needed();
        self.mark_ui_dirty();
        println!("Inventory closed.");
    }

    fn open_settings(&mut self) {
        if !self.paused {
            self.open_pause();
        }
        if self.settings_open {
            return;
        }
        self.enter_menu_mode();
        self.settings_open = true;
        self.settings_selected_tab = SettingsTab::Display;
        self.settings_focus_index = 0;
        self.settings_fov_deg = self.settings_fov_deg.clamp(60.0, 100.0);
        self.settings_sensitivity = self.controller.sensitivity();
        self.mark_ui_dirty();
    }

    fn close_settings(&mut self) {
        if !self.settings_open {
            return;
        }
        self.settings_open = false;
        self.mark_ui_dirty();
    }

    fn handle_settings_key(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Escape => {
                self.close_settings();
                true
            }
            KeyCode::Tab => {
                self.cycle_settings_tab(1);
                true
            }
            KeyCode::ArrowLeft => {
                self.adjust_setting(-1.0);
                true
            }
            KeyCode::ArrowRight => {
                self.adjust_setting(1.0);
                true
            }
            KeyCode::ArrowUp => {
                self.move_settings_focus(-1);
                true
            }
            KeyCode::ArrowDown => {
                self.move_settings_focus(1);
                true
            }
            _ => false,
        }
    }

    fn cycle_settings_tab(&mut self, delta: i32) {
        let current = self.settings_selected_tab.index() as i32;
        let next = (current + delta).rem_euclid(SettingsTab::ALL.len() as i32) as usize;
        self.settings_selected_tab = SettingsTab::ALL[next];
        let count = self.settings_focus_count();
        if count == 0 {
            self.settings_focus_index = 0;
        } else if self.settings_focus_index >= count {
            self.settings_focus_index = count - 1;
        }
        self.mark_ui_dirty();
    }

    fn settings_focus_count(&self) -> usize {
        match self.settings_selected_tab {
            SettingsTab::Display => 2,
            SettingsTab::Audio => 1,
            SettingsTab::Controls => 0,
        }
    }

    fn move_settings_focus(&mut self, delta: i32) {
        let count = self.settings_focus_count();
        if count == 0 {
            return;
        }
        let current = self.settings_focus_index as i32;
        let next = (current + delta).rem_euclid(count as i32) as usize;
        if next != self.settings_focus_index {
            self.settings_focus_index = next;
            self.mark_ui_dirty();
        }
    }

    fn adjust_setting(&mut self, delta: f32) {
        match self.settings_selected_tab {
            SettingsTab::Display => match self.settings_focus_index {
                0 => {
                    self.settings_fov_deg = (self.settings_fov_deg + delta).clamp(60.0, 100.0);
                    self.apply_display_settings();
                }
                1 => {
                    let step = 0.00025;
                    self.settings_sensitivity =
                        (self.settings_sensitivity + delta * step).clamp(0.0005, 0.02);
                    self.apply_display_settings();
                }
                _ => {}
            },
            SettingsTab::Audio => {
                self.settings_volume = (self.settings_volume + delta * 0.05).clamp(0.0, 1.0);
                self.mark_ui_dirty();
            }
            SettingsTab::Controls => {}
        }
    }

    fn apply_display_settings(&mut self) {
        self.projection
            .set_target_fov(Rad(self.settings_fov_deg.to_radians()));
        self.controller.set_sensitivity(self.settings_sensitivity);
        self.renderer.update_camera(&self.camera, &self.projection);
        self.mark_ui_dirty();
    }

    fn hotbar_state(&self) -> HotbarState {
        if self.controller.noclip {
            HotbarState::Noclip
        } else if self.player_is_submerged() {
            HotbarState::Underwater
        } else {
            HotbarState::Normal
        }
    }

    fn hotbar_theme(&self) -> HotbarTheme {
        match self.hotbar_state() {
            HotbarState::Normal => HotbarTheme {
                panel_border: [0.06, 0.07, 0.12, 0.96],
                panel_fill: [0.04, 0.05, 0.08, 0.88],
                panel_highlight: [0.34, 0.52, 0.86, 0.28],
                slot_default: [0.16, 0.19, 0.27, 0.88],
                slot_selected: [0.28, 0.36, 0.55, 0.95],
                status: None,
            },
            HotbarState::Noclip => HotbarTheme {
                panel_border: [0.14, 0.08, 0.24, 0.96],
                panel_fill: [0.1, 0.05, 0.18, 0.9],
                panel_highlight: [0.54, 0.38, 0.86, 0.32],
                slot_default: [0.2, 0.13, 0.28, 0.88],
                slot_selected: [0.48, 0.34, 0.7, 0.95],
                status: Some(HotbarStatusData {
                    label: "NOCLIP MODE",
                    detail: Some("Press F to toggle"),
                    chip_fill: [0.46, 0.24, 0.6, 0.95],
                    chip_text: [0.96, 0.94, 1.0, 1.0],
                }),
            },
            HotbarState::Underwater => HotbarTheme {
                panel_border: [0.05, 0.16, 0.2, 0.96],
                panel_fill: [0.04, 0.12, 0.16, 0.9],
                panel_highlight: [0.22, 0.48, 0.7, 0.32],
                slot_default: [0.12, 0.18, 0.24, 0.88],
                slot_selected: [0.26, 0.52, 0.7, 0.95],
                status: Some(HotbarStatusData {
                    label: "IN WATER",
                    detail: Some("Swim to recover breath"),
                    chip_fill: [0.18, 0.48, 0.66, 0.95],
                    chip_text: [0.9, 0.97, 1.0, 1.0],
                }),
            },
        }
    }

    fn player_is_submerged(&self) -> bool {
        let pos = self.camera.position;
        let x = pos.x.floor() as i32;
        let y = pos.y.floor() as i32;
        let z = pos.z.floor() as i32;
        matches!(self.world.get_block(x, y, z), BlockType::Water)
    }

    fn new(window: &'window Window) -> anyhow::Result<Self> {
        let size = window.inner_size();

        let projection =
            Projection::new(size.width, size.height, 45.0_f32.to_radians(), 0.1, 1000.0);
        let ui_scaler = UiScaler::new(projection.aspect());
        let settings_fov_deg = projection.base_fov().0.to_degrees();

        let renderer = Renderer::new(&window).context("failed to create renderer")?;
        let fluid_system = FluidSystem::new(renderer.device_handle(), renderer.queue_handle());
        let mut world = World::new();

        let spawn_x = 0.5;
        let spawn_z = 0.5;
        let mut camera = Camera::new(point3(spawn_x, 30.0, spawn_z), Rad(0.0), Rad(-0.3));
        let controller = CameraController::new(15.0, 0.0025);
        let settings_sensitivity = controller.sensitivity();
        let settings_volume = 0.8;
        let inventory = Inventory::new();

        let _ = world.update_loaded_chunks(camera.position, 3);

        let column_x = camera.position.x.floor() as i32;
        let column_z = camera.position.z.floor() as i32;
        if let Some(surface_y) = find_surface_level(&world, column_x, column_z) {
            camera.position.y = surface_y + PLAYER_EYE_HEIGHT + 0.05;
        }
        for _ in 0..50 {
            if !player_aabb_collides(&world, camera.position) {
                break;
            }
            camera.position.y += 0.1;
        }

        let mut state = Self {
            window,
            renderer,
            fluid_system,
            world,
            camera,
            projection,
            controller,
            modifiers: Modifiers::default(),
            inventory,
            inventory_cursor: 0,
            inventory_hover_slot: None,
            inventory_palette_hover: None,
            inventory_cursor_pos: None,
            inventory_drag_origin: None,
            inventory_drag_block: None,
            inventory_swap_slot: None,
            inventory_last_hover_slot: None,
            inventory_last_hover_palette: None,
            inventory_filter_chip_hover: None,
            inventory_active_category: 0,
            inventory_search_query: String::new(),
            inventory_search_active: false,
            inventory_palette_scroll: 0.0,
            inventory_palette_filtered: Vec::new(),
            last_frame: Instant::now(),
            highlight_target: None,
            inspect_info: None,
            config_editor: None,
            tick_accumulator: 0.0,
            animation_time: 0.0,
            debug_tick_counter: 0,
            water_tick_counter: 0,
            mouse_grabbed: false,
            world_dirty: true,
            dirty_chunks: HashSet::new(),
            force_full_remesh: true,
            debug_mode: false,
            paused: false,
            inventory_open: false,
            menu_restore_mouse: false,
            ui_dirty: true,
            ui_scaler,
            settings_open: false,
            settings_selected_tab: SettingsTab::Display,
            settings_focus_index: 0,
            settings_fov_deg,
            settings_sensitivity,
            settings_volume,
        };

        state.refresh_palette_filter();

        // Generate initial mesh
        state.renderer.rebuild_world_mesh(&state.world);
        state
            .renderer
            .update_camera(&state.camera, &state.projection);
        let initial_sky = state.world.sky_color_at(
            state.camera.position.x.floor() as i32,
            state.camera.position.z.floor() as i32,
        );
        state.renderer.set_clear_color(initial_sky);
        state.world_dirty = false;
        state.force_full_remesh = false;

        // Print initial selection
        state.print_selected();

        state.rebuild_ui();

        Ok(state)
    }

    fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.renderer.resize(new_size, &mut self.projection);
        self.ui_scaler = UiScaler::new(self.projection.aspect());
        self.mark_ui_dirty();
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::KeyboardInput { event, .. } = event {
            if let PhysicalKey::Code(key) = event.physical_key {
                if event.state == ElementState::Pressed {
                    if self.settings_open && self.handle_settings_key(key) {
                        return true;
                    }
                    if self.handle_config_key(key) {
                        return true;
                    }
                    match key {
                        KeyCode::Escape => {
                            if self.settings_open {
                                self.close_settings();
                            } else if self.paused {
                                self.close_pause();
                            } else if self.inventory_open {
                                self.close_inventory();
                                self.close_pause();
                            } else {
                                self.open_pause();
                            }
                            return true;
                        }
                        KeyCode::KeyS => {
                            if self.paused {
                                if self.settings_open {
                                    self.close_settings();
                                } else {
                                    self.open_settings();
                                }
                                return true;
                            }
                        }
                        KeyCode::KeyE => {
                            if self.inventory_open {
                                self.close_inventory();
                            } else if !self.paused {
                                self.open_inventory();
                            }
                            return true;
                        }
                        KeyCode::KeyT => {
                            if self.toggle_config_editor() {
                                return true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if self.inventory_open && self.handle_inventory_input(event) {
            return true;
        }

        if self.is_in_menu() {
            return false;
        }

        if self.controller.process_events(event) {
            return true;
        }

        match event {
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = *mods;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if !self.mouse_grabbed {
                    if *button == MouseButton::Left && *state == ElementState::Pressed {
                        self.set_mouse_grab(true);
                        return true;
                    }
                } else if *state == ElementState::Pressed {
                    match button {
                        MouseButton::Left => {
                            self.break_block();
                            return true;
                        }
                        MouseButton::Right => {
                            self.place_block();
                            return true;
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                if !self.inventory_search_active {
                    return false;
                }
                let mut handled = false;
                for ch in text.chars() {
                    if ch.is_control() {
                        continue;
                    }
                    let ch = ch.to_ascii_uppercase();
                    if !(ch.is_ascii_alphanumeric() || ch == ' ') {
                        continue;
                    }
                    if self.inventory_search_query.len() >= 24 {
                        handled = true;
                        break;
                    }
                    self.inventory_search_query.push(ch);
                    handled = true;
                }
                if handled {
                    self.inventory_palette_scroll = 0.0;
                    self.refresh_palette_filter();
                    return true;
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if let PhysicalKey::Code(key) = event.physical_key {
                        if self.handle_config_key(key) {
                            return true;
                        }
                        match key {
                            KeyCode::Digit1 => {
                                self.inventory.select_slot(0);
                                self.print_selected();
                                self.mark_ui_dirty();
                                return true;
                            }
                            KeyCode::Digit2 => {
                                self.inventory.select_slot(1);
                                self.print_selected();
                                self.mark_ui_dirty();
                                return true;
                            }
                            KeyCode::Digit3 => {
                                self.inventory.select_slot(2);
                                self.print_selected();
                                self.mark_ui_dirty();
                                return true;
                            }
                            KeyCode::Digit4 => {
                                self.inventory.select_slot(3);
                                self.print_selected();
                                self.mark_ui_dirty();
                                return true;
                            }
                            KeyCode::Digit5 => {
                                self.inventory.select_slot(4);
                                self.print_selected();
                                self.mark_ui_dirty();
                                return true;
                            }
                            KeyCode::Digit6 => {
                                self.inventory.select_slot(5);
                                self.print_selected();
                                self.mark_ui_dirty();
                                return true;
                            }
                            KeyCode::Digit7 => {
                                self.inventory.select_slot(6);
                                self.print_selected();
                                self.mark_ui_dirty();
                                return true;
                            }
                            KeyCode::Digit8 => {
                                self.inventory.select_slot(7);
                                self.print_selected();
                                self.mark_ui_dirty();
                                return true;
                            }
                            KeyCode::Digit9 => {
                                self.inventory.select_slot(8);
                                self.print_selected();
                                self.mark_ui_dirty();
                                return true;
                            }
                            KeyCode::KeyF => {
                                self.controller.toggle_noclip();
                                println!("\n========================================");
                                if self.controller.noclip {
                                    println!("NOCLIP ON - Fly mode (no collision/gravity)");
                                } else {
                                    println!("NOCLIP OFF - Collision and gravity enabled");
                                    println!("You will fall until you land on blocks");
                                }
                                println!("========================================\n");
                                return true;
                            }
                            KeyCode::F3 => {
                                self.debug_mode = !self.debug_mode;
                                println!(
                                    "Debug Mode: {}",
                                    if self.debug_mode { "ON" } else { "OFF" }
                                );
                                return true;
                            }
                            _ => {}
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if self.mouse_grabbed {
                    let scroll = match delta {
                        MouseScrollDelta::LineDelta(_, y) => -(*y as i32),
                        MouseScrollDelta::PixelDelta(pos) => -(pos.y.signum() as i32),
                    };
                    self.inventory.cycle_selection(scroll);
                    self.print_selected();
                    self.mark_ui_dirty();
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    fn print_selected(&self) {
        if let Some(block) = self.inventory.selected_block() {
            println!("Selected: {}", block.name());
        } else {
            println!("Selected: Empty");
        }
    }

    fn break_block(&mut self) {
        let direction = self.crosshair_direction();
        if let Some(hit) = raycast(&self.world, self.camera.position, direction, 5.0) {
            let face = BlockFace::from_normal_f32(hit.normal)
                .or_else(|| BlockFace::from_normal_f32(-hit.normal))
                .unwrap_or(BlockFace::Top);
            if self.world.remove_electrical_face(
                hit.block_pos.0,
                hit.block_pos.1,
                hit.block_pos.2,
                face,
            ) {
                self.mark_block_dirty(hit.block_pos.0, hit.block_pos.1, hit.block_pos.2);
                self.refresh_inspect_info();
            } else {
                self.world.set_block(
                    hit.block_pos.0,
                    hit.block_pos.1,
                    hit.block_pos.2,
                    BlockType::Air,
                );
                self.mark_block_dirty(hit.block_pos.0, hit.block_pos.1, hit.block_pos.2);
            }
        }
    }

    fn place_block(&mut self) {
        if let Some(block_type) = self.inventory.selected_block() {
            let direction = self.crosshair_direction();
            if let Some(hit) = raycast(&self.world, self.camera.position, direction, 5.0) {
                if block_type.is_electrical() {
                    self.place_electrical_component(block_type, &hit);
                    return;
                }

                let place_pos = (
                    hit.block_pos.0 + hit.normal.x as i32,
                    hit.block_pos.1 + hit.normal.y as i32,
                    hit.block_pos.2 + hit.normal.z as i32,
                );

                // Don't place block if it would intersect with the player
                // Player bounding box: feet at (camera.y - PLAYER_EYE_HEIGHT), head at (camera.y - PLAYER_EYE_HEIGHT + PLAYER_HEIGHT)
                let player_feet_y = self.camera.position.y - PLAYER_EYE_HEIGHT;
                let player_head_y = player_feet_y + PLAYER_HEIGHT;

                // Define player bounding box with proper radius
                let player_min = (
                    (self.camera.position.x - PLAYER_RADIUS).floor() as i32,
                    player_feet_y.floor() as i32,
                    (self.camera.position.z - PLAYER_RADIUS).floor() as i32,
                );
                let player_max = (
                    (self.camera.position.x + PLAYER_RADIUS).ceil() as i32,
                    player_head_y.ceil() as i32,
                    (self.camera.position.z + PLAYER_RADIUS).ceil() as i32,
                );

                // Check if placement position is INSIDE the player's bounding box (prevent placement if true)
                let intersects_player = place_pos.0 >= player_min.0
                    && place_pos.0 <= player_max.0
                    && place_pos.1 >= player_min.1
                    && place_pos.1 <= player_max.1
                    && place_pos.2 >= player_min.2
                    && place_pos.2 <= player_max.2;

                if intersects_player {
                    // Don't allow placing blocks inside the player
                    return;
                }

                // Check if the target position already has a solid block
                let existing = self.world.get_block(place_pos.0, place_pos.1, place_pos.2);
                if existing.is_solid() {
                    return;
                }

                // Place the block
                if block_type == BlockType::Water {
                    self.world.add_fluid(
                        place_pos.0,
                        place_pos.1,
                        place_pos.2,
                        MAX_FLUID_LEVEL,
                    );
                } else {
                    self.world.set_block_with_axis(
                        place_pos.0,
                        place_pos.1,
                        place_pos.2,
                        block_type,
                        None,
                        None,
                    );
                }
                self.mark_block_dirty(place_pos.0, place_pos.1, place_pos.2);
            }
        }
    }

    fn place_electrical_component(&mut self, block_type: BlockType, hit: &RaycastHit) {
        let Some(face) = BlockFace::from_normal_f32(hit.normal) else {
            return;
        };

        let axis = self.determine_electrical_axis(block_type, face);
        self.world.set_block_with_axis(
            hit.block_pos.0,
            hit.block_pos.1,
            hit.block_pos.2,
            block_type,
            Some(axis),
            Some(face),
        );
        self.mark_block_dirty(hit.block_pos.0, hit.block_pos.1, hit.block_pos.2);
        self.refresh_inspect_info();
    }

    fn mark_block_dirty(&mut self, world_x: i32, _world_y: i32, world_z: i32) {
        self.world_dirty = true;
        if self.force_full_remesh {
            return;
        }

        let chunk_size = CHUNK_SIZE as i32;
        let chunk_x = world_x.div_euclid(chunk_size);
        let chunk_z = world_z.div_euclid(chunk_size);
        let local_x = world_x.rem_euclid(chunk_size);
        let local_z = world_z.rem_euclid(chunk_size);

        self.dirty_chunks.insert(ChunkPos {
            x: chunk_x,
            z: chunk_z,
        });

        if local_x == 0 {
            self.dirty_chunks.insert(ChunkPos {
                x: chunk_x - 1,
                z: chunk_z,
            });
        }
        if local_x == chunk_size - 1 {
            self.dirty_chunks.insert(ChunkPos {
                x: chunk_x + 1,
                z: chunk_z,
            });
        }
        if local_z == 0 {
            self.dirty_chunks.insert(ChunkPos {
                x: chunk_x,
                z: chunk_z - 1,
            });
        }
        if local_z == chunk_size - 1 {
            self.dirty_chunks.insert(ChunkPos {
                x: chunk_x,
                z: chunk_z + 1,
            });
        }
    }

    fn determine_electrical_axis(&self, block_type: BlockType, face: BlockFace) -> Axis {
        if !block_type.is_electrical() {
            return block_type.default_axis();
        }
        match block_type {
            BlockType::Ground => Axis::Y,
            BlockType::VoltageSource | BlockType::Resistor | BlockType::CopperWire => {
                self.axis_in_face_plane(face, self.crosshair_direction())
            }
            _ => block_type.default_axis(),
        }
    }

    fn axis_in_face_plane(&self, face: BlockFace, direction: Vector3<f32>) -> Axis {
        let face_axis = face.axis();
        let candidates: [Axis; 2] = match face_axis {
            Axis::X => [Axis::Z, Axis::Y],
            Axis::Y => [Axis::X, Axis::Z],
            Axis::Z => [Axis::X, Axis::Y],
        };
        let mut best = candidates[0];
        let mut best_value = 0.0;
        for &candidate in &candidates {
            let value = match candidate {
                Axis::X => direction.x.abs(),
                Axis::Y => direction.y.abs(),
                Axis::Z => direction.z.abs(),
            };
            if value > best_value {
                best_value = value;
                best = candidate;
            }
        }
        if best_value < 0.1 {
            best = candidates[0];
        }
        best
    }

    fn crosshair_screen_uv(&self) -> (f32, f32) {
        self.ui_scaler.project((0.5, 0.5))
    }

    fn crosshair_ui_center(&self) -> (f32, f32) {
        self.ui_scaler.unproject(self.crosshair_screen_uv())
    }

    fn crosshair_direction(&self) -> Vector3<f32> {
        let screen = self.crosshair_screen_uv();
        self.projection.ray_direction(&self.camera, screen)
    }

    fn set_mouse_grab(&mut self, grab: bool) {
        if self.mouse_grabbed == grab {
            return;
        }
        self.mouse_grabbed = grab;
        self.window.set_cursor_visible(!grab);
        if grab {
            let _ = self
                .window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| self.window.set_cursor_grab(CursorGrabMode::Confined));
        } else {
            let _ = self.window.set_cursor_grab(CursorGrabMode::None);
        }
        self.ui_dirty = true;
    }

    fn mouse_motion(&mut self, delta: (f64, f64)) {
        if self.mouse_grabbed {
            self.controller.process_mouse(delta, &mut self.camera);
        }
    }

    fn inventory_slot_rect(&self, index: usize) -> Option<((f32, f32), (f32, f32))> {
        if index >= INVENTORY_SLOT_COUNT {
            return None;
        }
        let col = index % INVENTORY_COLS;
        let row = index / INVENTORY_COLS;
        let step_x = ui_width(INVENTORY_SLOT_SIZE + INVENTORY_SLOT_GAP);
        let min_x = INVENTORY_START_X + col as f32 * step_x;
        let min_y = INVENTORY_START_Y + row as f32 * (INVENTORY_SLOT_SIZE + INVENTORY_SLOT_GAP);
        let max_x = min_x + ui_width(INVENTORY_SLOT_SIZE);
        let max_y = min_y + INVENTORY_SLOT_SIZE;
        Some(((min_x, min_y), (max_x, max_y)))
    }

    fn inventory_slot_from_point(&self, point: (f32, f32)) -> Option<usize> {
        for index in 0..INVENTORY_SLOT_COUNT {
            if let Some((min, max)) = self.inventory_slot_rect(index) {
                if point.0 >= min.0 && point.0 <= max.0 && point.1 >= min.1 && point.1 <= max.1 {
                    return Some(index);
                }
            }
        }
        None
    }

    fn inventory_layout(&self) -> InventoryLayout {
        let panel_min = (ui_width(0.12), 0.1);
        let panel_max = (1.0 - ui_width(0.12), 0.9);
        let header_min = (panel_min.0 + ui_width(0.032), panel_min.1 + 0.032);
        let header_max = (panel_max.0 - ui_width(0.032), header_min.1 + 0.082);

        let mut grid_panel_min = (panel_min.0 + ui_width(0.04), header_max.1 + 0.05);
        let mut grid_panel_max = (panel_min.0 + ui_width(0.42), header_max.1 + 0.46);

        if let (Some((slot_min, _)), Some((_, slot_max))) = (
            self.inventory_slot_rect(0),
            self.inventory_slot_rect(HOTBAR_SIZE - 1),
        ) {
            let margin_x = ui_width(0.035);
            let margin_top = 0.045;
            let margin_bottom = 0.065;
            grid_panel_min = (
                (slot_min.0 - margin_x).max(panel_min.0 + ui_width(0.028)),
                (slot_min.1 - margin_top).max(header_max.1 + 0.028),
            );
            grid_panel_max = (
                (slot_max.0 + margin_x).min(panel_min.0 + ui_width(0.45)),
                (slot_max.1 + margin_bottom).min(panel_max.1 - 0.24),
            );
        }

        let palette_panel_min = (grid_panel_max.0 + ui_width(0.045), grid_panel_min.1);
        let palette_panel_max = (panel_max.0 - ui_width(0.02), panel_max.1 - 0.24);

        let instructions_panel_min = (panel_min.0 + ui_width(0.04), panel_max.1 - 0.16);
        let instructions_panel_max = (panel_max.0 - ui_width(0.04), panel_max.1 - 0.04);

        let search_min = (
            palette_panel_min.0 + ui_width(FILTER_AREA_PADDING_X),
            palette_panel_min.1 + FILTER_AREA_PADDING_Y,
        );
        let search_max = (
            palette_panel_max.0 - ui_width(FILTER_AREA_PADDING_X),
            (search_min.1 + SEARCH_FIELD_HEIGHT).min(palette_panel_max.1 - FILTER_AREA_PADDING_Y),
        );

        let search_clear_width = ui_width(SEARCH_FIELD_HEIGHT * 0.62);
        let search_clear_rect = (
            (
                search_max.0 - search_clear_width - ui_width(SEARCH_FIELD_PADDING * 0.5),
                search_min.1 + SEARCH_FIELD_PADDING * 0.25,
            ),
            (
                search_max.0 - ui_width(SEARCH_FIELD_PADDING * 0.25),
                search_max.1 - SEARCH_FIELD_PADDING * 0.25,
            ),
        );

        let chip_start_x = palette_panel_min.0 + ui_width(FILTER_AREA_PADDING_X);
        let chip_available_width =
            palette_panel_max.0 - ui_width(FILTER_AREA_PADDING_X) - chip_start_x;
        let chip_height = FILTER_CHIP_HEIGHT;
        let mut chip_rects = Vec::with_capacity(PALETTE_CATEGORIES.len());
        let mut chip_cursor_x = chip_start_x;
        let mut chip_cursor_y = search_max.1 + FILTER_AREA_PADDING_Y;
        for category in PALETTE_CATEGORIES.iter() {
            let label_len = category.name.len() as f32;
            let chip_width = (ui_width(0.055) + label_len * ui_width(0.008))
                .min(chip_available_width.max(ui_width(0.08)));
            if chip_cursor_x + chip_width > palette_panel_max.0 - ui_width(FILTER_AREA_PADDING_X) {
                chip_cursor_x = chip_start_x;
                chip_cursor_y += chip_height + FILTER_CHIP_GAP;
            }
            let rect = (
                (chip_cursor_x, chip_cursor_y),
                (chip_cursor_x + chip_width, chip_cursor_y + chip_height),
            );
            chip_rects.push(rect);
            chip_cursor_x = chip_cursor_x + chip_width + ui_width(FILTER_CHIP_GAP);
        }
        let chips_bottom = chip_rects
            .last()
            .map(|(_, max)| max.1)
            .unwrap_or(search_max.1);

        let palette_content_origin = (
            palette_panel_min.0 + ui_width(FILTER_AREA_PADDING_X),
            chips_bottom + FILTER_AREA_PADDING_Y,
        );
        let palette_view_height =
            (palette_panel_max.1 - FILTER_AREA_PADDING_Y) - palette_content_origin.1;

        InventoryLayout {
            panel: (panel_min, panel_max),
            header: (header_min, header_max),
            hotbar_panel: (grid_panel_min, grid_panel_max),
            palette_panel: (palette_panel_min, palette_panel_max),
            instructions_panel: (instructions_panel_min, instructions_panel_max),
            search_rect: (search_min, search_max),
            search_clear_rect,
            chip_rects,
            palette_content_origin,
            palette_view_height: palette_view_height.max(0.0),
        }
    }

    fn palette_slot_rect(&self, layout: &InventoryLayout, index: usize) -> Option<Rect> {
        if index >= self.inventory_palette_filtered.len() {
            return None;
        }
        let base_origin = layout.palette_content_origin;
        let col = index % PALETTE_COLS;
        let row = index / PALETTE_COLS;
        let step_x = ui_width(PALETTE_SLOT_SIZE + PALETTE_SLOT_GAP);
        let step_y = PALETTE_SLOT_SIZE + PALETTE_SLOT_GAP;
        let min_x = base_origin.0 + col as f32 * step_x;
        let min_y = base_origin.1 + row as f32 * step_y - self.inventory_palette_scroll;
        let max_x = min_x + ui_width(PALETTE_SLOT_SIZE);
        let max_y = min_y + PALETTE_SLOT_SIZE;
        Some(((min_x, min_y), (max_x, max_y)))
    }

    fn palette_index_from_point(
        &self,
        layout: &InventoryLayout,
        point: (f32, f32),
    ) -> Option<usize> {
        for index in 0..self.inventory_palette_filtered.len() {
            if let Some((min, max)) = self.palette_slot_rect(layout, index) {
                if point.0 >= min.0 && point.0 <= max.0 && point.1 >= min.1 && point.1 <= max.1 {
                    return Some(index);
                }
            }
        }
        None
    }

    fn refresh_palette_filter(&mut self) {
        let mut blocks: Vec<BlockType> =
            if let Some(category) = PALETTE_CATEGORIES.get(self.inventory_active_category) {
                category.blocks.to_vec()
            } else {
                AVAILABLE_BLOCKS.to_vec()
            };

        blocks.sort_by_key(|block| {
            AVAILABLE_BLOCKS
                .iter()
                .position(|candidate| candidate == block)
                .unwrap_or(usize::MAX)
        });
        blocks.dedup();

        if !self.inventory_search_query.is_empty() {
            let needle = self.inventory_search_query.to_ascii_lowercase();
            blocks.retain(|block| block.name().to_ascii_lowercase().contains(&needle));
        }

        self.inventory_palette_filtered = blocks;
        self.inventory_palette_hover = None;
        self.inventory_last_hover_palette = None;
        self.inventory_filter_chip_hover = None;

        let layout = self.inventory_layout();
        let max_scroll = self.max_palette_scroll(&layout);
        if self.inventory_palette_filtered.is_empty() {
            self.inventory_palette_scroll = 0.0;
        } else {
            self.inventory_palette_scroll = self.inventory_palette_scroll.clamp(0.0, max_scroll);
        }
        self.mark_ui_dirty();
    }

    fn max_palette_scroll(&self, layout: &InventoryLayout) -> f32 {
        if self.inventory_palette_filtered.is_empty() {
            return 0.0;
        }
        let rows = (self.inventory_palette_filtered.len() + PALETTE_COLS - 1) / PALETTE_COLS;
        let step_y = PALETTE_SLOT_SIZE + PALETTE_SLOT_GAP;
        let total_height = rows as f32 * step_y - PALETTE_SLOT_GAP;
        (total_height - layout.palette_view_height).max(0.0)
    }

    fn ensure_palette_scroll_bounds(&mut self, layout: &InventoryLayout) {
        let max_scroll = self.max_palette_scroll(layout);
        self.inventory_palette_scroll = self.inventory_palette_scroll.clamp(0.0, max_scroll);
    }

    fn cancel_inventory_drag(&mut self) {
        if let Some(block) = self.inventory_drag_block.take() {
            if let Some(origin) = self.inventory_drag_origin.take() {
                self.inventory.set_slot(origin, Some(block));
                self.inventory_cursor = origin;
                self.inventory.select_slot(origin);
                self.print_selected();
            }
            self.mark_ui_dirty();
        } else {
            self.inventory_drag_origin = None;
        }
    }

    fn move_inventory_cursor(&mut self, dx: i32, dy: i32) {
        let cols = INVENTORY_COLS as i32;
        let rows = INVENTORY_ROWS as i32;
        let mut col = (self.inventory_cursor % INVENTORY_COLS) as i32;
        let mut row = (self.inventory_cursor / INVENTORY_COLS) as i32;
        col = (col + dx).rem_euclid(cols);
        row = (row + dy).rem_euclid(rows);
        let new_index = (row * cols + col) as usize;
        self.inventory_cursor = new_index.min(HOTBAR_SIZE - 1);
        self.inventory.select_slot(self.inventory_cursor);
        self.print_selected();
        self.mark_ui_dirty();
    }

    fn handle_inventory_input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let size = self.window.inner_size();
                if size.width == 0 || size.height == 0 {
                    return false;
                }
                let norm_x = (position.x as f32 / size.width as f32).clamp(0.0, 1.0);
                let norm_y = (position.y as f32 / size.height as f32).clamp(0.0, 1.0);
                let ui_point = self.ui_scaler.unproject((norm_x, norm_y));
                self.inventory_cursor_pos = Some(ui_point);

                let layout = self.inventory_layout();

                let slot_hover = self.inventory_slot_from_point(ui_point);
                if slot_hover != self.inventory_hover_slot {
                    self.inventory_hover_slot = slot_hover;
                    if let Some(slot) = slot_hover {
                        let description = self.inventory.hotbar[slot]
                            .map(|block| block.name())
                            .unwrap_or("Empty");
                        if self.inventory_last_hover_slot != Some(slot) {
                            println!("Hovering hotbar slot {} ({})", slot + 1, description);
                        }
                        self.inventory_last_hover_slot = Some(slot);
                    } else {
                        self.inventory_last_hover_slot = None;
                    }
                    self.mark_ui_dirty();
                }

                let palette_hover = self.palette_index_from_point(&layout, ui_point);
                if palette_hover != self.inventory_palette_hover {
                    self.inventory_palette_hover = palette_hover;
                    if let Some(index) = palette_hover {
                        if self.inventory_last_hover_palette != Some(index) {
                            if let Some(block) = self.inventory_palette_filtered.get(index) {
                                println!("Palette block: {}", block.name());
                            }
                        }
                        self.inventory_last_hover_palette = Some(index);
                    } else {
                        self.inventory_last_hover_palette = None;
                    }
                    self.mark_ui_dirty();
                }

                let chip_hover = layout.chip_rects.iter().position(|rect| {
                    ui_point.0 >= (rect.0).0
                        && ui_point.0 <= (rect.1).0
                        && ui_point.1 >= (rect.0).1
                        && ui_point.1 <= (rect.1).1
                });
                if chip_hover != self.inventory_filter_chip_hover {
                    self.inventory_filter_chip_hover = chip_hover;
                    self.mark_ui_dirty();
                }

                false
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let mut direction = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y.round() as i32,
                    MouseScrollDelta::PixelDelta(pos) => pos.y.signum() as i32,
                };
                direction = direction.clamp(-1, 1);
                if direction == 0 {
                    return false;
                }

                if let Some(cursor) = self.inventory_cursor_pos {
                    let layout = self.inventory_layout();
                    if cursor.0 >= (layout.palette_panel.0).0
                        && cursor.0 <= (layout.palette_panel.1).0
                        && cursor.1 >= (layout.palette_panel.0).1
                        && cursor.1 <= (layout.palette_panel.1).1
                    {
                        let delta_normalized =
                            (PALETTE_SLOT_SIZE + PALETTE_SLOT_GAP) * direction as f32 * -0.9;
                        self.inventory_palette_scroll += delta_normalized;
                        self.ensure_palette_scroll_bounds(&layout);
                        let new_hover = self.palette_index_from_point(&layout, cursor);
                        if new_hover != self.inventory_palette_hover {
                            self.inventory_palette_hover = new_hover;
                        }
                        self.mark_ui_dirty();
                        return true;
                    }
                }

                let direction = -direction;
                let slot = self
                    .inventory_hover_slot
                    .unwrap_or(self.inventory_cursor)
                    .min(HOTBAR_SIZE - 1);
                self.inventory_cursor = slot;
                self.inventory.select_slot(slot);
                self.inventory.cycle_slot_block(slot, direction);
                let description = self.inventory.hotbar[slot]
                    .map(|block| block.name())
                    .unwrap_or("Empty");
                println!("Slot {} set to {}.", slot + 1, description);
                self.print_selected();
                self.mark_ui_dirty();
                true
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let layout = self.inventory_layout();
                let cursor = self.inventory_cursor_pos;
                let point_in_rect = |pt: (f32, f32), rect: Rect| {
                    pt.0 >= (rect.0).0
                        && pt.0 <= (rect.1).0
                        && pt.1 >= (rect.0).1
                        && pt.1 <= (rect.1).1
                };

                match (state, button) {
                    (ElementState::Pressed, MouseButton::Left) => {
                        let shift = self.modifiers.state().shift_key();
                        if let Some(point) = cursor {
                            if point_in_rect(point, layout.search_clear_rect)
                                && !self.inventory_search_query.is_empty()
                            {
                                self.inventory_search_query.clear();
                                self.inventory_search_active = true;
                                self.inventory_palette_scroll = 0.0;
                                self.refresh_palette_filter();
                                return true;
                            }

                            if point_in_rect(point, layout.search_rect) {
                                self.inventory_search_active = true;
                                self.mark_ui_dirty();
                                return true;
                            } else {
                                self.inventory_search_active = false;
                            }

                            if let Some(chip_index) = layout
                                .chip_rects
                                .iter()
                                .position(|rect| point_in_rect(point, *rect))
                            {
                                // Toggle category if clicking the active one, otherwise switch to new category
                                let new_category = if chip_index == self.inventory_active_category
                                    && chip_index != 0
                                {
                                    0
                                } else {
                                    chip_index
                                };

                                // Only reset scroll if changing category
                                if new_category != self.inventory_active_category {
                                    self.inventory_palette_scroll = 0.0;
                                }

                                self.inventory_active_category = new_category;
                                self.refresh_palette_filter();
                                return true;
                            }
                        }

                        if shift {
                            if let Some(index) = self.inventory_palette_hover {
                                if let Some(block) =
                                    self.inventory_palette_filtered.get(index).copied()
                                {
                                    let target_slot = self
                                        .inventory
                                        .first_empty_slot()
                                        .unwrap_or(self.inventory_cursor)
                                        .min(HOTBAR_SIZE - 1);
                                    self.inventory.set_slot(target_slot, Some(block));
                                    self.inventory_cursor = target_slot;
                                    self.inventory.select_slot(target_slot);
                                    self.print_selected();
                                    println!(
                                        "Quick-slotted {} to {}.",
                                        block.name(),
                                        target_slot + 1
                                    );
                                    self.mark_ui_dirty();
                                    return true;
                                }
                            }

                            if let Some(slot) = self.inventory_hover_slot {
                                if slot != self.inventory_cursor {
                                    self.inventory.swap_slots(self.inventory_cursor, slot);
                                    println!(
                                        "Swapped hotbar slots {} and {}.",
                                        self.inventory_cursor + 1,
                                        slot + 1
                                    );
                                    self.inventory_cursor = slot;
                                    self.inventory.select_slot(slot);
                                    self.print_selected();
                                    self.mark_ui_dirty();
                                    return true;
                                }
                            }
                        }

                        if self.inventory_drag_block.is_some() {
                            return true;
                        }

                        if let Some(origin) = self.inventory_swap_slot {
                            if let Some(target) = self.inventory_hover_slot {
                                if origin == target {
                                    println!("Swap cancelled.");
                                } else {
                                    self.inventory.swap_slots(origin, target);
                                    println!(
                                        "Swapped hotbar slots {} and {}.",
                                        origin + 1,
                                        target + 1
                                    );
                                    self.inventory_cursor = target;
                                    self.inventory.select_slot(target);
                                    self.print_selected();
                                }
                                self.inventory_swap_slot = None;
                                self.mark_ui_dirty();
                                return true;
                            }
                        }

                        if let Some(index) = self.inventory_palette_hover {
                            if let Some(block) = self.inventory_palette_filtered.get(index).copied()
                            {
                                let slot = self
                                    .inventory_hover_slot
                                    .unwrap_or(self.inventory_cursor)
                                    .min(HOTBAR_SIZE - 1);
                                self.inventory.set_slot(slot, Some(block));
                                println!("Slot {} set to {}.", slot + 1, block.name());
                                self.inventory_cursor = slot;
                                self.inventory.select_slot(slot);
                                self.print_selected();
                                self.mark_ui_dirty();
                                return true;
                            }
                        }

                        if let Some(slot) = self.inventory_hover_slot {
                            self.inventory_cursor = slot;
                            self.inventory.select_slot(slot);
                            self.print_selected();
                            if let Some(block) = self.inventory.hotbar[slot] {
                                self.inventory_drag_origin = Some(slot);
                                self.inventory_drag_block = Some(block);
                                self.inventory.set_slot(slot, None);
                                println!("Picked up {} from slot {}.", block.name(), slot + 1);
                            }
                            self.inventory_swap_slot = None;
                            self.mark_ui_dirty();
                            return true;
                        }

                        false
                    }
                    (ElementState::Released, MouseButton::Left) => {
                        if let Some(block) = self.inventory_drag_block.take() {
                            let origin = self.inventory_drag_origin.take();
                            if let Some(slot) = self.inventory_hover_slot {
                                let previous = self.inventory.hotbar[slot];
                                self.inventory.set_slot(slot, Some(block));
                                if let Some(origin_slot) = origin {
                                    if origin_slot != slot {
                                        self.inventory.set_slot(origin_slot, previous);
                                    }
                                }
                                self.inventory_cursor = slot;
                                self.inventory.select_slot(slot);
                                println!("Placed {} in slot {}.", block.name(), slot + 1);
                                self.print_selected();
                            } else if let Some(index) = self.inventory_palette_hover {
                                if let Some(new_block) =
                                    self.inventory_palette_filtered.get(index).copied()
                                {
                                    let target_slot = origin
                                        .unwrap_or(self.inventory_cursor)
                                        .min(HOTBAR_SIZE - 1);
                                    self.inventory.set_slot(target_slot, Some(new_block));
                                    self.inventory_cursor = target_slot;
                                    self.inventory.select_slot(target_slot);
                                    println!(
                                        "Replaced slot {} with {} (was {}).",
                                        target_slot + 1,
                                        new_block.name(),
                                        block.name()
                                    );
                                    self.print_selected();
                                }
                            } else if let Some(origin_slot) = origin {
                                self.inventory.set_slot(origin_slot, Some(block));
                                self.inventory_cursor = origin_slot;
                                self.inventory.select_slot(origin_slot);
                                self.print_selected();
                            } else {
                                let slot = self.inventory_cursor.min(HOTBAR_SIZE - 1);
                                self.inventory.set_slot(slot, Some(block));
                                println!("Slot {} set to {}.", slot + 1, block.name());
                                self.inventory.select_slot(slot);
                                self.print_selected();
                            }
                            self.mark_ui_dirty();
                            return true;
                        }
                        false
                    }
                    (ElementState::Pressed, MouseButton::Right) => {
                        if self.inventory_drag_block.is_some() {
                            self.cancel_inventory_drag();
                            println!("Drag cancelled.");
                            return true;
                        }

                        if let Some(slot) = self.inventory_hover_slot {
                            self.inventory.clear_slot(slot);
                            println!("Cleared hotbar slot {}.", slot + 1);
                            if self.inventory_cursor == slot {
                                self.print_selected();
                            }
                            self.mark_ui_dirty();
                            return true;
                        }

                        if let Some(index) = self.inventory_palette_hover {
                            if let Some(block) = self.inventory_palette_filtered.get(index).copied()
                            {
                                let slot =
                                    self.inventory_hover_slot.unwrap_or(self.inventory_cursor);
                                self.inventory.set_slot(slot, Some(block));
                                println!("Slot {} set to {}.", slot + 1, block.name());
                                self.inventory_cursor = slot;
                                self.inventory.select_slot(slot);
                                self.print_selected();
                                self.mark_ui_dirty();
                                return true;
                            }
                        }

                        false
                    }
                    _ => false,
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return false;
                }
                if let PhysicalKey::Code(key) = event.physical_key {
                    if self.inventory_search_active {
                        match key {
                            KeyCode::Backspace => {
                                if !self.inventory_search_query.is_empty() {
                                    self.inventory_search_query.pop();
                                    self.refresh_palette_filter();
                                }
                                return true;
                            }
                            KeyCode::Escape => {
                                self.inventory_search_active = false;
                                self.inventory_search_query.clear();
                                self.inventory_palette_scroll = 0.0;
                                self.refresh_palette_filter();
                                return true;
                            }
                            KeyCode::Enter => {
                                self.inventory_search_active = false;
                                self.mark_ui_dirty();
                                return true;
                            }
                            KeyCode::ArrowLeft
                            | KeyCode::ArrowRight
                            | KeyCode::ArrowUp
                            | KeyCode::ArrowDown => {}
                            _ => {
                                return false;
                            }
                        }
                    }

                    match key {
                        KeyCode::ArrowLeft => {
                            self.move_inventory_cursor(-1, 0);
                            return true;
                        }
                        KeyCode::ArrowRight => {
                            self.move_inventory_cursor(1, 0);
                            return true;
                        }
                        KeyCode::ArrowUp => {
                            self.move_inventory_cursor(0, -1);
                            return true;
                        }
                        KeyCode::ArrowDown => {
                            self.move_inventory_cursor(0, 1);
                            return true;
                        }
                        KeyCode::Enter | KeyCode::Space => {
                            if let Some(origin) = self.inventory_swap_slot {
                                if origin == self.inventory_cursor {
                                    println!("Swap cancelled.");
                                    self.inventory_swap_slot = None;
                                } else {
                                    let target = self.inventory_cursor;
                                    self.inventory.swap_slots(origin, target);
                                    println!(
                                        "Swapped hotbar slots {} and {}.",
                                        origin + 1,
                                        target + 1
                                    );
                                    self.inventory_swap_slot = None;
                                    self.print_selected();
                                }
                            } else {
                                self.inventory_swap_slot = Some(self.inventory_cursor);
                                println!(
                                    "Slot {} ready to swap. Select another slot.",
                                    self.inventory_cursor + 1
                                );
                            }
                            self.mark_ui_dirty();
                            return true;
                        }
                        KeyCode::KeyZ => {
                            self.inventory.cycle_slot_block(self.inventory_cursor, -1);
                            let description = self.inventory.hotbar[self.inventory_cursor]
                                .map(|block| block.name())
                                .unwrap_or("Empty");
                            println!("Slot {} set to {}.", self.inventory_cursor + 1, description);
                            self.inventory.select_slot(self.inventory_cursor);
                            self.print_selected();
                            self.mark_ui_dirty();
                            return true;
                        }
                        KeyCode::KeyX => {
                            self.inventory.cycle_slot_block(self.inventory_cursor, 1);
                            let description = self.inventory.hotbar[self.inventory_cursor]
                                .map(|block| block.name())
                                .unwrap_or("Empty");
                            println!("Slot {} set to {}.", self.inventory_cursor + 1, description);
                            self.inventory.select_slot(self.inventory_cursor);
                            self.print_selected();
                            self.mark_ui_dirty();
                            return true;
                        }
                        KeyCode::Backspace | KeyCode::Delete => {
                            self.inventory.clear_slot(self.inventory_cursor);
                            println!("Cleared hotbar slot {}.", self.inventory_cursor + 1);
                            self.print_selected();
                            self.mark_ui_dirty();
                            return true;
                        }
                        KeyCode::Digit1
                        | KeyCode::Digit2
                        | KeyCode::Digit3
                        | KeyCode::Digit4
                        | KeyCode::Digit5
                        | KeyCode::Digit6
                        | KeyCode::Digit7
                        | KeyCode::Digit8
                        | KeyCode::Digit9 => {
                            let slot_index = match key {
                                KeyCode::Digit1 => 0,
                                KeyCode::Digit2 => 1,
                                KeyCode::Digit3 => 2,
                                KeyCode::Digit4 => 3,
                                KeyCode::Digit5 => 4,
                                KeyCode::Digit6 => 5,
                                KeyCode::Digit7 => 6,
                                KeyCode::Digit8 => 7,
                                KeyCode::Digit9 => 8,
                                _ => 0,
                            };
                            if slot_index < HOTBAR_SIZE {
                                self.inventory_cursor = slot_index;
                                self.inventory.select_slot(slot_index);
                                self.print_selected();
                                self.mark_ui_dirty();
                                return true;
                            }
                        }
                        _ => {}
                    }
                }
                false
            }
            _ => false,
        }
    }
    fn draw_hotbar(&self, ui: &mut UiGeometry) {
        let slot_count = self.inventory.hotbar.len();
        if slot_count == 0 {
            return;
        }

        let theme = self.hotbar_theme();

        let slot_height = 0.072;
        let slot_width = ui_width(slot_height);
        let slot_gap = ui_width(0.012);
        let panel_pad_x = ui_width(0.028);
        let panel_pad_y = 0.018;

        let total_width =
            slot_count as f32 * slot_width + (slot_count.saturating_sub(1) as f32) * slot_gap;

        let bar_bottom = 0.97;
        let bar_top = (bar_bottom - (slot_height + panel_pad_y * 2.0)).max(0.82);
        let bar_left = (0.5 - total_width * 0.5 - panel_pad_x).max(ui_width(0.04));
        let bar_right = (0.5 + total_width * 0.5 + panel_pad_x).min(1.0 - ui_width(0.04));

        let shadow_offset = ui_width(0.012);
        ui.add_rect(
            (bar_left + shadow_offset, bar_top + 0.018),
            (bar_right + shadow_offset, bar_bottom + 0.018),
            [0.0, 0.0, 0.0, 0.35],
        );

        ui.add_panel(
            (bar_left, bar_top),
            (bar_right, bar_bottom),
            theme.panel_border,
            theme.panel_fill,
            Some(theme.panel_highlight),
        );

        let title_pos = (bar_left, (bar_top - 0.03).max(0.06));
        ui.add_text(title_pos, 0.016, [0.86, 0.9, 1.0, 0.95], "QUICK BAR");

        let slot_start_x = 0.5 - total_width * 0.5;
        let slot_top = bar_top + panel_pad_y;
        let slot_bottom = bar_bottom - panel_pad_y;
        let selected_slot = self.inventory.selected_slot_index();

        for (index, slot) in self.inventory.hotbar.iter().enumerate() {
            let x = slot_start_x + index as f32 * (slot_width + slot_gap);
            let slot_min = (x, slot_top);
            let slot_max = (x + slot_width, slot_bottom);

            let mut slot_fill = if index == selected_slot {
                theme.slot_selected
            } else {
                theme.slot_default
            };

            if self.inventory_open {
                if self.inventory_drag_origin == Some(index) && self.inventory_drag_block.is_some()
                {
                    slot_fill = [0.56, 0.34, 0.34, 0.92];
                } else if self.inventory_cursor == index {
                    slot_fill = [0.32, 0.42, 0.6, 0.94];
                }
            }

            ui.add_panel(
                slot_min,
                slot_max,
                [0.08, 0.09, 0.13, 0.96],
                slot_fill,
                None,
            );

            if index == selected_slot {
                let indicator_height = 0.007;
                ui.add_rect(
                    (slot_min.0, slot_max.1 - indicator_height),
                    (slot_max.0, slot_max.1),
                    [0.38, 0.62, 0.92, 0.9],
                );
            }

            let icon_pad_y = 0.0075;
            let icon_pad_x = ui_width(icon_pad_y);
            let icon_min = (slot_min.0 + icon_pad_x, slot_min.1 + icon_pad_y);
            let icon_max = (slot_max.0 - icon_pad_x, slot_max.1 - icon_pad_y);

            if let Some(block) = slot {
                let tint = if index == selected_slot {
                    [1.0, 0.96, 0.86, 1.0]
                } else if self.inventory_cursor == index {
                    [1.0, 0.98, 0.92, 1.0]
                } else {
                    [1.0, 1.0, 1.0, 1.0]
                };
                ui.add_rect_textured(icon_min, icon_max, block.atlas_coords(BlockFace::Top), tint);
            } else {
                ui.add_rect(icon_min, icon_max, [0.08, 0.09, 0.12, 0.55]);
            }

            let label_pos = (slot_min.0 + ui_width(0.004), slot_max.1 - 0.014);
            ui.add_text(
                label_pos,
                0.011,
                [0.7, 0.76, 0.92, 1.0],
                &(index + 1).to_string(),
            );
        }

        if let Some(status) = &theme.status {
            let chip_height = 0.05;
            let chip_width = ui_width(0.21);
            let chip_min = (
                (bar_right - chip_width).max(bar_left),
                (bar_top - chip_height - 0.02).max(0.06),
            );
            let chip_max = (chip_min.0 + chip_width, chip_min.1 + chip_height);
            ui.add_panel(
                chip_min,
                chip_max,
                [0.08, 0.09, 0.14, 0.9],
                status.chip_fill,
                None,
            );
            ui.add_text(
                (chip_min.0 + ui_width(0.014), chip_min.1 + 0.016),
                0.014,
                status.chip_text,
                status.label,
            );
            if let Some(detail) = status.detail {
                ui.add_text(
                    (chip_min.0 + ui_width(0.014), chip_min.1 + 0.034),
                    0.011,
                    [0.78, 0.82, 0.96, 1.0],
                    detail,
                );
            }
        }

        ui.add_text(
            (bar_left, (bar_bottom + 0.014).min(0.985)),
            0.012,
            [0.7, 0.78, 0.92, 0.9],
            "Scroll or press 1-9 to switch items",
        );
    }
    fn draw_pause_overlay(&self, ui: &mut UiGeometry) {
        if self.settings_open {
            self.draw_settings_overlay(ui);
            return;
        }

        ui.add_rect_fullscreen((0.0, 0.0), (1.0, 1.0), [0.01, 0.02, 0.05, 0.68]);

        let panel_min = (ui_width(0.22), 0.24);
        let panel_max = (1.0 - ui_width(0.22), 0.78);
        let shadow_offset = ui_width(0.016);

        ui.add_rect(
            (panel_min.0 + shadow_offset, panel_min.1 + 0.02),
            (panel_max.0 + shadow_offset, panel_max.1 + 0.02),
            [0.0, 0.0, 0.0, 0.4],
        );

        ui.add_panel(
            panel_min,
            panel_max,
            [0.12, 0.14, 0.2, 0.98],
            [0.08, 0.09, 0.14, 0.94],
            Some([0.36, 0.54, 0.88, 0.3]),
        );

        let header_min = (panel_min.0 + ui_width(0.03), panel_min.1 + 0.034);
        let header_max = (panel_max.0 - ui_width(0.03), header_min.1 + 0.084);
        ui.add_rect(header_min, header_max, [0.18, 0.2, 0.28, 0.96]);
        ui.add_text(
            (header_min.0 + ui_width(0.012), header_min.1 + 0.02),
            0.03,
            [0.95, 0.98, 1.0, 1.0],
            "PAUSED",
        );
        ui.add_text(
            (header_min.0 + ui_width(0.012), header_max.1 + 0.016),
            0.014,
            [0.78, 0.83, 0.96, 1.0],
            "Take a breath, then dive back in.",
        );

        let menu_items = [
            ("RESUME", "Press ESC to return to the game"),
            ("SETTINGS", "Press S to adjust display, audio, and controls"),
            ("QUIT TO DESKTOP", "Press Alt+F4 to close the game"),
        ];

        let mut item_top = header_max.1 + 0.07;
        for (title, detail) in menu_items.iter() {
            let item_min = (panel_min.0 + ui_width(0.04), item_top - 0.015);
            let item_max = (panel_max.0 - ui_width(0.04), item_top + 0.085);
            ui.add_panel(
                item_min,
                item_max,
                [0.14, 0.16, 0.23, 0.92],
                [0.11, 0.13, 0.2, 0.9],
                Some([0.32, 0.5, 0.84, 0.34]),
            );
            ui.add_text(
                (item_min.0 + ui_width(0.02), item_top + 0.002),
                0.018,
                [0.93, 0.96, 1.0, 1.0],
                title,
            );
            ui.add_text(
                (item_min.0 + ui_width(0.02), item_top + 0.034),
                0.013,
                [0.76, 0.81, 0.94, 1.0],
                detail,
            );
            item_top += 0.11;
        }

        ui.add_text(
            (panel_min.0 + ui_width(0.04), panel_max.1 - 0.06),
            0.012,
            [0.72, 0.78, 0.92, 1.0],
            "ESC: resume | S: open settings | Click: return to cursor",
        );
    }
    fn draw_settings_overlay(&self, ui: &mut UiGeometry) {
        ui.add_rect_fullscreen((0.0, 0.0), (1.0, 1.0), [0.01, 0.02, 0.05, 0.72]);

        let panel_min = (ui_width(0.18), 0.16);
        let panel_max = (1.0 - ui_width(0.18), 0.84);
        let shadow_offset = ui_width(0.014);

        ui.add_rect(
            (panel_min.0 + shadow_offset, panel_min.1 + 0.02),
            (panel_max.0 + shadow_offset, panel_max.1 + 0.02),
            [0.0, 0.0, 0.0, 0.42],
        );

        ui.add_panel(
            panel_min,
            panel_max,
            [0.12, 0.14, 0.2, 0.98],
            [0.08, 0.09, 0.14, 0.95],
            Some([0.36, 0.54, 0.88, 0.34]),
        );

        let header_min = (panel_min.0 + ui_width(0.03), panel_min.1 + 0.032);
        let header_max = (panel_max.0 - ui_width(0.03), header_min.1 + 0.08);
        ui.add_rect(header_min, header_max, [0.18, 0.2, 0.28, 0.96]);
        ui.add_text(
            (header_min.0 + ui_width(0.012), header_min.1 + 0.018),
            0.028,
            [0.95, 0.98, 1.0, 1.0],
            "SETTINGS",
        );
        ui.add_text(
            (header_min.0 + ui_width(0.012), header_max.1 + 0.016),
            0.013,
            [0.78, 0.82, 0.94, 1.0],
            "Fine tune how the world feels and responds.",
        );

        let tabs_min = (panel_min.0 + ui_width(0.03), header_max.1 + 0.026);
        let tab_height = 0.05;
        let mut tab_cursor_x = tabs_min.0;
        for tab in SettingsTab::ALL.iter() {
            let label = tab.label();
            let tab_width = ui_width(0.09) + label.len() as f32 * ui_width(0.01);
            let tab_min = (tab_cursor_x, tabs_min.1);
            let tab_max = (tab_cursor_x + tab_width, tabs_min.1 + tab_height);
            let active = *tab == self.settings_selected_tab;
            let fill = if active {
                [0.32, 0.5, 0.84, 0.92]
            } else {
                [0.16, 0.19, 0.26, 0.9]
            };
            ui.add_panel(tab_min, tab_max, [0.1, 0.11, 0.17, 0.94], fill, None);
            ui.add_text(
                (tab_min.0 + ui_width(0.014), tab_min.1 + 0.016),
                0.014,
                if active {
                    [0.95, 0.98, 1.0, 1.0]
                } else {
                    [0.78, 0.82, 0.94, 1.0]
                },
                label,
            );
            tab_cursor_x += tab_width + ui_width(0.018);
        }

        let content_min = (
            panel_min.0 + ui_width(0.04),
            tabs_min.1 + tab_height + 0.026,
        );
        let content_max = (panel_max.0 - ui_width(0.04), panel_max.1 - 0.12);
        let slider_width = ui_width(0.32);
        let slider_height = 0.012;

        let mut cursor_y = content_min.1;
        match self.settings_selected_tab {
            SettingsTab::Display => {
                let mut entries = Vec::new();
                let fov_ratio = ((self.settings_fov_deg - 60.0) / 40.0).clamp(0.0, 1.0);
                entries.push((
                    "FIELD OF VIEW".to_string(),
                    format!("{:.0} DEG", self.settings_fov_deg),
                    fov_ratio,
                    0usize,
                ));
                let sens_ratio =
                    ((self.settings_sensitivity - 0.0005) / (0.02 - 0.0005)).clamp(0.0, 1.0);
                entries.push((
                    "LOOK SENSITIVITY".to_string(),
                    format!("{:.3}", self.settings_sensitivity * 1000.0),
                    sens_ratio,
                    1usize,
                ));

                for (label, value, ratio, focus_index) in entries {
                    let focused = self.settings_focus_index == focus_index
                        && self.settings_selected_tab == SettingsTab::Display;
                    let label_color = if focused {
                        [0.95, 0.98, 1.0, 1.0]
                    } else {
                        [0.78, 0.82, 0.94, 1.0]
                    };
                    ui.add_text((content_min.0, cursor_y), 0.014, label_color, &label);
                    ui.add_text(
                        (content_max.0 - ui_width(0.09), cursor_y),
                        0.014,
                        [0.86, 0.9, 1.0, 1.0],
                        &value,
                    );
                    cursor_y += 0.024;

                    let track_min = (content_min.0, cursor_y);
                    let track_max = (content_min.0 + slider_width, cursor_y + slider_height);
                    ui.add_rect(track_min, track_max, [0.16, 0.18, 0.26, 0.9]);
                    let fill_max_x = track_min.0 + slider_width * ratio;
                    ui.add_rect(
                        track_min,
                        (fill_max_x, track_max.1),
                        [0.36, 0.54, 0.88, 0.95],
                    );
                    let handle_width = ui_width(0.01);
                    let handle_min_x = (fill_max_x - handle_width * 0.5)
                        .clamp(track_min.0, track_max.0 - handle_width);
                    ui.add_rect(
                        (handle_min_x, track_min.1 - 0.005),
                        (handle_min_x + handle_width, track_max.1 + 0.005),
                        if focused {
                            [0.95, 0.98, 1.0, 1.0]
                        } else {
                            [0.72, 0.78, 0.94, 1.0]
                        },
                    );
                    cursor_y += slider_height + 0.04;
                }
            }
            SettingsTab::Audio => {
                let focused = self.settings_focus_index == 0;
                ui.add_text(
                    (content_min.0, cursor_y),
                    0.014,
                    if focused {
                        [0.95, 0.98, 1.0, 1.0]
                    } else {
                        [0.78, 0.82, 0.94, 1.0]
                    },
                    "MASTER VOLUME",
                );
                ui.add_text(
                    (content_max.0 - ui_width(0.09), cursor_y),
                    0.014,
                    [0.86, 0.9, 1.0, 1.0],
                    &format!("{:.0}%", self.settings_volume * 100.0),
                );
                cursor_y += 0.024;
                let track_min = (content_min.0, cursor_y);
                let track_max = (content_min.0 + slider_width, cursor_y + slider_height);
                let ratio = self.settings_volume.clamp(0.0, 1.0);
                ui.add_rect(track_min, track_max, [0.16, 0.18, 0.26, 0.9]);
                let fill_max_x = track_min.0 + slider_width * ratio;
                ui.add_rect(
                    track_min,
                    (fill_max_x, track_max.1),
                    [0.28, 0.62, 0.82, 0.95],
                );
                let handle_width = ui_width(0.01);
                let handle_min_x = (fill_max_x - handle_width * 0.5)
                    .clamp(track_min.0, track_max.0 - handle_width);
                ui.add_rect(
                    (handle_min_x, track_min.1 - 0.005),
                    (handle_min_x + handle_width, track_max.1 + 0.005),
                    if focused {
                        [0.95, 0.98, 1.0, 1.0]
                    } else {
                        [0.72, 0.78, 0.94, 1.0]
                    },
                );
                cursor_y += slider_height + 0.04;
                ui.add_text(
                    (content_min.0, cursor_y),
                    0.012,
                    [0.74, 0.79, 0.94, 1.0],
                    "Volume slider is placeholder until audio mix is implemented.",
                );
            }
            SettingsTab::Controls => {
                ui.add_text(
                    (content_min.0, cursor_y),
                    0.014,
                    [0.9, 0.93, 1.0, 1.0],
                    "Control remapping is coming soon.",
                );
                cursor_y += 0.028;
                ui.add_text(
                    (content_min.0, cursor_y),
                    0.012,
                    [0.74, 0.79, 0.94, 1.0],
                    "Use T on highlighted components to tweak electrical settings.",
                );
            }
        }

        ui.add_text(
            (panel_min.0 + ui_width(0.04), panel_max.1 - 0.075),
            0.012,
            [0.72, 0.78, 0.92, 1.0],
            "TAB: cycle categories | arrows: adjust | ESC: close",
        );
    }
    fn draw_inventory_overlay(&self, ui: &mut UiGeometry) {
        let layout = self.inventory_layout();
        let (panel_min, panel_max) = layout.panel;
        let (header_min, header_max) = layout.header;
        let (hotbar_panel_min, hotbar_panel_max) = layout.hotbar_panel;
        let (palette_panel_min, palette_panel_max) = layout.palette_panel;
        let (instructions_panel_min, instructions_panel_max) = layout.instructions_panel;
        let (search_min, search_max) = layout.search_rect;
        let (search_clear_min, search_clear_max) = layout.search_clear_rect;

        let point_in_rect = |pt: (f32, f32), rect: Rect| {
            pt.0 >= (rect.0).0 && pt.0 <= (rect.1).0 && pt.1 >= (rect.0).1 && pt.1 <= (rect.1).1
        };

        ui.add_rect_fullscreen((0.0, 0.0), (1.0, 1.0), [0.01, 0.02, 0.05, 0.6]);

        let shadow_offset = ui_width(0.014);
        ui.add_rect(
            (panel_min.0 + shadow_offset, panel_min.1 + 0.02),
            (panel_max.0 + shadow_offset, panel_max.1 + 0.02),
            [0.0, 0.0, 0.0, 0.4],
        );

        ui.add_panel(
            panel_min,
            panel_max,
            [0.12, 0.14, 0.2, 0.98],
            [0.08, 0.09, 0.14, 0.95],
            Some([0.36, 0.54, 0.88, 0.32]),
        );

        ui.add_rect(header_min, header_max, [0.18, 0.2, 0.28, 0.96]);
        ui.add_text(
            (header_min.0 + ui_width(0.014), header_min.1 + 0.018),
            0.028,
            [0.95, 0.98, 1.0, 1.0],
            "INVENTORY",
        );
        ui.add_text(
            (header_min.0 + ui_width(0.014), header_max.1 + 0.016),
            0.013,
            [0.78, 0.82, 0.94, 1.0],
            "Arrange your hotbar, filter blocks, and queue favourites.",
        );

        // Hotbar panel
        ui.add_panel(
            hotbar_panel_min,
            hotbar_panel_max,
            [0.14, 0.16, 0.22, 0.92],
            [0.11, 0.12, 0.18, 0.92],
            Some([0.24, 0.38, 0.62, 0.34]),
        );
        ui.add_text(
            (
                hotbar_panel_min.0 + ui_width(0.02),
                hotbar_panel_min.1 + 0.02,
            ),
            0.016,
            [0.9, 0.93, 1.0, 1.0],
            "HOTBAR",
        );
        ui.add_text(
            (
                hotbar_panel_min.0 + ui_width(0.02),
                hotbar_panel_min.1 + 0.048,
            ),
            0.012,
            [0.74, 0.79, 0.94, 1.0],
            "Drag to reorder, hover to preview, scroll to cycle.",
        );

        let selected_slot = self.inventory.selected_slot_index();
        for idx in 0..HOTBAR_SIZE {
            if let Some((min, max)) = self.inventory_slot_rect(idx) {
                let mut slot_fill = [0.18, 0.2, 0.28, 0.82];
                if Some(idx) == self.inventory_hover_slot {
                    slot_fill = [0.3, 0.34, 0.46, 0.9];
                }
                if self.inventory_drag_block.is_some()
                    && self.inventory_drag_origin != Some(idx)
                    && self.inventory_hover_slot == Some(idx)
                {
                    slot_fill = [0.56, 0.42, 0.32, 0.92];
                } else if self.inventory_drag_origin == Some(idx)
                    && self.inventory_drag_block.is_some()
                {
                    slot_fill = [0.56, 0.34, 0.34, 0.9];
                } else if Some(idx) == self.inventory_swap_slot {
                    slot_fill = [0.9, 0.56, 0.32, 0.88];
                } else if idx == selected_slot {
                    slot_fill = [0.34, 0.42, 0.6, 0.94];
                }
                if idx == self.inventory_cursor {
                    slot_fill = [0.4, 0.46, 0.65, 0.94];
                }

                ui.add_panel(
                    min,
                    max,
                    [0.11, 0.12, 0.18, 0.92],
                    slot_fill,
                    Some([0.32, 0.5, 0.78, 0.34]),
                );

                let icon_pad_y = INVENTORY_ICON_PAD;
                let icon_pad_x = ui_width(INVENTORY_ICON_PAD);
                let icon_min = (min.0 + icon_pad_x, min.1 + icon_pad_y);
                let icon_max = (max.0 - icon_pad_x, max.1 - icon_pad_y);

                if let Some(block) = self.inventory.hotbar[idx] {
                    ui.add_rect_textured(
                        icon_min,
                        icon_max,
                        block.atlas_coords(BlockFace::Top),
                        [1.0, 1.0, 1.0, 1.0],
                    );
                } else {
                    ui.add_rect(icon_min, icon_max, [0.08, 0.09, 0.12, 0.5]);
                }

                ui.add_text(
                    (min.0 + ui_width(0.012), max.1 - 0.02),
                    0.012,
                    [0.72, 0.76, 0.95, 1.0],
                    &format!("{}", idx + 1),
                );
            }
        }

        // Palette
        ui.add_panel(
            palette_panel_min,
            palette_panel_max,
            [0.14, 0.16, 0.22, 0.92],
            [0.11, 0.12, 0.18, 0.92],
            Some([0.24, 0.38, 0.62, 0.34]),
        );

        ui.add_text(
            (
                palette_panel_min.0 + ui_width(0.02),
                palette_panel_min.1 + 0.018,
            ),
            0.016,
            [0.9, 0.93, 1.0, 1.0],
            "BLOCK PALETTE",
        );
        ui.add_text(
            (
                palette_panel_min.0 + ui_width(0.02),
                palette_panel_min.1 + 0.046,
            ),
            0.012,
            [0.74, 0.79, 0.94, 1.0],
            "Click or drag to assign, shift-click to quick slot.",
        );

        // Search field
        let search_hover = self
            .inventory_cursor_pos
            .map(|pt| point_in_rect(pt, layout.search_rect))
            .unwrap_or(false);
        let search_clear_hover = self
            .inventory_cursor_pos
            .map(|pt| point_in_rect(pt, layout.search_clear_rect))
            .unwrap_or(false);
        let mut search_fill = [0.17, 0.19, 0.25, 0.96];
        if self.inventory_search_active {
            search_fill = [0.26, 0.3, 0.42, 0.96];
        } else if search_hover {
            search_fill = [0.22, 0.24, 0.34, 0.94];
        }
        ui.add_panel(
            search_min,
            search_max,
            [0.12, 0.13, 0.19, 0.96],
            search_fill,
            None,
        );

        let query = if self.inventory_search_query.is_empty() {
            "Search blocks...".to_string()
        } else {
            self.inventory_search_query.to_ascii_uppercase()
        };
        let search_text_color = if self.inventory_search_query.is_empty() {
            [0.65, 0.7, 0.82, 1.0]
        } else {
            [0.9, 0.94, 1.0, 1.0]
        };
        ui.add_text(
            (
                search_min.0 + ui_width(SEARCH_FIELD_PADDING),
                search_min.1 + 0.012,
            ),
            0.015,
            search_text_color,
            &query,
        );

        let clear_color = if self.inventory_search_query.is_empty() {
            [0.52, 0.56, 0.72, 0.6]
        } else if search_clear_hover {
            [0.92, 0.88, 0.76, 0.95]
        } else {
            [0.82, 0.86, 0.98, 0.85]
        };
        ui.add_panel(
            search_clear_min,
            search_clear_max,
            [0.18, 0.2, 0.28, 0.0],
            clear_color,
            None,
        );
        ui.add_text(
            (
                (search_clear_min.0 + search_clear_max.0) * 0.5 - ui_width(0.005),
                search_clear_min.1 + 0.006,
            ),
            0.018,
            [0.18, 0.2, 0.28, 1.0],
            "×",
        );

        for (idx, rect) in layout.chip_rects.iter().enumerate() {
            let (min, max) = *rect;
            let mut fill = [0.18, 0.2, 0.28, 0.8];
            if idx == self.inventory_active_category {
                fill = [0.36, 0.46, 0.68, 0.92];
            } else if Some(idx) == self.inventory_filter_chip_hover {
                fill = [0.28, 0.32, 0.46, 0.88];
            }
            ui.add_panel(min, max, [0.12, 0.13, 0.19, 0.0], fill, None);
            ui.add_text(
                (min.0 + ui_width(0.012), min.1 + 0.008),
                0.013,
                [0.92, 0.95, 1.0, 1.0],
                PALETTE_CATEGORIES[idx].name,
            );
        }

        let palette_blocks = &self.inventory_palette_filtered;
        let palette_view_top = layout.palette_content_origin.1;
        let palette_view_bottom = palette_panel_max.1 - FILTER_AREA_PADDING_Y;

        if palette_blocks.is_empty() {
            ui.add_text(
                (
                    palette_panel_min.0 + ui_width(0.02),
                    palette_view_top + 0.03,
                ),
                0.014,
                [0.76, 0.8, 0.94, 1.0],
                "No blocks match your filters.",
            );
        }

        for (index, block) in palette_blocks.iter().enumerate() {
            if let Some((min, max)) = self.palette_slot_rect(&layout, index) {
                if max.1 < palette_view_top - 0.01 || min.1 > palette_view_bottom + 0.01 {
                    continue;
                }

                let mut color = [0.18, 0.2, 0.28, 0.82];
                if Some(index) == self.inventory_palette_hover {
                    color = [0.32, 0.35, 0.46, 0.9];
                }
                if self.inventory_drag_block.is_some()
                    && self.inventory_palette_hover == Some(index)
                {
                    color = [0.58, 0.4, 0.34, 0.92];
                }
                if self.inventory.hotbar[self.inventory_cursor] == Some(*block) {
                    color = [0.36, 0.44, 0.62, 0.9];
                }
                ui.add_panel(
                    min,
                    max,
                    [0.12, 0.13, 0.19, 0.92],
                    color,
                    Some([0.3, 0.45, 0.72, 0.32]),
                );

                let icon_pad = PALETTE_ICON_PAD;
                let icon_min = (min.0 + ui_width(icon_pad), min.1 + icon_pad);
                let icon_max = (max.0 - ui_width(icon_pad), max.1 - icon_pad);
                ui.add_rect_textured(
                    icon_min,
                    icon_max,
                    block.atlas_coords(BlockFace::Top),
                    [1.0, 1.0, 1.0, 1.0],
                );
            }
        }

        // Instructions footer
        ui.add_panel(
            instructions_panel_min,
            instructions_panel_max,
            [0.14, 0.16, 0.22, 0.92],
            [0.11, 0.12, 0.18, 0.92],
            Some([0.24, 0.38, 0.62, 0.32]),
        );
        ui.add_text(
            (
                instructions_panel_min.0 + ui_width(0.018),
                instructions_panel_min.1 + 0.018,
            ),
            0.012,
            [0.9, 0.93, 1.0, 1.0],
            "Left click: drag/place  |  Right click: clear slot  |  Shift+Click: quick assign",
        );
        ui.add_text(
            (
                instructions_panel_min.0 + ui_width(0.018),
                instructions_panel_min.1 + 0.042,
            ),
            0.012,
            [0.75, 0.8, 0.94, 1.0],
            "Scroll over palette to browse, type to search, Enter/Esc to exit search.",
        );

        if let (Some(block), Some(cursor)) = (self.inventory_drag_block, self.inventory_cursor_pos)
        {
            let half_y = DRAG_ICON_SIZE * 0.5;
            let half_x = ui_width(half_y);
            let icon_width = ui_width(DRAG_ICON_SIZE);
            let min_x = (cursor.0 - half_x).clamp(0.0, 1.0 - icon_width);
            let min_y = (cursor.1 - half_y).clamp(0.0, 1.0 - DRAG_ICON_SIZE);
            let max_x = (min_x + icon_width).min(0.995);
            let max_y = (min_y + DRAG_ICON_SIZE).min(0.995);
            ui.add_rect_textured(
                (min_x, min_y),
                (max_x, max_y),
                block.atlas_coords(BlockFace::Top),
                [1.0, 1.0, 1.0, 0.92],
            );
            ui.add_rect((min_x, min_y), (max_x, max_y), [0.95, 0.98, 1.0, 0.32]);
        }
    }
    fn build_ui_geometry(&self) -> UiGeometry {
        let mut ui = UiGeometry::new(self.ui_scaler);

        if self.mouse_grabbed && !self.is_in_menu() {
            let center = self.crosshair_ui_center();
            let thickness = 0.0045;
            let half_thickness = thickness * 0.5;
            let half_thickness_x = ui_width(half_thickness);
            let gap = 0.014;
            let gap_x = ui_width(gap);
            let arm = 0.03;
            let arm_x = ui_width(arm);
            let crosshair_color = [1.0, 1.0, 1.0, 0.78];

            ui.add_rect(
                (center.0 - half_thickness_x, center.1 - gap - arm),
                (center.0 + half_thickness_x, center.1 - gap),
                crosshair_color,
            );
            ui.add_rect(
                (center.0 - half_thickness_x, center.1 + gap),
                (center.0 + half_thickness_x, center.1 + gap + arm),
                crosshair_color,
            );
            ui.add_rect(
                (center.0 - gap_x - arm_x, center.1 - half_thickness),
                (center.0 - gap_x, center.1 + half_thickness),
                crosshair_color,
            );
            ui.add_rect(
                (center.0 + gap_x, center.1 - half_thickness),
                (center.0 + gap_x + arm_x, center.1 + half_thickness),
                crosshair_color,
            );

            let dot = 0.006;
            let dot_half = dot * 0.5;
            let dot_half_x = ui_width(dot_half);
            ui.add_rect(
                (center.0 - dot_half_x, center.1 - dot_half),
                (center.0 + dot_half_x, center.1 + dot_half),
                [1.0, 1.0, 1.0, 0.9],
            );
        }

        if let Some(editor) = &self.config_editor {
            self.draw_config_overlay(&mut ui, editor);
        } else if let Some(info) = &self.inspect_info {
            self.draw_inspect_overlay(&mut ui, info);
        }

        if !self.paused {
            self.draw_hotbar(&mut ui);
        }

        if self.inventory_open {
            self.draw_inventory_overlay(&mut ui);
        }

        if self.settings_open {
            self.draw_settings_overlay(&mut ui);
        } else if self.paused {
            self.draw_pause_overlay(&mut ui);
        }

        ui
    }

    fn draw_inspect_overlay(&self, ui: &mut UiGeometry, info: &InspectInfo) {
        let width = ui_width(0.36);
        let height = 0.09;
        let min = (0.5 - width * 0.5, 0.04);
        let max = (min.0 + width, min.1 + height);
        ui.add_panel(
            min,
            max,
            [0.12, 0.14, 0.2, 0.9],
            [0.08, 0.09, 0.14, 0.94],
            Some([0.34, 0.52, 0.86, 0.32]),
        );
        ui.add_text(
            (min.0 + ui_width(0.02), min.1 + 0.02),
            0.018,
            [0.92, 0.95, 1.0, 1.0],
            &info.label.to_ascii_uppercase(),
        );

        let mut lines: Vec<String> = vec![format!(
            "Live Voltage: {:.2} V | Live Current: {:.2} A",
            info.telemetry.voltage, info.telemetry.current
        )];
        let orientation_line = match info.component {
            ElectricalComponent::Ground => format!(
                "Ground link: {} <-> {}",
                block_face_name(info.positive_face),
                block_face_name(info.negative_face)
            ),
            _ => format!(
                "Axis: {} | Positive: {} | Negative: {}",
                axis_name(info.axis),
                block_face_name(info.positive_face),
                block_face_name(info.negative_face)
            ),
        };
        lines.push(orientation_line);
        match info.component {
            ElectricalComponent::VoltageSource => {
                if let Some(v) = info.params.voltage_volts {
                    lines.push(format!("Rated Voltage: {:.2} V", v));
                }
                if let Some(r) = info.params.resistance_ohms {
                    lines.push(format!("Internal R: {:.2} OHM", r));
                }
                if let Some(i) = info.params.max_current_amps {
                    lines.push(format!("Max Current: {:.2} A", i));
                }
            }
            ElectricalComponent::Resistor | ElectricalComponent::Wire => {
                if let Some(r) = info.params.resistance_ohms {
                    lines.push(format!("Resistance: {:.2} OHM", r));
                }
                if let Some(i) = info.params.max_current_amps {
                    lines.push(format!("Rated Current: {:.2} A", i));
                }
            }
            ElectricalComponent::Ground => {
                lines.push("Reference node".to_string());
            }
        }
        if lines.len() == 1 {
            lines.push("No component parameters".to_string());
        }

        let mut y = min.1 + 0.048;
        let line_height = 0.016;
        for line in &lines {
            ui.add_text(
                (min.0 + ui_width(0.02), y),
                line_height,
                [0.88, 0.92, 1.0, 1.0],
                line,
            );
            y += line_height + 0.008;
        }
    }
    fn draw_config_overlay(&self, ui: &mut UiGeometry, editor: &ConfigEditor) {
        let width = 0.46;
        let height = 0.2;
        let min = (0.5 - width * 0.5, 0.22);
        let max = (0.5 + width * 0.5, 0.22 + height);
        ui.add_panel(
            min,
            max,
            [0.1, 0.12, 0.18, 0.9],
            [0.06, 0.07, 0.1, 0.95],
            Some([0.28, 0.42, 0.85, 0.25]),
        );
        ui.add_text(
            (min.0 + 0.02, min.1 + 0.024),
            0.02,
            [0.95, 0.97, 1.0, 1.0],
            &format!("CONFIGURE {}", editor.label.to_ascii_uppercase()),
        );

        let telemetry = self
            .world
            .electrical()
            .telemetry_at(editor.handle.pos, editor.handle.face)
            .unwrap_or_default();
        let axis = self
            .world
            .electrical()
            .axis_at(editor.handle.pos, editor.handle.face)
            .unwrap_or_else(|| editor.component.default_axis());
        let (positive_face, negative_face) =
            editor.component.terminal_faces(axis, editor.handle.face);
        let mut lines: Vec<String> = vec![format!(
            "Live Voltage: {:.2} V | Live Current: {:.2} A",
            telemetry.voltage, telemetry.current
        )];
        let orientation_line = match editor.component {
            ElectricalComponent::Ground => format!(
                "Ground link: {} <-> {}",
                block_face_name(positive_face),
                block_face_name(negative_face)
            ),
            _ => format!(
                "Axis: {} | Positive: {} | Negative: {}",
                axis_name(axis),
                block_face_name(positive_face),
                block_face_name(negative_face)
            ),
        };
        lines.push(orientation_line);
        match editor.component {
            ElectricalComponent::VoltageSource => {
                if let Some(v) = editor.params.voltage_volts {
                    lines.push(format!("Rated Voltage: {:.2} V", v));
                }
                if let Some(i) = editor.params.max_current_amps {
                    lines.push(format!("Max Current: {:.2} A", i));
                }
                if let Some(r) = editor.params.resistance_ohms {
                    lines.push(format!("Internal R: {:.2} OHM", r));
                }
            }
            ElectricalComponent::Resistor => {
                if let Some(r) = editor.params.resistance_ohms {
                    lines.push(format!("Resistance: {:.2} OHM", r));
                }
                if let Some(i) = editor.params.max_current_amps {
                    lines.push(format!("Rated Current: {:.2} A", i));
                }
            }
            _ => {}
        }

        let mut y = min.1 + 0.072;
        let line_height = 0.016;
        for line in &lines {
            ui.add_text((min.0 + 0.02, y), line_height, [0.88, 0.92, 1.0, 1.0], line);
            y += line_height + 0.008;
        }

        let instructions: &[&str] = match editor.component {
            ElectricalComponent::VoltageSource => &[
                "UP/DOWN: adjust voltage",
                "LEFT/RIGHT: adjust max current",
                "ENTER: apply   ESC: close",
            ],
            ElectricalComponent::Resistor => &[
                "UP/DOWN: adjust resistance",
                "LEFT/RIGHT: adjust max current",
                "ENTER: apply   ESC: close",
            ],
            _ => &["ENTER: apply   ESC: close"],
        };

        for line in instructions {
            ui.add_text((min.0 + 0.02, y), 0.014, [0.76, 0.82, 0.94, 1.0], line);
            y += 0.02;
        }
    }

    fn update_inspect_state(
        &mut self,
        target: Option<AttachmentTarget>,
        info: Option<InspectInfo>,
    ) {
        if self.highlight_target != target {
            self.highlight_target = target;
        }
        if self.inspect_info != info {
            self.inspect_info = info;
            self.mark_ui_dirty();
        }
    }

    fn collect_power_highlights(
        &self,
        min_current: f32,
    ) -> Vec<(Vector3<f32>, ElectricalComponent, ComponentTelemetry)> {
        self.world
            .electrical()
            .powered_nodes(min_current)
            .into_iter()
            .map(|(pos, component, telemetry)| {
                (
                    Vector3::new(pos.x as f32 + 0.5, pos.y as f32 + 0.5, pos.z as f32 + 0.5),
                    component,
                    telemetry,
                )
            })
            .collect()
    }

    fn inspect_info_for(&self, handle: AttachmentTarget) -> Option<InspectInfo> {
        let component = self
            .world
            .electrical()
            .component_at(handle.pos, handle.face)?;
        let params = self
            .world
            .electrical()
            .params_at(handle.pos, handle.face)
            .unwrap_or_else(|| component.default_params());
        let telemetry = self
            .world
            .electrical()
            .telemetry_at(handle.pos, handle.face)
            .unwrap_or_default();
        let label = component.block_type().name().to_string();
        let axis = self
            .world
            .electrical()
            .axis_at(handle.pos, handle.face)
            .unwrap_or_else(|| component.default_axis());
        let (positive_face, negative_face) = component.terminal_faces(axis, handle.face);
        Some(InspectInfo {
            handle,
            label,
            component,
            axis,
            positive_face,
            negative_face,
            params,
            telemetry,
        })
    }

    fn refresh_inspect_info(&mut self) {
        let info = self
            .highlight_target
            .and_then(|handle| self.inspect_info_for(handle));
        self.update_inspect_state(self.highlight_target, info);
    }

    fn open_config_editor(
        &mut self,
        handle: AttachmentTarget,
        component: ElectricalComponent,
        params: ComponentParams,
    ) {
        self.enter_menu_mode();
        self.config_editor = Some(ConfigEditor {
            handle,
            label: component.block_type().name().to_string(),
            component,
            params,
        });
        self.mark_ui_dirty();
    }

    fn close_config_editor(&mut self) {
        if self.config_editor.take().is_some() {
            self.exit_menu_mode_if_needed();
            self.refresh_inspect_info();
            self.mark_ui_dirty();
        }
    }

    fn toggle_config_editor(&mut self) -> bool {
        if self.config_editor.is_some() {
            self.close_config_editor();
            return true;
        }
        if self.inventory_open || self.paused {
            return false;
        }
        let Some(handle) = self.highlight_target else {
            return false;
        };
        let Some(component) = self
            .world
            .electrical()
            .component_at(handle.pos, handle.face)
        else {
            return false;
        };
        if !matches!(
            component,
            ElectricalComponent::Resistor | ElectricalComponent::VoltageSource
        ) {
            return false;
        }
        let params = self
            .world
            .electrical()
            .params_at(handle.pos, handle.face)
            .unwrap_or_else(|| component.default_params());
        self.open_config_editor(handle, component, params);
        true
    }

    fn handle_config_key(&mut self, key: KeyCode) -> bool {
        if self.config_editor.is_none() {
            return false;
        }
        match key {
            KeyCode::Escape => {
                self.close_config_editor();
                true
            }
            KeyCode::Enter => {
                self.close_config_editor();
                true
            }
            KeyCode::ArrowUp => {
                self.adjust_config_primary(1.0);
                true
            }
            KeyCode::ArrowDown => {
                self.adjust_config_primary(-1.0);
                true
            }
            KeyCode::ArrowLeft => {
                self.adjust_config_secondary(-1.0);
                true
            }
            KeyCode::ArrowRight => {
                self.adjust_config_secondary(1.0);
                true
            }
            _ => false,
        }
    }

    fn adjust_config_primary(&mut self, direction: f32) {
        if let Some(editor) = self.config_editor.as_mut() {
            match editor.component {
                ElectricalComponent::VoltageSource => {
                    if let Some(mut value) = editor.params.voltage_volts {
                        value = (value + direction * 1.0).max(0.0);
                        editor.params.voltage_volts = Some(value);
                    }
                }
                ElectricalComponent::Resistor => {
                    if let Some(mut value) = editor.params.resistance_ohms {
                        value = (value + direction * 10.0).max(0.1);
                        editor.params.resistance_ohms = Some(value);
                    }
                }
                _ => {}
            }
            self.commit_config_params();
        }
    }

    fn adjust_config_secondary(&mut self, direction: f32) {
        if let Some(editor) = self.config_editor.as_mut() {
            match editor.component {
                ElectricalComponent::VoltageSource | ElectricalComponent::Resistor => {
                    let current = editor.params.max_current_amps.unwrap_or(0.0);
                    let new_current = (current + direction * 0.5).max(0.0);
                    editor.params.max_current_amps = Some(new_current);
                }
                _ => {}
            }
            self.commit_config_params();
        }
    }

    fn commit_config_params(&mut self) {
        if let Some(editor) = &self.config_editor {
            self.world.electrical_mut().set_params(
                editor.handle.pos,
                editor.handle.face,
                editor.params,
            );
            self.refresh_inspect_info();
            self.mark_ui_dirty();
        }
    }

    fn update(&mut self) {
        let now = Instant::now();
        let frame_dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.tick_accumulator += frame_dt;
        self.animation_time += frame_dt;

        let frame_profiler = profiler::begin_frame();
        let _update_scope = frame_profiler
            .as_ref()
            .map(|ctx| ctx.section("frame_update"));

        let in_menu = self.is_in_menu();
        let mut ticks_executed = 0;
        while self.tick_accumulator >= FIXED_TICK_STEP && ticks_executed < MAX_TICKS_PER_FRAME {
            self.tick_accumulator -= FIXED_TICK_STEP;
            self.fixed_update(FIXED_TICK_STEP, in_menu, &frame_profiler);
            ticks_executed += 1;
        }
        if ticks_executed == MAX_TICKS_PER_FRAME {
            // Avoid spiral of death; keep a small remainder to catch up gradually.
            self.tick_accumulator = self.tick_accumulator.min(FIXED_TICK_STEP);
        }

        self.frame_update(frame_dt, in_menu, ticks_executed, &frame_profiler);

        if self.ui_dirty {
            profiler::scope(&frame_profiler, "ui_rebuild", || {
                self.rebuild_ui();
            });
        }
    }

    fn fixed_update(
        &mut self,
        tick_dt: f32,
        in_menu: bool,
        frame_profiler: &Option<profiler::FrameCtx>,
    ) {
        if in_menu {
            self.controller.reset_motion();
            let base_fov = self.projection.base_fov();
            self.projection.set_target_fov(base_fov);
        } else {
            {
                let world_ref = &self.world;
                let check_collision =
                    |pos: cgmath::Point3<f32>| player_aabb_collides(world_ref, pos);
                self.controller
                    .update_camera(&mut self.camera, tick_dt, check_collision);
            }
            let sprint_bonus = if self.controller.is_sprinting() {
                7.0_f32.to_radians()
            } else {
                0.0
            };
            let base_fov = self.projection.base_fov();
            self.projection
                .set_target_fov(Rad(base_fov.0 + sprint_bonus));
        }
        self.projection.animate(tick_dt);

        self.world.advance_time(tick_dt);

        // Increment tick counters
        self.water_tick_counter = self.water_tick_counter.wrapping_add(1);

        if self.debug_mode {
            self.debug_tick_counter = self.debug_tick_counter.wrapping_add(1);
            if self.debug_tick_counter % FIXED_TICK_RATE as u32 == 0 {
                let pos = self.camera.position;
                let block_below = self.world.get_block(
                    pos.x.floor() as i32,
                    (pos.y - 0.1).floor() as i32,
                    pos.z.floor() as i32,
                );
                println!(
                    "Pos: ({:.2}, {:.2}, {:.2}) | Below: {:?} | Noclip: {}",
                    pos.x, pos.y, pos.z, block_below, self.controller.noclip
                );
            }
        }

        let updated_chunks = if !in_menu {
            profiler::scope(&frame_profiler, "world_update_chunks", || {
                self.world.update_loaded_chunks(self.camera.position, 3)
            })
        } else {
            false
        };
        if updated_chunks {
            self.world_dirty = true;
            self.force_full_remesh = true;
            self.dirty_chunks.clear();
        }

        // Water simulation runs every 10 ticks (6 times per second) to reduce lag
        if self.water_tick_counter % WATER_UPDATE_INTERVAL == 0 {
            if profiler::scope(&frame_profiler, "fluid_poll", || {
                self.fluid_system.poll_results(&mut self.world)
            }) {
                self.world_dirty = true;
                self.force_full_remesh = true;
                self.dirty_chunks.clear();
            }

            if !in_menu {
                profiler::scope(&frame_profiler, "fluid_pump", || {
                    self.fluid_system.pump(&self.world);
                });
            }

            if profiler::scope(&frame_profiler, "fluid_fallback", || {
                self.fluid_system.fallback_step(&mut self.world)
            }) {
                self.world_dirty = true;
                self.force_full_remesh = true;
                self.dirty_chunks.clear();
            }
        }

        profiler::scope(&frame_profiler, "electric_tick", || {
            self.world.tick_electrical();
        });
        self.refresh_inspect_info();
    }

    fn frame_update(
        &mut self,
        frame_dt: f32,
        in_menu: bool,
        ticks_executed: usize,
        frame_profiler: &Option<profiler::FrameCtx>,
    ) {
        if in_menu && ticks_executed == 0 {
            // Ensure motion is cleared when no fixed step ran this frame.
            self.controller.reset_motion();
            let base_fov = self.projection.base_fov();
            self.projection.set_target_fov(base_fov);
            self.projection.animate(frame_dt.min(FIXED_TICK_STEP));
        }

        self.renderer.update_camera(&self.camera, &self.projection);

        let atmosphere = self.world.atmosphere_at(
            self.camera.position.x.floor() as i32,
            self.camera.position.z.floor() as i32,
        );
        self.renderer.update_environment(
            &atmosphere,
            [
                self.camera.position.x,
                self.camera.position.y,
                self.camera.position.z,
            ],
        );
        let blended_clear = [
            (atmosphere.sky_zenith[0] + atmosphere.sky_horizon[0]) * 0.5,
            (atmosphere.sky_zenith[1] + atmosphere.sky_horizon[1]) * 0.5,
            (atmosphere.sky_zenith[2] + atmosphere.sky_horizon[2]) * 0.5,
        ];
        self.renderer.set_clear_color(blended_clear);

        let mut highlight_bounds = None;
        let mut new_highlight = None;
        let mut new_info = None;

        if !in_menu {
            let direction = self.crosshair_direction();
            if let Some(hit) = raycast(&self.world, self.camera.position, direction, 6.0) {
                let pad = 0.002;
                let min = [
                    hit.block_pos.0 as f32 - 0.5 - pad,
                    hit.block_pos.1 as f32 - 0.5 - pad,
                    hit.block_pos.2 as f32 - 0.5 - pad,
                ];
                let max = [
                    hit.block_pos.0 as f32 + 0.5 + pad,
                    hit.block_pos.1 as f32 + 0.5 + pad,
                    hit.block_pos.2 as f32 + 0.5 + pad,
                ];
                highlight_bounds = Some((min, max));

                let face = BlockFace::from_normal_f32(hit.normal)
                    .or_else(|| BlockFace::from_normal_f32(-hit.normal))
                    .unwrap_or(BlockFace::Top);
                let pos = BlockPos3::new(hit.block_pos.0, hit.block_pos.1, hit.block_pos.2);
                if let Some(component) = self.world.electrical().component_at(pos, face) {
                    let params = self
                        .world
                        .electrical()
                        .params_at(pos, face)
                        .unwrap_or_else(|| component.default_params());
                    let telemetry = self
                        .world
                        .electrical()
                        .telemetry_at(pos, face)
                        .unwrap_or_default();
                    let label = component.block_type().name().to_string();
                    let axis = self
                        .world
                        .electrical()
                        .axis_at(pos, face)
                        .unwrap_or_else(|| component.default_axis());
                    let (positive_face, negative_face) = component.terminal_faces(axis, face);
                    let handle = AttachmentTarget { pos, face };
                    new_highlight = Some(handle);
                    new_info = Some(InspectInfo {
                        handle,
                        label,
                        component,
                        axis,
                        positive_face,
                        negative_face,
                        params,
                        telemetry,
                    });
                }
            }
        }

        let power_instances = if in_menu {
            Vec::new()
        } else {
            self.collect_power_highlights(0.01)
        };
        self.renderer
            .update_power_overlays(&power_instances, self.animation_time);
        self.renderer.update_highlight(highlight_bounds);
        self.update_inspect_state(new_highlight, new_info);

        if in_menu {
            self.renderer.update_hand(None, &self.camera);
        } else {
            self.renderer
                .update_hand(self.inventory.selected_block(), &self.camera);
        }

        if !in_menu && self.world_dirty {
            profiler::scope(&frame_profiler, "mesh_update", || {
                if self.force_full_remesh {
                    self.renderer.rebuild_world_mesh(&self.world);
                    self.dirty_chunks.clear();
                } else {
                    let dirty_chunks: HashSet<ChunkPos> = self.dirty_chunks.drain().collect();
                    self.renderer.update_chunks(&self.world, &dirty_chunks);
                }
            });
            self.world_dirty = false;
            self.force_full_remesh = false;
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let start = Instant::now();
        let result = self.renderer.render();
        profiler::record_background("render", start.elapsed());
        result
    }
}

fn player_aabb_collides(world: &World, pos: cgmath::Point3<f32>) -> bool {
    const EPSILON: f32 = 0.001;

    let bottom = pos.y - PLAYER_EYE_HEIGHT;
    let top = bottom + PLAYER_HEIGHT;

    let min_x_bound = pos.x - PLAYER_RADIUS;
    let max_x_bound = pos.x + PLAYER_RADIUS;
    let min_y_bound = bottom;
    let max_y_bound = top;
    let min_z_bound = pos.z - PLAYER_RADIUS;
    let max_z_bound = pos.z + PLAYER_RADIUS;

    let min_x = (min_x_bound - 0.5).ceil() as i32;
    let max_x = (max_x_bound + 0.5 - EPSILON).floor() as i32;
    let min_y = (min_y_bound - 0.5).ceil() as i32;
    let max_y = (max_y_bound + 0.5 - EPSILON).floor() as i32;
    let min_z = (min_z_bound - 0.5).ceil() as i32;
    let max_z = (max_z_bound + 0.5 - EPSILON).floor() as i32;

    if min_x > max_x || min_y > max_y || min_z > max_z {
        return false;
    }

    for x in min_x..=max_x {
        for y in min_y..=max_y {
            for z in min_z..=max_z {
                if world.get_block(x, y, z).is_solid() {
                    return true;
                }
            }
        }
    }

    false
}

fn find_surface_level(world: &World, x: i32, z: i32) -> Option<f32> {
    for y in (0..CHUNK_HEIGHT as i32).rev() {
        if world.get_block(x, y, z).is_solid() {
            return Some(y as f32 + 0.5);
        }
    }
    None
}

#[derive(Clone, Copy, Debug)]
struct UiScaler {
    safe_width: f32,
    safe_height: f32,
    offset_x: f32,
    offset_y: f32,
}

impl UiScaler {
    const REFERENCE_ASPECT: f32 = UI_REFERENCE_ASPECT;

    fn new(aspect: f32) -> Self {
        let aspect = if aspect.is_normal() && aspect > 0.0 {
            aspect
        } else {
            Self::REFERENCE_ASPECT
        };

        let (safe_width, safe_height) = if aspect >= Self::REFERENCE_ASPECT {
            (Self::REFERENCE_ASPECT / aspect, 1.0)
        } else {
            (1.0, aspect / Self::REFERENCE_ASPECT)
        };

        let offset_x = (1.0 - safe_width) * 0.5;
        let offset_y = (1.0 - safe_height) * 0.5;

        Self {
            safe_width,
            safe_height,
            offset_x,
            offset_y,
        }
    }

    fn project(&self, point: (f32, f32)) -> (f32, f32) {
        (
            point.0 * self.safe_width + self.offset_x,
            point.1 * self.safe_height + self.offset_y,
        )
    }

    fn project_rect(&self, min: (f32, f32), max: (f32, f32)) -> Option<((f32, f32), (f32, f32))> {
        let min_x = min.0.min(max.0);
        let min_y = min.1.min(max.1);
        let max_x = max.0.max(min.0);
        let max_y = max.1.max(min.1);

        let mapped_min = self.project((min_x, min_y));
        let mapped_max = self.project((max_x, max_y));

        let clamped_min = (mapped_min.0.clamp(0.0, 1.0), mapped_min.1.clamp(0.0, 1.0));
        let clamped_max = (mapped_max.0.clamp(0.0, 1.0), mapped_max.1.clamp(0.0, 1.0));

        if clamped_max.0 <= clamped_min.0 || clamped_max.1 <= clamped_min.1 {
            return None;
        }

        Some((clamped_min, clamped_max))
    }

    fn unproject(&self, point: (f32, f32)) -> (f32, f32) {
        let x = if self.safe_width > f32::EPSILON {
            ((point.0 - self.offset_x) / self.safe_width).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let y = if self.safe_height > f32::EPSILON {
            ((point.1 - self.offset_y) / self.safe_height).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (x, y)
    }
}

const FONT_WIDTH: usize = 5;
const FONT_HEIGHT: usize = 7;

fn glyph_for_char(ch: char) -> Option<[u8; FONT_HEIGHT]> {
    match ch {
        'A' => Some([
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ]),
        'B' => Some([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ]),
        'C' => Some([
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ]),
        'D' => Some([
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ]),
        'E' => Some([
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ]),
        'F' => Some([
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ]),
        'G' => Some([
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ]),
        'H' => Some([
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ]),
        'I' => Some([
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ]),
        'J' => Some([
            0b00001, 0b00001, 0b00001, 0b00001, 0b10001, 0b10001, 0b01110,
        ]),
        'K' => Some([
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ]),
        'L' => Some([
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ]),
        'M' => Some([
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ]),
        'N' => Some([
            0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001,
        ]),
        'O' => Some([
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ]),
        'P' => Some([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ]),
        'Q' => Some([
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ]),
        'R' => Some([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ]),
        'S' => Some([
            0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110,
        ]),
        'T' => Some([
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ]),
        'U' => Some([
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ]),
        'V' => Some([
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ]),
        'W' => Some([
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ]),
        'X' => Some([
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ]),
        'Y' => Some([
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ]),
        'Z' => Some([
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ]),
        '0' => Some([
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ]),
        '1' => Some([
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ]),
        '2' => Some([
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ]),
        '3' => Some([
            0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110,
        ]),
        '4' => Some([
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ]),
        '5' => Some([
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ]),
        '6' => Some([
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ]),
        '7' => Some([
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ]),
        '8' => Some([
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ]),
        '9' => Some([
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ]),
        '-' => Some([
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ]),
        '.' => Some([
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100,
        ]),
        ':' => Some([
            0b00000, 0b00100, 0b00000, 0b00000, 0b00000, 0b00100, 0b00000,
        ]),
        '/' => Some([
            0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b00000, 0b00000,
        ]),
        '(' => Some([
            0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
        ]),
        ')' => Some([
            0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
        ]),
        '%' => Some([
            0b11001, 0b11010, 0b00100, 0b01000, 0b10110, 0b00110, 0b00000,
        ]),
        '!' => Some([
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100,
        ]),
        ',' => Some([
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b01000,
        ]),
        '\'' => Some([
            0b00100, 0b00100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000,
        ]),
        '"' => Some([
            0b01010, 0b01010, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000,
        ]),
        '?' => Some([
            0b01110, 0b10001, 0b00010, 0b00100, 0b00100, 0b00000, 0b00100,
        ]),
        '|' => Some([
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ]),
        _ => None,
    }
}
struct UiGeometry {
    scaler: UiScaler,
    vertices: Vec<UiVertex>,
    indices: Vec<u16>,
}

impl UiGeometry {
    fn new(scaler: UiScaler) -> Self {
        Self {
            scaler,
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn add_rect(&mut self, min: (f32, f32), max: (f32, f32), color: [f32; 4]) {
        self.add_rect_internal(min, max, color, None, true);
    }

    fn add_rect_fullscreen(&mut self, min: (f32, f32), max: (f32, f32), color: [f32; 4]) {
        self.add_rect_internal(min, max, color, None, false);
    }

    fn add_rect_textured(
        &mut self,
        min: (f32, f32),
        max: (f32, f32),
        tile: (u32, u32),
        tint: [f32; 4],
    ) {
        let uv = atlas_uv_bounds(tile.0, tile.1);
        self.add_rect_internal(min, max, tint, Some(uv), true);
    }

    fn add_panel(
        &mut self,
        min: (f32, f32),
        max: (f32, f32),
        border_color: [f32; 4],
        fill_color: [f32; 4],
        highlight_color: Option<[f32; 4]>,
    ) {
        self.add_rect(min, max, border_color);
        let inset = 0.004;
        let inner_min = (min.0 + inset, min.1 + inset);
        let inner_max = (max.0 - inset, max.1 - inset);
        if inner_max.0 <= inner_min.0 || inner_max.1 <= inner_min.1 {
            return;
        }
        self.add_rect(inner_min, inner_max, fill_color);

        if let Some(color) = highlight_color {
            let highlight_height = ((max.1 - min.1) * 0.18).clamp(0.004, max.1 - min.1);
            let top_max = (
                inner_max.0,
                (inner_min.1 + highlight_height).min(inner_max.1),
            );
            self.add_rect(inner_min, top_max, color);
        }
    }

    fn add_text(&mut self, origin: (f32, f32), height: f32, color: [f32; 4], text: &str) {
        if height <= 0.0 {
            return;
        }
        let scale = height / FONT_HEIGHT as f32;
        let char_width = FONT_WIDTH as f32 * scale;
        let spacing = scale * 0.4;
        let line_height = height + scale * 1.6;

        let mut cursor_x = origin.0;
        let mut cursor_y = origin.1;

        for ch in text.chars() {
            if ch == '\n' {
                cursor_x = origin.0;
                cursor_y += line_height;
                continue;
            }
            if ch == ' ' {
                cursor_x += char_width + spacing;
                continue;
            }
            let upper = ch.to_ascii_uppercase();
            if let Some(pattern) = glyph_for_char(upper) {
                for (row, bits) in pattern.iter().enumerate() {
                    for col in 0..FONT_WIDTH {
                        if (bits >> (FONT_WIDTH - 1 - col)) & 1 == 1 {
                            let min =
                                (cursor_x + col as f32 * scale, cursor_y + row as f32 * scale);
                            let max = (min.0 + scale, min.1 + scale);
                            self.add_rect(min, max, color);
                        }
                    }
                }
                cursor_x += char_width + spacing;
            } else {
                cursor_x += char_width + spacing;
            }
            if cursor_x > 1.2 {
                cursor_x = origin.0;
                cursor_y += line_height;
            }
        }
    }

    fn add_rect_internal(
        &mut self,
        min: (f32, f32),
        max: (f32, f32),
        color: [f32; 4],
        uv_bounds: Option<(f32, f32, f32, f32)>,
        scaled: bool,
    ) {
        let mapped = if scaled {
            self.scaler.project_rect(min, max)
        } else {
            let min_x = min.0.min(max.0).clamp(0.0, 1.0);
            let min_y = min.1.min(max.1).clamp(0.0, 1.0);
            let max_x = max.0.max(min.0).clamp(0.0, 1.0);
            let max_y = max.1.max(min.1).clamp(0.0, 1.0);
            if max_x <= min_x || max_y <= min_y {
                return;
            }
            Some(((min_x, min_y), (max_x, max_y)))
        };

        let Some((proj_min, proj_max)) = mapped else {
            return;
        };

        let x0 = proj_min.0 * 2.0 - 1.0;
        let x1 = proj_max.0 * 2.0 - 1.0;
        let y0 = 1.0 - proj_min.1 * 2.0;
        let y1 = 1.0 - proj_max.1 * 2.0;

        let base = self.vertices.len();
        if base > (u16::MAX as usize) - 4 {
            return;
        }
        let base_index = base as u16;

        let positions = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];

        let (uvs, mode) = if let Some((u_min, u_max, v_min, v_max)) = uv_bounds {
            (
                [
                    [u_min, v_min],
                    [u_max, v_min],
                    [u_max, v_max],
                    [u_min, v_max],
                ],
                1.0,
            )
        } else {
            ([[0.0, 0.0]; 4], 0.0)
        };

        for (pos, uv) in positions.into_iter().zip(uvs) {
            self.vertices.push(UiVertex {
                position: pos,
                color,
                uv,
                mode,
            });
        }

        self.indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);
    }
}

fn main() -> anyhow::Result<()> {
    println!("╔════════════════════════════════════════╗");
    println!("║     MINECRAFT CLONE - VOXEL WORLD     ║");
    println!("╚════════════════════════════════════════╝");
    println!();
    println!("CONTROLS:");
    println!("  Click           - Grab mouse");
    println!("  ESC             - Release mouse");
    println!("  W/A/S/D         - Move (fly when noclip ON)");
    println!("  Space           - Jump / Up");
    println!("  F               - Toggle Noclip (collision ON/OFF)");
    println!("  F3              - Toggle Debug Info");
    println!("  Mouse           - Look around");
    println!("  Left Click      - Break block");
    println!("  Right Click     - Place block");
    println!("  1-9 Keys        - Select block type");
    println!("  Mouse Wheel     - Cycle inventory");
    println!();
    println!("BLOCKS AVAILABLE:");
    println!("  1-Grass  2-Dirt  3-Stone  4-Copper Wire  5-Voltage Source  6-Ground");
    println!("  7-Water  8-Rose  9-Tulip");
    println!();

    if let Err(err) = profiler::init_session() {
        eprintln!("Failed to initialise profiler: {err:?}");
    }

    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Minecraft Clone - Voxel Builder")
        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
        .build(&event_loop)?;

    let mut state = State::new(&window)?;

    event_loop.run(move |event, target| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == state.window().id() => {
            if !state.input(event) {
                match event {
                    WindowEvent::CloseRequested => target.exit(),
                    WindowEvent::Resized(physical_size) => state.resize(*physical_size),
                    WindowEvent::ScaleFactorChanged { .. } => {
                        let new_size = state.window().inner_size();
                        state.resize(new_size)
                    }
                    WindowEvent::RedrawRequested => match state.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => {
                            let size = state.window().inner_size();
                            state.resize(size);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => target.exit(),
                        Err(e) => eprintln!("render error: {e:?}"),
                    },
                    WindowEvent::Focused(false) => state.set_mouse_grab(false),
                    WindowEvent::KeyboardInput { event, .. } => {
                        if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                            if event.state == ElementState::Pressed {
                                state.set_mouse_grab(false);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Event::DeviceEvent {
            event: DeviceEvent::MouseMotion { delta },
            ..
        } => {
            state.mouse_motion(delta);
        }
        Event::AboutToWait => {
            state.update();
            state.window().request_redraw();
        }
        _ => {}
    })?;

    Ok(())
}
