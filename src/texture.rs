use wgpu::util::DeviceExt;

pub const TILE_SIZE: u32 = 16;
pub const ATLAS_COLS: u32 = 39;
pub const ATLAS_ROWS: u32 = 1;
pub const ATLAS_WIDTH: u32 = TILE_SIZE * ATLAS_COLS;
pub const ATLAS_HEIGHT: u32 = TILE_SIZE * ATLAS_ROWS;

pub type TileCoord = (u32, u32);

pub const TILE_WIRE_TOP_CONNECTED: TileCoord = (20, 0);
pub const TILE_WIRE_TOP_UNCONNECTED: TileCoord = (21, 0);
pub const TILE_WIRE_SIDE_CONNECTED: TileCoord = (22, 0);
pub const TILE_WIRE_SIDE_UNCONNECTED: TileCoord = (23, 0);
pub const TILE_RESISTOR_TOP_CONNECTED: TileCoord = (24, 0);
pub const TILE_RESISTOR_TOP_UNCONNECTED: TileCoord = (25, 0);
pub const TILE_RESISTOR_SIDE_CONNECTED: TileCoord = (26, 0);
pub const TILE_RESISTOR_SIDE_UNCONNECTED: TileCoord = (27, 0);
pub const TILE_VOLTAGE_TOP_CONNECTED: TileCoord = (28, 0);
pub const TILE_VOLTAGE_TOP_UNCONNECTED: TileCoord = (29, 0);
pub const TILE_VOLTAGE_SIDE_CONNECTED: TileCoord = (30, 0);
pub const TILE_VOLTAGE_SIDE_UNCONNECTED: TileCoord = (31, 0);
pub const TILE_GROUND_TOP_CONNECTED: TileCoord = (32, 0);
pub const TILE_GROUND_TOP_UNCONNECTED: TileCoord = (33, 0);
pub const TILE_GROUND_SIDE_CONNECTED: TileCoord = (34, 0);
pub const TILE_GROUND_SIDE_UNCONNECTED: TileCoord = (35, 0);

pub const TILE_FLOWER_ROSE_PETAL: TileCoord = (11, 0);
pub const TILE_FLOWER_TULIP_PETAL: TileCoord = (12, 0);
pub const TILE_FLOWER_STEM: TileCoord = (36, 0);
pub const TILE_FLOWER_LEAF: TileCoord = (37, 0);
pub const TILE_GLOW_SHROOM_CAP: TileCoord = (38, 0);

pub fn atlas_uv_bounds(tile_x: u32, tile_y: u32) -> (f32, f32, f32, f32) {
    let tile_width = 1.0 / ATLAS_COLS as f32;
    let tile_height = 1.0 / ATLAS_ROWS as f32;
    let pad_u = 0.5 / (TILE_SIZE as f32 * ATLAS_COLS as f32);
    let pad_v = 0.5 / (TILE_SIZE as f32 * ATLAS_ROWS as f32);

    let u_min = tile_x as f32 * tile_width + pad_u;
    let u_max = (tile_x + 1) as f32 * tile_width - pad_u;
    let v_min = tile_y as f32 * tile_height + pad_v;
    let v_max = (tile_y + 1) as f32 * tile_height - pad_v;

    (u_min, u_max, v_min, v_max)
}

pub struct TextureAtlas {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl TextureAtlas {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let mut pixels = vec![0u8; (ATLAS_WIDTH * ATLAS_HEIGHT * 4) as usize];

