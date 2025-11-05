const MAX_FLUID_LEVEL: u32 = 12u;
const MIN_FLOW: u32 = 1u;
const FLOW_THRESHOLD: i32 = 1;
const MAX_LATERAL_FLOW: u32 = (MAX_FLUID_LEVEL + 2u) / 3u;

struct SimParams {
    grid_width_blocks: u32,
    grid_depth_blocks: u32,
    grid_height: u32,
    _pad: u32,
};

@group(0) @binding(0)
var<storage, read> original_fluids: array<u32>;

@group(0) @binding(1)
var<storage, read> solid_mask: array<u32>;

@group(0) @binding(2)
var<uniform> params: SimParams;

@group(1) @binding(0)
var<storage, read> src_fluids: array<u32>;

@group(1) @binding(1)
var<storage, read_write> dst_fluids: array<u32>;

fn index_of(x: u32, y: u32, z: u32) -> u32 {
    return x + params.grid_width_blocks * (z + params.grid_depth_blocks * y);
}

@compute @workgroup_size(8, 8, 1)
fn vertical_pass(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let z = gid.y;

    if (x >= params.grid_width_blocks || z >= params.grid_depth_blocks) {
        return;
    }

    for (var y: u32 = 0u; y < params.grid_height; y = y + 1u) {
        let idx = index_of(x, y, z);
        let amount = src_fluids[idx];
        var value = amount;
        if (solid_mask[idx] != 0u) {
            value = 0u;
        }
        dst_fluids[idx] = value;
    }

    for (var y: u32 = 0u; y < params.grid_height; y = y + 1u) {
        if (y == 0u) {
            continue;
        }

        let idx = index_of(x, y, z);
        if (solid_mask[idx] != 0u) {
            continue;
        }

        var current = dst_fluids[idx];
        if (current == 0u) {
            continue;
        }

        let below_idx = index_of(x, y - 1u, z);
        if (solid_mask[below_idx] != 0u) {
            continue;
        }

        var below = dst_fluids[below_idx];
        let total = current + below;
        let desired_below = min(total, MAX_FLUID_LEVEL);
        let desired_current = total - desired_below;

        dst_fluids[idx] = desired_current;
        dst_fluids[below_idx] = desired_below;
    }
}

@compute @workgroup_size(8, 8, 1)
fn equalize_x(@builtin(global_invocation_id) gid: vec3<u32>) {
    let y = gid.x;
    let z = gid.y;

    if (y >= params.grid_height || z >= params.grid_depth_blocks) {
        return;
    }

    for (var x: u32 = 0u; x < params.grid_width_blocks; x = x + 1u) {
        let idx = index_of(x, y, z);
        var value = src_fluids[idx];
        if (solid_mask[idx] != 0u) {
            value = 0u;
        }
        dst_fluids[idx] = value;
    }

    for (var x: u32 = 0u; x < params.grid_width_blocks; x = x + 1u) {
        let idx = index_of(x, y, z);
        if (solid_mask[idx] != 0u) {
            continue;
        }

        var current = dst_fluids[idx];
        if (current == 0u || current <= MIN_FLOW) {
            continue;
        }

        if (x > 0u) {
            let neighbor_idx = index_of(x - 1u, y, z);
            if (solid_mask[neighbor_idx] == 0u) {
                var neighbor_amount = dst_fluids[neighbor_idx];
                let diff = i32(current) - i32(neighbor_amount);
                if (diff > FLOW_THRESHOLD) {
                    var moved = diff / 2;
                    if (moved < i32(MIN_FLOW)) {
                        moved = i32(MIN_FLOW);
                    }
                    if (moved > i32(MAX_LATERAL_FLOW)) {
                        moved = i32(MAX_LATERAL_FLOW);
                    }

                    var actual = u32(moved);
                    if (actual > current) {
                        actual = current;
                    }

                    if (actual > 0u) {
                        neighbor_amount = min(neighbor_amount + actual, MAX_FLUID_LEVEL);
                        current = current - actual;
                        dst_fluids[neighbor_idx] = neighbor_amount;
                        dst_fluids[idx] = current;
                    }
                }
            }
        }

        if (current <= MIN_FLOW) {
            continue;
        }

        if (x + 1u < params.grid_width_blocks) {
            let neighbor_idx = index_of(x + 1u, y, z);
            if (solid_mask[neighbor_idx] == 0u) {
                var neighbor_amount = dst_fluids[neighbor_idx];
                let diff = i32(current) - i32(neighbor_amount);
                if (diff > FLOW_THRESHOLD) {
                    var moved = diff / 2;
                    if (moved < i32(MIN_FLOW)) {
                        moved = i32(MIN_FLOW);
                    }
                    if (moved > i32(MAX_LATERAL_FLOW)) {
                        moved = i32(MAX_LATERAL_FLOW);
                    }

                    var actual = u32(moved);
                    if (actual > current) {
                        actual = current;
                    }

                    if (actual > 0u) {
                        neighbor_amount = min(neighbor_amount + actual, MAX_FLUID_LEVEL);
                        current = current - actual;
                        dst_fluids[neighbor_idx] = neighbor_amount;
                        dst_fluids[idx] = current;
                    }
                }
            }
        }
    }
}

