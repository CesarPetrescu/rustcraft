struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) material: f32,
    @location(4) tint: vec3<f32>,
    @location(5) light: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) material: f32,
    @location(3) world_pos: vec3<f32>,
    @location(4) tint: vec3<f32>,
    @location(5) light: f32,
};

@group(1) @binding(0)
var atlas_texture: texture_2d<f32>;

@group(1) @binding(1)
var atlas_sampler: sampler;

struct Environment {
    sky_zenith: vec4<f32>,
    sky_horizon: vec4<f32>,
    fog_color: vec4<f32>,
    camera_position: vec4<f32>,
    fog_params: vec4<f32>,
    time_params: vec4<f32>,
    screen_params: vec4<f32>,
};

@group(2) @binding(0)
var<uniform> environment: Environment;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = camera.view_proj * vec4<f32>(input.position, 1.0);
    output.normal = input.normal;
    output.uv = input.uv;
    output.material = input.material;
    output.world_pos = input.position;
    output.tint = input.tint;
    output.light = input.light;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let albedo = textureSample(atlas_texture, atlas_sampler, input.uv);
    if (albedo.a < 0.01) {
        discard;
    }

    let base = clamp(albedo.rgb * input.tint, vec3<f32>(0.0), vec3<f32>(1.0));
    let normal = normalize(input.normal);
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let daylight = environment.time_params.x;

    // Per-block lighting (0-15 converted to 0.0-1.0)
    let block_light = clamp(input.light / 15.0, 0.0, 1.0);

    // Directional lighting for visual depth
    let directional = clamp(dot(normal, light_dir), 0.0, 1.0) * 0.3;

    // Combine block light with directional shading
    let ambient = environment.fog_params.y;
    let light = (block_light * (0.8 + 0.2 * daylight)) + directional + ambient * 0.2;
    var color = base * clamp(light, 0.0, 1.0);

    var alpha = albedo.a;
    if (input.material < 1.5) {
        if (input.material > 0.5 && albedo.a < 0.4) {
            discard;
        }
        alpha = 1.0;
    } else {
        alpha = clamp(albedo.a * 0.8, 0.0, 1.0);
    }

    let camera_pos = environment.camera_position.xyz;
    let to_camera = camera_pos - input.world_pos;
    let distance = length(to_camera);
    let fog_density = environment.fog_params.x;
    let height_falloff = environment.fog_params.w;
    let height = max(input.world_pos.y - camera_pos.y, 0.0);
    let fog_factor = clamp(1.0 - exp(-distance * fog_density) * exp(-height * height_falloff), 0.0, 1.0);
    color = mix(color, environment.fog_color.rgb, fog_factor);

    let ndc = input.position.xy / input.position.w;
    let uv = ndc * 0.5 + vec2<f32>(0.5, 0.5);
    let offset = uv - vec2<f32>(0.5, 0.5);
    let vignette_strength = environment.fog_params.z;
    let vignette = clamp(1.0 - dot(offset, offset) * 1.6, 0.0, 1.0);
    color *= mix(1.0, vignette, vignette_strength);

    return vec4<f32>(color, alpha);
}