        generate_tiles(&mut pixels);

        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("texture_atlas"),
                size: wgpu::Extent3d {
                    width: ATLAS_WIDTH,
                    height: ATLAS_HEIGHT,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &pixels,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("texture_atlas_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture_atlas_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture_atlas_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            _texture: texture,
            _view: view,
            _sampler: sampler,
            bind_group_layout,
            bind_group,
        }
    }
}

fn generate_tiles(pixels: &mut [u8]) {
    fill_tile(pixels, 0, 0, grass_top_pattern);
    fill_tile(pixels, 1, 0, grass_side_pattern);
    fill_tile(pixels, 2, 0, dirt_pattern);
    fill_tile(pixels, 3, 0, stone_pattern);
    fill_tile(pixels, 4, 0, wood_side_pattern);
    fill_tile(pixels, 5, 0, wood_top_pattern);
    fill_tile(pixels, 6, 0, sand_pattern);
    fill_tile_rgba(pixels, 7, 0, |gx, gy, lx, ly| {
        let color = leaves_pattern(gx, gy, lx, ly);
        let alpha = leaves_alpha(gx, gy, lx, ly);
        [color[0], color[1], color[2], alpha]
    });
    fill_tile(pixels, 8, 0, coal_ore_pattern);
    fill_tile(pixels, 9, 0, iron_ore_pattern);
    fill_tile_rgba(pixels, 10, 0, water_pattern);
    fill_tile(
        pixels,
        TILE_FLOWER_ROSE_PETAL.0,
        TILE_FLOWER_ROSE_PETAL.1,
        rose_petal_pattern,
    );
    fill_tile(
        pixels,
        TILE_FLOWER_TULIP_PETAL.0,
        TILE_FLOWER_TULIP_PETAL.1,
        tulip_petal_pattern,
    );
    fill_tile(
        pixels,
        TILE_FLOWER_STEM.0,
        TILE_FLOWER_STEM.1,
        flower_stem_pattern,
    );
    fill_tile(
        pixels,
        TILE_FLOWER_LEAF.0,
        TILE_FLOWER_LEAF.1,
        flower_leaf_pattern,
    );
    fill_tile(
        pixels,
        TILE_GLOW_SHROOM_CAP.0,
        TILE_GLOW_SHROOM_CAP.1,
        glow_shroom_pattern,
    );
    fill_tile(pixels, 13, 0, terracotta_pattern);
    fill_tile(pixels, 14, 0, lily_pad_pattern);
    fill_tile(pixels, 15, 0, snow_pattern);
    fill_tile(pixels, 16, 0, copper_wire_pattern);
    fill_tile(pixels, 17, 0, resistor_pattern);
    fill_tile(pixels, 18, 0, voltage_source_pattern);
    fill_tile(pixels, 19, 0, ground_pattern);
    fill_tile(
        pixels,
        TILE_WIRE_TOP_CONNECTED.0,
        TILE_WIRE_TOP_CONNECTED.1,
        |gx, gy, lx, ly| copper_wire_connection_top_pattern(gx, gy, lx, ly, true),
    );
    fill_tile(
        pixels,
        TILE_WIRE_TOP_UNCONNECTED.0,
        TILE_WIRE_TOP_UNCONNECTED.1,
        |gx, gy, lx, ly| copper_wire_connection_top_pattern(gx, gy, lx, ly, false),
    );
    fill_tile(
        pixels,
        TILE_WIRE_SIDE_CONNECTED.0,
        TILE_WIRE_SIDE_CONNECTED.1,
        |gx, gy, lx, ly| copper_wire_connection_side_pattern(gx, gy, lx, ly, true),
    );
    fill_tile(
        pixels,
        TILE_WIRE_SIDE_UNCONNECTED.0,
        TILE_WIRE_SIDE_UNCONNECTED.1,
        |gx, gy, lx, ly| copper_wire_connection_side_pattern(gx, gy, lx, ly, false),
    );
    fill_tile(
        pixels,
        TILE_RESISTOR_TOP_CONNECTED.0,
        TILE_RESISTOR_TOP_CONNECTED.1,
        |gx, gy, lx, ly| resistor_connection_top_pattern(gx, gy, lx, ly, true),
    );
    fill_tile(
        pixels,
        TILE_RESISTOR_TOP_UNCONNECTED.0,
        TILE_RESISTOR_TOP_UNCONNECTED.1,
        |gx, gy, lx, ly| resistor_connection_top_pattern(gx, gy, lx, ly, false),
    );
    fill_tile(
        pixels,
        TILE_RESISTOR_SIDE_CONNECTED.0,
        TILE_RESISTOR_SIDE_CONNECTED.1,
        |gx, gy, lx, ly| resistor_connection_side_pattern(gx, gy, lx, ly, true),
    );
    fill_tile(
        pixels,
        TILE_RESISTOR_SIDE_UNCONNECTED.0,
        TILE_RESISTOR_SIDE_UNCONNECTED.1,
        |gx, gy, lx, ly| resistor_connection_side_pattern(gx, gy, lx, ly, false),
    );
    fill_tile(
        pixels,
        TILE_VOLTAGE_TOP_CONNECTED.0,
        TILE_VOLTAGE_TOP_CONNECTED.1,
        |gx, gy, lx, ly| voltage_connection_top_pattern(gx, gy, lx, ly, true),
    );
    fill_tile(
        pixels,
        TILE_VOLTAGE_TOP_UNCONNECTED.0,
        TILE_VOLTAGE_TOP_UNCONNECTED.1,
        |gx, gy, lx, ly| voltage_connection_top_pattern(gx, gy, lx, ly, false),
    );
    fill_tile(
        pixels,
        TILE_VOLTAGE_SIDE_CONNECTED.0,
        TILE_VOLTAGE_SIDE_CONNECTED.1,
        |gx, gy, lx, ly| voltage_connection_side_pattern(gx, gy, lx, ly, true),
    );
    fill_tile(
        pixels,
        TILE_VOLTAGE_SIDE_UNCONNECTED.0,
        TILE_VOLTAGE_SIDE_UNCONNECTED.1,
        |gx, gy, lx, ly| voltage_connection_side_pattern(gx, gy, lx, ly, false),
    );
    fill_tile(
        pixels,
        TILE_GROUND_TOP_CONNECTED.0,
        TILE_GROUND_TOP_CONNECTED.1,
        |gx, gy, lx, ly| ground_connection_top_pattern(gx, gy, lx, ly, true),
    );
    fill_tile(
        pixels,
        TILE_GROUND_TOP_UNCONNECTED.0,
        TILE_GROUND_TOP_UNCONNECTED.1,
        |gx, gy, lx, ly| ground_connection_top_pattern(gx, gy, lx, ly, false),
    );
    fill_tile(
        pixels,
        TILE_GROUND_SIDE_CONNECTED.0,
        TILE_GROUND_SIDE_CONNECTED.1,
        |gx, gy, lx, ly| ground_connection_side_pattern(gx, gy, lx, ly, true),
    );
    fill_tile(
        pixels,
        TILE_GROUND_SIDE_UNCONNECTED.0,
        TILE_GROUND_SIDE_UNCONNECTED.1,
        |gx, gy, lx, ly| ground_connection_side_pattern(gx, gy, lx, ly, false),
    );
}

fn fill_tile<F>(pixels: &mut [u8], tile_x: u32, tile_y: u32, mut f: F)
where
    F: FnMut(u32, u32, u32, u32) -> [f32; 3],
{
    fill_tile_rgba(pixels, tile_x, tile_y, move |gx, gy, lx, ly| {
        let color = f(gx, gy, lx, ly);
        [color[0], color[1], color[2], 1.0]
    });
}

fn fill_tile_rgba<F>(pixels: &mut [u8], tile_x: u32, tile_y: u32, mut f: F)
where
    F: FnMut(u32, u32, u32, u32) -> [f32; 4],
{
    for ly in 0..TILE_SIZE {
        for lx in 0..TILE_SIZE {
            let gx = tile_x * TILE_SIZE + lx;
            let gy = tile_y * TILE_SIZE + ly;
            let color = f(gx, gy, lx, ly);
            let idx = ((gy * ATLAS_WIDTH + gx) * 4) as usize;
            pixels[idx] = to_u8(color[0]);
            pixels[idx + 1] = to_u8(color[1]);
            pixels[idx + 2] = to_u8(color[2]);
            pixels[idx + 3] = to_u8(color[3]);
        }
    }
}

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

fn noise(gx: u32, gy: u32, seed: u32) -> f32 {
    let n = gx
        .wrapping_mul(374_761_393)
        .wrapping_add(gy.wrapping_mul(668_265_263))
        .wrapping_add(seed.wrapping_mul(362_437));
    let n = (n << 13) ^ n;
    let nn = n.wrapping_mul(1_274_126_177);
    ((nn >> 16) & 0xFF) as f32 / 255.0
}

fn fbm_signed(gx: u32, gy: u32, seed: u32) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 0.5;
    let mut total = 0.0;
    let mut scale = 1u32;
    for i in 0..4 {
        let divisor = scale.max(1);
        let sample = noise(gx / divisor, gy / divisor, seed.wrapping_add(i as u32 * 97));
        value += sample * amplitude;
        total += amplitude;
        amplitude *= 0.5;
        scale = (scale << 1).max(1);
    }
    ((value / total.max(f32::EPSILON)) * 2.0 - 1.0).clamp(-1.0, 1.0)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < f32::EPSILON {
        if x >= edge1 {
            1.0
        } else {
            0.0
        }
    } else {
        let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
}

fn grass_top_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let base = [0.21, 0.68, 0.24];
    let coarse = fbm_signed(gx, gy, 19) * 0.12;
    let medium = fbm_signed(gx.wrapping_add(97), gy.wrapping_add(211), 29) * 0.08;
    let fine = (noise(gx.wrapping_add(17), gy.wrapping_add(37), 43) - 0.5) * 0.05;
    let blade = if ((gx.wrapping_add(gy).wrapping_add(lx).wrapping_add(ly)) & 3) == 0 {
        0.04
    } else {
        0.0
    };
    let dryness = fbm_signed(gx / 2, gy / 2, 61) * 0.10;
    let center = (TILE_SIZE as f32 - 1.0) * 0.5;
    let dx = lx as f32 - center;
    let dy = ly as f32 - center;
    let distance = (dx * dx + dy * dy).sqrt() / (TILE_SIZE as f32 * 0.9);
    let edge_highlight = (1.0 - distance.clamp(0.0, 1.0)).powf(2.0) * 0.05;