@compute @workgroup_size(8, 8, 1)
fn equalize_z(@builtin(global_invocation_id) gid: vec3<u32>) {
    let y = gid.x;
    let x = gid.y;

    if (y >= params.grid_height || x >= params.grid_width_blocks) {
        return;
    }

    for (var z: u32 = 0u; z < params.grid_depth_blocks; z = z + 1u) {
        let idx = index_of(x, y, z);
        var value = src_fluids[idx];
        if (solid_mask[idx] != 0u) {
            value = 0u;
        }
        dst_fluids[idx] = value;
    }

    for (var z: u32 = 0u; z < params.grid_depth_blocks; z = z + 1u) {
        let idx = index_of(x, y, z);
        if (solid_mask[idx] != 0u) {
            continue;
        }

        var current = dst_fluids[idx];
        if (current == 0u || current <= MIN_FLOW) {
            continue;
        }

        if (z > 0u) {
            let neighbor_idx = index_of(x, y, z - 1u);
            if (solid_mask[neighbor_idx] == 0u) {
                var neighbor_amount = dst_fluids[neighbor_idx];
                let diff = i32(current) - i32(neighbor_amount);
                if (diff > FLOW_THRESHOLD) {
                    var moved = diff / 2;
                    if (moved < i32(MIN_FLOW)) {
                        moved = i32(MIN_FLOW);
                    }
                    if (moved > i32(MAX_LATERAL_FLOW)) {
                        moved = i32(MAX_LATERAL_FLOW);
                    }

                    var actual = u32(moved);
                    if (actual > current) {
                        actual = current;
                    }

                    if (actual > 0u) {
                        neighbor_amount = min(neighbor_amount + actual, MAX_FLUID_LEVEL);
                        current = current - actual;
                        dst_fluids[neighbor_idx] = neighbor_amount;
                        dst_fluids[idx] = current;
                    }
                }
            }
        }

        if (current <= MIN_FLOW) {
            continue;
        }

        if (z + 1u < params.grid_depth_blocks) {
            let neighbor_idx = index_of(x, y, z + 1u);
            if (solid_mask[neighbor_idx] == 0u) {
                var neighbor_amount = dst_fluids[neighbor_idx];
                let diff = i32(current) - i32(neighbor_amount);
                if (diff > FLOW_THRESHOLD) {
                    var moved = diff / 2;
                    if (moved < i32(MIN_FLOW)) {
                        moved = i32(MIN_FLOW);
                    }
                    if (moved > i32(MAX_LATERAL_FLOW)) {
                        moved = i32(MAX_LATERAL_FLOW);
                    }

                    var actual = u32(moved);
                    if (actual > current) {
                        actual = current;
                    }

                    if (actual > 0u) {
                        neighbor_amount = min(neighbor_amount + actual, MAX_FLUID_LEVEL);
                        current = current - actual;
                        dst_fluids[neighbor_idx] = neighbor_amount;
                        dst_fluids[idx] = current;
                    }
                }
            }
        }
    }
}