struct Environment {
    sky_zenith: vec4<f32>,
    sky_horizon: vec4<f32>,
    fog_color: vec4<f32>,
    camera_position: vec4<f32>,
    fog_params: vec4<f32>,
    time_params: vec4<f32>,
    screen_params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> environment: Environment;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );

    let pos = positions[vertex_index];
    var output: VertexOutput;
    output.position = vec4<f32>(pos, 0.0, 1.0);
    output.uv = vec2<f32>(0.5 * (pos.x + 1.0), 0.5 * (pos.y + 1.0));
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let t = clamp(input.uv.y, 0.0, 1.0);
    let sky = mix(environment.sky_horizon.rgb, environment.sky_zenith.rgb, t);
    return vec4<f32>(sky, 1.0);
}