    [
        (base[0]
            + coarse * 0.5
            + medium * 0.4
            + fine * 0.3
            + dryness * 0.4
            + blade * 0.6
            + edge_highlight * 0.6)
            .clamp(0.0, 1.0),
        (base[1] + coarse * 0.6 + medium * 0.5 + fine * 0.2 - dryness * 0.5
            + blade * 0.3
            + edge_highlight * 0.4)
            .clamp(0.0, 1.0),
        (base[2] + coarse * 0.3 + medium * 0.2 + fine * 0.1 - dryness * 0.2 + edge_highlight * 0.2)
            .clamp(0.0, 1.0),
    ]
}
fn grass_side_pattern(gx: u32, gy: u32, _lx: u32, ly: u32) -> [f32; 3] {
    let grass_top = [0.24, 0.66, 0.23];
    let dirt_base = [0.42, 0.30, 0.18];

    let ridge = fbm_signed(gx, gy / 2, 71) * 2.2;
    let base_line = 4.6 + ridge;
    let y = ly as f32 + (noise(gx.wrapping_add(ly), gy.wrapping_add(113), 79) - 0.5) * 1.5;
    let mix = smoothstep(base_line - 1.2, base_line + 1.8, y);

    let grass_variation = fbm_signed(gx.wrapping_add(199), gy, 83) * 0.08;
    let dirt_variation = fbm_signed(gx.wrapping_add(311), gy.wrapping_add(613), 97) * 0.12;
    let blade = if ((gx.wrapping_add(ly)) & 3) == 0 {
        0.04
    } else {
        0.0
    };

    let grass = [
        (grass_top[0] + grass_variation * 0.6 + blade * 0.5).clamp(0.0, 1.0),
        (grass_top[1] + grass_variation * 0.4 + blade * 0.3).clamp(0.0, 1.0),
        (grass_top[2] + grass_variation * 0.2 + blade * 0.1).clamp(0.0, 1.0),
    ];

    let depth = ly as f32 / TILE_SIZE as f32;
    let dirt = [
        (dirt_base[0] + dirt_variation * 0.5 - depth * 0.12).clamp(0.0, 1.0),
        (dirt_base[1] + dirt_variation * 0.4 - depth * 0.08).clamp(0.0, 1.0),
        (dirt_base[2] + dirt_variation * 0.3 - depth * 0.05).clamp(0.0, 1.0),
    ];

    [
        (dirt[0] * mix + grass[0] * (1.0 - mix)).clamp(0.0, 1.0),
        (dirt[1] * mix + grass[1] * (1.0 - mix)).clamp(0.0, 1.0),
        (dirt[2] * mix + grass[2] * (1.0 - mix)).clamp(0.0, 1.0),
    ]
}
fn dirt_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let base = [0.42, 0.30, 0.18];
    let coarse = fbm_signed(gx, gy, 101) * 0.16;
    let damp = fbm_signed((gx / 2).wrapping_add(211), (gy / 2).wrapping_add(977), 113) * 0.10;
    let clay = fbm_signed(gx.wrapping_add(17), gy.wrapping_add(29), 131) * 0.05;

    let pebble_noise = noise((gx & !1).wrapping_add(lx), (gy & !1).wrapping_add(ly), 149);
    let pebble = if pebble_noise > 0.82 {
        ((noise(gx.wrapping_add(913), gy.wrapping_add(211), 151) - 0.5) * 0.2 + 0.12).max(0.0)
    } else {
        0.0
    };

    [
        (base[0] + coarse * 0.5 + damp * 0.4 + clay * 0.3 + pebble * 0.7).clamp(0.0, 1.0),
        (base[1] + coarse * 0.3 + damp * 0.5 + clay * 0.2 + pebble * 0.4).clamp(0.0, 1.0),
        (base[2] + coarse * 0.2 + damp * 0.4 + clay * 0.2 + pebble * 0.3).clamp(0.0, 1.0),
    ]
}
fn stone_pattern(gx: u32, gy: u32, _lx: u32, _ly: u32) -> [f32; 3] {
    let base = [0.56, 0.56, 0.60];
    let macro_variation = fbm_signed(gx, gy, 193) * 0.18;
    let mid_variation = fbm_signed(gx.wrapping_add(391), gy.wrapping_add(877), 211) * 0.12;
    let fine_variation = (noise(gx.wrapping_add(17), gy.wrapping_add(43), 223) - 0.5) * 0.07;
    let cool_tint = fbm_signed(gx / 4, gy / 4, 229) * 0.05;

    let bright_speckle = if noise(
        (gx & !3).wrapping_add(101),
        (gy & !3).wrapping_add(211),
        239,
    ) > 0.86
    {
        0.08
    } else {
        0.0
    };
    let dark_speckle = if noise(gx.wrapping_add(503), gy.wrapping_add(613), 241) > 0.9 {
        -0.12
    } else {
        0.0
    };

    let variation = macro_variation + mid_variation + fine_variation;
    [
        (base[0] + variation * 0.6 + cool_tint * 0.2 + bright_speckle + dark_speckle)
            .clamp(0.0, 1.0),
        (base[1] + variation * 0.5 + cool_tint * 0.3 + bright_speckle * 0.8 + dark_speckle)
            .clamp(0.0, 1.0),
        (base[2] + variation * 0.6 + cool_tint * 0.6 + bright_speckle * 0.9 + dark_speckle * 0.8)
            .clamp(0.0, 1.0),
    ]
}
fn wood_side_pattern(gx: u32, gy: u32, lx: u32, _ly: u32) -> [f32; 3] {
    let base = [0.60, 0.45, 0.26];
    let stripe_width = (TILE_SIZE / 4).max(1);
    let stripe = ((lx / stripe_width) % 2) as f32;
    let stripe_offset = stripe * 0.12 - 0.06;

    let grain = fbm_signed(gx / 2, gy, 251) * 0.18;
    let ripple = fbm_signed(gx, gy.wrapping_add(401), 263) * 0.08;
    let seam = if ((lx + (gx & 7)) % 7) == 0 {
        -0.06
    } else {
        0.0
    };

    [
        (base[0] + stripe_offset * 0.4 + grain * 0.6 + ripple * 0.4 + seam).clamp(0.0, 1.0),
        (base[1] + stripe_offset * 0.3 + grain * 0.5 + ripple * 0.3 + seam * 0.6).clamp(0.0, 1.0),
        (base[2] + stripe_offset * 0.2 + grain * 0.4 + ripple * 0.2 + seam * 0.5).clamp(0.0, 1.0),
    ]
}
fn wood_top_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let center = (TILE_SIZE as f32 - 1.0) * 0.5;
    let dx = lx as f32 - center;
    let dy = ly as f32 - center;
    let radius = (dx * dx + dy * dy).sqrt() / (TILE_SIZE as f32 * 0.5);
    let ring = (radius * 5.0).fract();
    let inner = [0.63, 0.46, 0.26];
    let outer = [0.50, 0.34, 0.18];
    let t = radius.clamp(0.0, 1.0);

    let mut color = [
        outer[0] + (inner[0] - outer[0]) * (1.0 - t),
        outer[1] + (inner[1] - outer[1]) * (1.0 - t),
        outer[2] + (inner[2] - outer[2]) * (1.0 - t),
    ];

    let ring_profile = (0.5 - (ring - 0.5).abs()).powf(2.5) * 0.24;
    let growth = fbm_signed(gx, gy, 277) * 0.12;
    color[0] += ring_profile + growth * 0.4;
    color[1] += ring_profile * 0.7 + growth * 0.3;
    color[2] += ring_profile * 0.5 + growth * 0.2;

    let radial_noise = fbm_signed(gx / 2, gy / 2, 283) * 0.08;
    color[0] += radial_noise * 0.4;
    color[1] += radial_noise * 0.3;
    color[2] += radial_noise * 0.2;

    [
        color[0].clamp(0.0, 1.0),
        color[1].clamp(0.0, 1.0),
        color[2].clamp(0.0, 1.0),
    ]
}
fn sand_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let base = [0.90, 0.83, 0.63];
    let variation = (noise(gx, gy, 83) - 0.5) * 0.16;
    let dots = if ((lx + ly) & 3) == 0 { 0.08 } else { 0.0 };
    [
        (base[0] + variation + dots * 0.6).clamp(0.0, 1.0),
        (base[1] + variation * 0.6 + dots * 0.4).clamp(0.0, 1.0),
        (base[2] + variation * 0.4 + dots * 0.2).clamp(0.0, 1.0),
    ]
}

fn leaves_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let base = [0.23, 0.55, 0.25];
    let canopy = fbm_signed(gx, gy, 307) * 0.20;
    let sunlit = fbm_signed(gx / 2, gy / 2, 311) * 0.10;
    let sparkle_seed = (lx & 7) ^ ((ly << 1) & 7) ^ ((gx.wrapping_add(gy)) & 7);
    let sparkle = if sparkle_seed == 0 { 0.05 } else { 0.0 };
    let vein = (((lx as i32 - ly as i32).abs() % 5) == 0) as i32 as f32 * 0.08;

    [
        (base[0] + canopy * 0.4 + sunlit * 0.5 + vein * 0.5 + sparkle * 0.4).clamp(0.0, 1.0),
        (base[1] + canopy * 0.3 + sunlit * 0.3 + vein * 0.4 + sparkle * 0.3).clamp(0.0, 1.0),
        (base[2] + canopy * 0.2 + sunlit * 0.2 + vein * 0.3 + sparkle * 0.2).clamp(0.0, 1.0),
    ]
}

fn leaves_alpha(gx: u32, gy: u32, lx: u32, ly: u32) -> f32 {
    let coarse = fbm_signed(gx / 2, gy / 2, 331);
    let fine = fbm_signed(gx, gy, 337);
    let void = noise((gx & !3).wrapping_add(lx), (gy & !3).wrapping_add(ly), 347);

    let mut coverage = 0.68 + coarse * 0.18 + fine * 0.12;
    if void > 0.82 {
        coverage *= 0.3;
    } else if void > 0.74 {
        coverage *= 0.55;
    }

    let cross =
        (((lx as i32 - TILE_SIZE as i32 / 2).abs() + (ly as i32 - TILE_SIZE as i32 / 2).abs()) % 5)
            == 0;
    if cross {
        coverage *= 0.6;
    }

    if coverage < 0.22 {
        0.0
    } else {
        coverage.clamp(0.1, 0.9)
    }
}
fn coal_ore_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let stone = stone_pattern(gx, gy, lx, ly);
    let cluster = fbm_signed(gx / 2, gy / 2, 359);
    let detail = fbm_signed(gx, gy, 367);
    let sparkle = noise(
        gx.wrapping_add(lx.wrapping_mul(17)),
        gy.wrapping_add(ly.wrapping_mul(13)),
        373,
    );

    let mask = 0.55 + cluster * 0.35 + detail * 0.25 + (sparkle - 0.5) * 0.4;
    if mask > 0.6 {
        let mix = ((mask - 0.6) / 0.4).clamp(0.0, 1.0);
        let highlight = ((sparkle - 0.5) * 0.2).clamp(-0.05, 0.08);
        let depth = fbm_signed(gx.wrapping_add(997), gy.wrapping_add(613), 389) * 0.1;
        let ore = [
            (0.10 + depth * 0.2 + highlight).clamp(0.0, 1.0),
            (0.10 + depth * 0.15 + highlight * 0.6).clamp(0.0, 1.0),
            (0.11 + depth * 0.12 + highlight * 0.5).clamp(0.0, 1.0),
        ];
        [
            stone[0] * (1.0 - mix) + ore[0] * mix,
            stone[1] * (1.0 - mix) + ore[1] * mix,
            stone[2] * (1.0 - mix) + ore[2] * mix,
        ]
    } else {
        stone
    }
}
fn iron_ore_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let stone = stone_pattern(gx, gy, lx, ly);
    let cluster = fbm_signed(gx / 2, gy / 2, 401);
    let detail = fbm_signed(gx.wrapping_add(89), gy.wrapping_add(173), 409);
    let sparkle = noise(
        gx.wrapping_add(lx.wrapping_mul(11)).wrapping_add(7),
        gy.wrapping_add(ly.wrapping_mul(13)).wrapping_add(19),
        419,
    );

    let mask = 0.52 + cluster * 0.3 + detail * 0.25 + (sparkle - 0.5) * 0.35;
    if mask > 0.58 {
        let mix = ((mask - 0.58) / 0.42).clamp(0.0, 1.0);
        let warm = 0.32 + detail * 0.12;
        let highlight = ((sparkle - 0.5) * 0.25).clamp(-0.05, 0.12);
        let ore = [
            (warm + highlight).clamp(0.0, 1.0),
            (warm * 0.75 + highlight * 0.6).clamp(0.0, 1.0),
            (warm * 0.55 + highlight * 0.4).clamp(0.0, 1.0),
        ];
        [
            stone[0] * (1.0 - mix) + ore[0] * mix,
            stone[1] * (1.0 - mix) + ore[1] * mix,
            stone[2] * (1.0 - mix) + ore[2] * mix,
        ]
    } else {
        stone
    }
}
fn terracotta_pattern(gx: u32, gy: u32, _lx: u32, ly: u32) -> [f32; 3] {
    let base = [0.78, 0.42, 0.27];
    let stripe = ((ly / (TILE_SIZE / 4).max(1)) % 2) as f32;
    let hue_shift = if stripe > 0.5 { 0.06 } else { -0.04 };
    let variation = (noise(gx, gy, 211) - 0.5) * 0.08;
    [
        (base[0] + hue_shift + variation * 0.6).clamp(0.0, 1.0),
        (base[1] + hue_shift * 0.6 + variation * 0.5).clamp(0.0, 1.0),
        (base[2] + hue_shift * 0.4 + variation * 0.4).clamp(0.0, 1.0),
    ]
}

fn lily_pad_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let base = [0.16, 0.45, 0.23];
    let veins =
        (((lx as i32 - TILE_SIZE as i32 / 2).abs() + (ly as i32 - TILE_SIZE as i32 / 2).abs()) % 5)
            == 0;
    let variation = (noise(gx + 991, gy + 37, 317) - 0.5) * 0.1;
    let highlight = if veins { 0.08 } else { 0.0 };
    [
        (base[0] + variation * 0.6 + highlight * 0.5).clamp(0.0, 1.0),
        (base[1] + variation * 0.5 + highlight * 0.4).clamp(0.0, 1.0),
        (base[2] + variation * 0.3 + highlight * 0.2).clamp(0.0, 1.0),
    ]
}

fn water_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 4] {
    let nx = lx as f32 / (TILE_SIZE as f32 - 1.0);
    let ny = ly as f32 / (TILE_SIZE as f32 - 1.0);
    let ripple = (noise(gx + 277, gy + 911, 311) - 0.5) * 0.12;
    let swirl = (noise(gx + ly * 7 + 37, gy + lx * 13 + 613, 577) - 0.5) * 0.08;
    let sparkle = ((noise(gx * 5 + 193, gy * 5 + 271, 907) - 0.65).max(0.0)) * 0.22;
    let radial = (((nx - 0.5).powi(2) + (ny - 0.5).powi(2)).sqrt() * 1.8).clamp(0.0, 1.0);
    let shallow_mix = (1.0 - radial) * 0.6 + (1.0 - ny) * 0.4;
    let flow = ripple * 0.7 + swirl * 0.3;
    let mut color = [
        0.02 + shallow_mix * 0.08 + flow * 0.05,
        0.28 + shallow_mix * 0.30 + flow * 0.12 + sparkle * 0.12,
        0.62 + shallow_mix * 0.28 + flow * 0.18 + sparkle * 0.25,
    ];
    for c in &mut color {
        *c = c.clamp(0.0, 1.0);
    }
    let alpha = (0.52 + shallow_mix * 0.32 + sparkle * 0.18 + flow * 0.04).clamp(0.45, 0.88);
    [color[0], color[1], color[2], alpha]
}

fn rose_petal_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let center = (TILE_SIZE as f32 - 1.0) * 0.5;
    let dx = lx as f32 - center;
    let dy = ly as f32 - center;
    let dist = (dx * dx + dy * dy).sqrt();
    let radial = (dist / (TILE_SIZE as f32 * 0.55)).clamp(0.0, 1.3);
    let bloom = (1.0 - radial).powf(1.45).clamp(0.0, 1.0);
    let rim = 1.0 - bloom;
    let base = [0.84, 0.1, 0.22];
    let glow = [0.98, 0.62, 0.72];
    let shadow = [0.42, 0.02, 0.08];
    let shimmer = (noise(gx * 19 + lx * 7, gy * 23 + ly * 11, 911) - 0.5) * 0.16;
    let veining = (noise(gx + lx * 5, gy + ly * 7, 613) - 0.5) * 0.12;
    let striation = if ((lx as i32 - ly as i32).abs() % 6) == 0 {
        0.06
    } else {
        0.0
    };
    [
        (base[0] * bloom + shadow[0] * rim + glow[0] * bloom * 0.28 + shimmer * 0.34 + striation)
            .clamp(0.0, 1.0),
        (base[1] * bloom
            + shadow[1] * rim
            + glow[1] * bloom * 0.22
            + shimmer * 0.18
            + veining * 0.25)
            .clamp(0.0, 1.0),
        (base[2] * bloom
            + shadow[2] * rim
            + glow[2] * bloom * 0.42
            + shimmer * 0.26
            + striation * 0.45)
            .clamp(0.0, 1.0),
    ]
}

fn tulip_petal_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let vertical = ly as f32 / (TILE_SIZE as f32 - 1.0);
    let lateral = ((lx as f32 - (TILE_SIZE as f32 - 1.0) * 0.5).abs() / (TILE_SIZE as f32 * 0.45))
        .clamp(0.0, 1.0);
    let base = [0.98, 0.74, 0.34];
    let mid = [0.95, 0.42, 0.44];
    let tip = [0.88, 0.22, 0.68];
    let base_weight = (1.0 - vertical).max(0.0);
    let mid_weight = (vertical * (1.0 - lateral)).max(0.0);
    let tip_weight = (vertical * lateral).max(0.0);
    let sum = (base_weight + mid_weight + tip_weight).max(1e-3);
    let base_w = base_weight / sum;
    let mid_w = mid_weight / sum;
    let tip_w = tip_weight / sum;
    let mut color = [
        base[0] * base_w + mid[0] * mid_w + tip[0] * tip_w,
        base[1] * base_w + mid[1] * mid_w + tip[1] * tip_w,
        base[2] * base_w + mid[2] * mid_w + tip[2] * tip_w,
    ];
    let shimmer = (noise(gx * 17 + lx * 13, gy * 29 + ly * 5, 433) - 0.5) * 0.12;
    let veins = if ((lx + ly * 2 + (gx % 3)) % 7) == 0 {
        0.05
    } else {
        0.0
    };
    color[0] = (color[0] + shimmer * 0.28 + veins * 0.3).clamp(0.0, 1.0);
    color[1] = (color[1] + shimmer * 0.2 + veins * 0.18).clamp(0.0, 1.0);
    color[2] = (color[2] + shimmer * 0.32 + veins * 0.4).clamp(0.0, 1.0);
    color
}

fn flower_stem_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let vertical = ly as f32 / (TILE_SIZE as f32 - 1.0);
    let base = [0.16, 0.42, 0.17];
    let top = [0.34, 0.76, 0.33];
    let mix = vertical.powf(0.85);
    let mut color = [
        base[0] * (1.0 - mix) + top[0] * mix,
        base[1] * (1.0 - mix) + top[1] * mix,
        base[2] * (1.0 - mix) + top[2] * mix,
    ];
    let longitudinal = if ((lx + gy) % 5) == 0 { 0.05 } else { 0.0 };
    let speckle = (noise(gx * 23 + lx * 17, gy * 29 + ly * 11, 521) - 0.5) * 0.12;
    color[0] = (color[0] + speckle * 0.35 + longitudinal * 0.4).clamp(0.0, 1.0);
    color[1] = (color[1] + speckle * 0.28 + longitudinal * 0.2).clamp(0.0, 1.0);
    color[2] = (color[2] + speckle * 0.18 + longitudinal * 0.05).clamp(0.0, 1.0);
    color
}

fn flower_leaf_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let center = (TILE_SIZE as f32 - 1.0) * 0.5;
    let dx = lx as f32 - center;
    let dy = ly as f32 - center;
    let dist = (dx * dx + dy * dy).sqrt() / (TILE_SIZE as f32 * 0.75);
    let mix = (1.0 - dist).clamp(0.0, 1.0);
    let base = [0.18, 0.5, 0.21];
    let tip = [0.44, 0.82, 0.36];
    let mut color = [
        base[0] * (1.0 - mix) + tip[0] * mix,
        base[1] * (1.0 - mix) + tip[1] * mix,
        base[2] * (1.0 - mix) + tip[2] * mix,
    ];
    let vein_primary = if ((lx as i32 + (ly as i32 * 2)) % 7) == 0 {
        0.08
    } else {
        0.0
    };
    let vein_secondary = if (((lx as i32 - TILE_SIZE as i32 / 2).abs()) % 5) == 0 {
        0.05
    } else {
        0.0
    };
    let noise_val = (noise(gx * 17 + lx * 5, gy * 29 + ly * 13, 777) - 0.5) * 0.12;
    color[0] =
        (color[0] + noise_val * 0.28 + vein_primary * 0.4 + vein_secondary * 0.25).clamp(0.0, 1.0);
    color[1] =
        (color[1] + noise_val * 0.24 + vein_primary * 0.25 + vein_secondary * 0.12).clamp(0.0, 1.0);
    color[2] = (color[2] + noise_val * 0.18 + vein_primary * 0.1).clamp(0.0, 1.0);
    color
}

fn glow_shroom_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let cap_base = [0.92, 0.82, 0.28];
    let glow = [0.98, 0.53, 0.18];
    let center = (TILE_SIZE as f32 - 1.0) * 0.5;
    let dx = lx as f32 - center;
    let dy = ly as f32 - center;
    let dist = (dx * dx + dy * dy).sqrt() / (TILE_SIZE as f32 * 0.7);
    let mix = (1.0 - dist).clamp(0.0, 1.0);
    let noise_val = (noise(gx + 613, gy + 457, 223) - 0.5) * 0.12;
    [
        (cap_base[0] * mix + glow[0] * (1.0 - mix) + noise_val).clamp(0.0, 1.0),
        (cap_base[1] * mix + glow[1] * (1.0 - mix) + noise_val * 0.6).clamp(0.0, 1.0),
        (cap_base[2] * mix + glow[2] * (1.0 - mix) + noise_val * 0.4).clamp(0.0, 1.0),
    ]
}

fn snow_pattern(gx: u32, gy: u32, _lx: u32, _ly: u32) -> [f32; 3] {
    let base = [0.93, 0.95, 0.98];
    let variation = (noise(gx + 101, gy + 509, 701) - 0.5) * 0.08;
    [
        (base[0] + variation).clamp(0.0, 1.0),
        (base[1] + variation * 0.8).clamp(0.0, 1.0),
        (base[2] + variation * 0.6).clamp(0.0, 1.0),
    ]
}

fn copper_wire_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let u = (lx as f32 + 0.5) / TILE_SIZE as f32;
    let v = (ly as f32 + 0.5) / TILE_SIZE as f32;
    let radial = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt();
    let sheath = [0.18, 0.14, 0.2];
    let copper = [0.94, 0.6, 0.28];
    let glow = [1.0, 0.78, 0.42];

    let coil = ((u + v) * std::f32::consts::PI * 4.0).sin() * 0.12
        + ((u - v) * std::f32::consts::PI * 3.0).sin() * 0.08;
    let swirl = ((u * v) * 12.0).sin() * 0.06;
    let mut t = (0.45 - radial + coil * 0.15 + swirl * 0.1).clamp(0.0, 1.0);
    t = t.powf(1.4);

    let mut color = [
        sheath[0] * (1.0 - t) + copper[0] * t,
        sheath[1] * (1.0 - t) + copper[1] * t,
        sheath[2] * (1.0 - t) + copper[2] * t,
    ];

    let ridge = ((v - 0.5).abs() * 6.0).powf(1.6).clamp(0.0, 1.0);
    let highlight = ((1.0 - (u - 0.32).abs() * 3.2).clamp(0.0, 1.0)
        * (1.0 - radial * 1.6).clamp(0.0, 1.0))
        * (1.0 - ridge * 0.8);

    if highlight > 0.0 {
        color[0] = color[0] * (1.0 - highlight) + glow[0] * highlight;
        color[1] = color[1] * (1.0 - highlight) + glow[1] * highlight;
        color[2] = color[2] * (1.0 - highlight) + glow[2] * highlight;
    }

    let grain = (noise(gx + 321, gy + 77, lx + ly) - 0.5) * 0.08;
    [
        (color[0] + grain).clamp(0.0, 1.0),
        (color[1] + grain * 0.6).clamp(0.0, 1.0),
        (color[2] + grain * 0.4).clamp(0.0, 1.0),
    ]
}

fn resistor_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let u = (lx as f32 + 0.5) / TILE_SIZE as f32;
    let v = (ly as f32 + 0.5) / TILE_SIZE as f32;

    let top = [0.82, 0.68, 0.5];
    let bottom = [0.58, 0.44, 0.32];
    let curvature = (v - 0.5).abs();
    let shading = (1.0 - curvature.powf(1.8)).clamp(0.0, 1.0);
    let mut color = [
        top[0] * shading + bottom[0] * (1.0 - shading),
        top[1] * shading + bottom[1] * (1.0 - shading),
        top[2] * shading + bottom[2] * (1.0 - shading),
    ];

    let highlight =
        (1.0 - (u - 0.5).abs() * 2.2).clamp(0.0, 1.0) * (1.0 - curvature * 2.6).clamp(0.0, 1.0);
    if highlight > 0.0 {
        let glow = [1.0, 0.93, 0.78];
        let h = highlight * 0.45;
        color[0] = color[0] * (1.0 - h) + glow[0] * h;
        color[1] = color[1] * (1.0 - h) + glow[1] * h;
        color[2] = color[2] * (1.0 - h) + glow[2] * h;
    }

    let stripes = [
        (0.2, [0.74, 0.34, 0.26]),
        (0.36, [0.95, 0.84, 0.32]),
        (0.52, [0.2, 0.26, 0.5]),
        (0.68, [0.16, 0.44, 0.26]),
    ];
    let stripe_width = 0.035;
    for (center, stripe_color) in stripes {
        if (u - center).abs() < stripe_width {
            let mix = 0.7;
            color[0] = color[0] * (1.0 - mix) + stripe_color[0] * mix;
            color[1] = color[1] * (1.0 - mix) + stripe_color[1] * mix;
            color[2] = color[2] * (1.0 - mix) + stripe_color[2] * mix;
        }
    }

    let grain = (noise(gx + 17, gy + 941, lx + ly) - 0.5) * 0.06;
    [
        (color[0] + grain).clamp(0.0, 1.0),
        (color[1] + grain * 0.6).clamp(0.0, 1.0),
        (color[2] + grain * 0.45).clamp(0.0, 1.0),
    ]
}

fn voltage_source_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let u = (lx as f32 + 0.5) / TILE_SIZE as f32;
    let v = (ly as f32 + 0.5) / TILE_SIZE as f32;

    let top_cap = [0.12, 0.14, 0.2];
    let shell_low = [0.17, 0.3, 0.64];
    let shell_high = [0.15, 0.24, 0.52];

    let mut color = if v < 0.12 || v > 0.88 {
        top_cap
    } else {
        let gradient = ((v - 0.12) / 0.76).clamp(0.0, 1.0);
        [
            shell_high[0] * (1.0 - gradient) + shell_low[0] * gradient,
            shell_high[1] * (1.0 - gradient) + shell_low[1] * gradient,
            shell_high[2] * (1.0 - gradient) + shell_low[2] * gradient,
        ]
    };

    if (v - 0.5).abs() < 0.06 {
        let band_mix = (0.06 - (v - 0.5).abs()) / 0.06;
        let band_color = [0.94, 0.95, 0.98];
        color[0] = color[0] * (1.0 - band_mix) + band_color[0] * band_mix;
        color[1] = color[1] * (1.0 - band_mix) + band_color[1] * band_mix;
        color[2] = color[2] * (1.0 - band_mix) + band_color[2] * band_mix;
    }

    if v < 0.18 && u < 0.32 {
        color = [0.14, 0.17, 0.26];
    } else if v > 0.82 && u > 0.68 {
        color = [0.92, 0.3, 0.34];
    }

    let vertical_glow = (1.0 - (u - 0.5).abs() * 2.0).clamp(0.0, 1.0) * 0.25;
    if vertical_glow > 0.0 && v > 0.2 && v < 0.8 {
        let glow = [0.36, 0.5, 0.92];
        color[0] = color[0] * (1.0 - vertical_glow) + glow[0] * vertical_glow;
        color[1] = color[1] * (1.0 - vertical_glow) + glow[1] * vertical_glow;
        color[2] = color[2] * (1.0 - vertical_glow) + glow[2] * vertical_glow;
    }

    let grain = (noise(gx + 73, gy + 403, lx + ly) - 0.5) * 0.05;
    [
        (color[0] + grain).clamp(0.0, 1.0),
        (color[1] + grain * 0.6).clamp(0.0, 1.0),
        (color[2] + grain * 0.4).clamp(0.0, 1.0),
    ]
}

fn ground_pattern(gx: u32, gy: u32, lx: u32, ly: u32) -> [f32; 3] {
    let u = (lx as f32 + 0.5) / TILE_SIZE as f32;
    let v = (ly as f32 + 0.5) / TILE_SIZE as f32;

    let base = [0.14, 0.16, 0.2];
    let rim = [0.22, 0.26, 0.34];
    let glow = [0.94, 0.9, 0.54];

    let radial = ((u - 0.5).powi(2) + (v - 0.4).powi(2)).sqrt();
    let rim_mix = ((0.46 - radial) * 3.0).clamp(0.0, 1.0);
    let mut color = [
        base[0] * (1.0 - rim_mix) + rim[0] * rim_mix,
        base[1] * (1.0 - rim_mix) + rim[1] * rim_mix,
        base[2] * (1.0 - rim_mix) + rim[2] * rim_mix,
    ];

    if (u - 0.5).abs() < 0.07 && v < 0.58 {
        let stem_mix = (0.58 - v) / 0.58 * 0.6;
        color[0] = color[0] * (1.0 - stem_mix) + glow[0] * stem_mix;
        color[1] = color[1] * (1.0 - stem_mix) + glow[1] * stem_mix;
        color[2] = color[2] * (1.0 - stem_mix) + glow[2] * stem_mix;
    }

    let bars = [(0.62, 0.22), (0.72, 0.16), (0.82, 0.11)];
    for (height, width) in bars {
        if (v - height).abs() < 0.015 && (u - 0.5).abs() < width {
            let mix = 0.75;
            color[0] = color[0] * (1.0 - mix) + glow[0] * mix;
            color[1] = color[1] * (1.0 - mix) + glow[1] * mix;
            color[2] = color[2] * (1.0 - mix) + glow[2] * mix;
        }
    }

    let vignette = ((1.0 - ((u - 0.5).abs() + (v - 0.5).abs()) * 0.9).clamp(0.0, 1.0)) * 0.15;
    color[0] *= 1.0 - vignette * 0.5;
    color[1] *= 1.0 - vignette * 0.5;
    color[2] *= 1.0 - vignette * 0.4;

    let grain = (noise(gx + 271, gy + 812, lx + ly) - 0.5) * 0.05;
    [
        (color[0] + grain).clamp(0.0, 1.0),
        (color[1] + grain * 0.6).clamp(0.0, 1.0),
        (color[2] + grain * 0.4).clamp(0.0, 1.0),
    ]
}

fn apply_connection_rim(
    color: &mut [f32; 3],
    lx: u32,
    ly: u32,
    connected: bool,
    accent: [f32; 3],
    shadow: [f32; 3],
) {
    let u = (lx as f32 + 0.5) / TILE_SIZE as f32;
    let v = (ly as f32 + 0.5) / TILE_SIZE as f32;
    let edge = ((u - 0.5).abs().max((v - 0.5).abs())).clamp(0.0, 0.5);
    if connected {
        let rim = ((edge - 0.22) / 0.25).clamp(0.0, 1.0).powf(1.6) * 0.75;
        color[0] = color[0] * (1.0 - rim) + accent[0] * rim;
        color[1] = color[1] * (1.0 - rim) + accent[1] * rim;
        color[2] = color[2] * (1.0 - rim) + accent[2] * rim;
    } else {
        let rim = ((0.46 - edge) / 0.46).clamp(0.0, 1.0).powf(1.4) * 0.65;
        color[0] = color[0] * (1.0 - rim) + shadow[0] * rim;
        color[1] = color[1] * (1.0 - rim) + shadow[1] * rim;
        color[2] = color[2] * (1.0 - rim) + shadow[2] * rim;
    }
}

fn connection_side_pattern(
    gx: u32,
    gy: u32,
    lx: u32,
    ly: u32,
    connected: bool,
    base_low: [f32; 3],
    base_high: [f32; 3],
    accent_on: [f32; 3],
    accent_off: [f32; 3],
    seed: u32,
) -> [f32; 3] {
    let u = (lx as f32 + 0.5) / TILE_SIZE as f32;
    let v = (ly as f32 + 0.5) / TILE_SIZE as f32;
    let mut color = [
        base_low[0] * (1.0 - v) + base_high[0] * v,
        base_low[1] * (1.0 - v) + base_high[1] * v,
        base_low[2] * (1.0 - v) + base_high[2] * v,
    ];

    let center = (1.0 - (u - 0.5).abs() * 3.6).clamp(0.0, 1.0).powf(1.5);
    if connected {
        let mix = center * 0.75;
        color[0] = color[0] * (1.0 - mix) + accent_on[0] * mix;
        color[1] = color[1] * (1.0 - mix) + accent_on[1] * mix;
        color[2] = color[2] * (1.0 - mix) + accent_on[2] * mix;
    } else {
        let mix = center * 0.65;
        color[0] = color[0] * (1.0 - mix) + accent_off[0] * mix;
        color[1] = color[1] * (1.0 - mix) + accent_off[1] * mix;
        color[2] = color[2] * (1.0 - mix) + accent_off[2] * mix;
    }

    let grain = (noise(gx + seed, gy + seed * 17, lx + ly) - 0.5) * 0.05;
    [
        (color[0] + grain).clamp(0.0, 1.0),
        (color[1] + grain * 0.6).clamp(0.0, 1.0),
        (color[2] + grain * 0.45).clamp(0.0, 1.0),
    ]
}

fn copper_wire_connection_top_pattern(
    gx: u32,
    gy: u32,
    lx: u32,
    ly: u32,
    connected: bool,
) -> [f32; 3] {
    let mut color = copper_wire_pattern(gx, gy, lx, ly);
    apply_connection_rim(
        &mut color,
        lx,
        ly,
        connected,
        [1.0, 0.86, 0.46],
        [0.1, 0.08, 0.14],
    );
    color
}

fn copper_wire_connection_side_pattern(
    gx: u32,
    gy: u32,
    lx: u32,
    ly: u32,
    connected: bool,
) -> [f32; 3] {
    connection_side_pattern(
        gx,
        gy,
        lx,
        ly,
        connected,
        [0.16, 0.12, 0.22],
        [0.28, 0.2, 0.32],
        [0.98, 0.76, 0.42],
        [0.08, 0.07, 0.12],
        811,
    )
}

fn resistor_connection_top_pattern(
    gx: u32,
    gy: u32,
    lx: u32,
    ly: u32,
    connected: bool,
) -> [f32; 3] {
    let mut color = resistor_pattern(gx, gy, lx, ly);
    apply_connection_rim(
        &mut color,
        lx,
        ly,
        connected,
        [0.96, 0.84, 0.58],
        [0.32, 0.24, 0.18],
    );
    color
}

fn resistor_connection_side_pattern(
    gx: u32,
    gy: u32,
    lx: u32,
    ly: u32,
    connected: bool,
) -> [f32; 3] {
    connection_side_pattern(
        gx,
        gy,
        lx,
        ly,
        connected,
        [0.42, 0.3, 0.22],
        [0.74, 0.58, 0.46],
        [0.96, 0.86, 0.62],
        [0.26, 0.18, 0.14],
        947,
    )
}

fn voltage_connection_top_pattern(gx: u32, gy: u32, lx: u32, ly: u32, connected: bool) -> [f32; 3] {
    let mut color = voltage_source_pattern(gx, gy, lx, ly);
    apply_connection_rim(
        &mut color,
        lx,
        ly,
        connected,
        [0.42, 0.6, 0.98],
        [0.1, 0.12, 0.18],
    );
    color
}

fn voltage_connection_side_pattern(
    gx: u32,
    gy: u32,
    lx: u32,
    ly: u32,
    connected: bool,
) -> [f32; 3] {
    connection_side_pattern(
        gx,
        gy,
        lx,
        ly,
        connected,
        [0.14, 0.18, 0.26],
        [0.22, 0.34, 0.64],
        [0.56, 0.74, 1.0],
        [0.1, 0.12, 0.18],
        563,
    )
}

fn ground_connection_top_pattern(gx: u32, gy: u32, lx: u32, ly: u32, connected: bool) -> [f32; 3] {
    let mut color = ground_pattern(gx, gy, lx, ly);
    apply_connection_rim(
        &mut color,
        lx,
        ly,
        connected,
        [0.92, 0.9, 0.52],
        [0.08, 0.1, 0.14],
    );
    color
}

fn ground_connection_side_pattern(gx: u32, gy: u32, lx: u32, ly: u32, connected: bool) -> [f32; 3] {
    connection_side_pattern(
        gx,
        gy,
        lx,
        ly,
        connected,
        [0.12, 0.14, 0.18],
        [0.22, 0.26, 0.34],
        [0.84, 0.82, 0.46],
        [0.08, 0.09, 0.12],
        389,
    )
}
