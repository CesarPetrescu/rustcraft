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
    var sky = mix(environment.sky_horizon.rgb, environment.sky_zenith.rgb, t);

    // time_params: [daylight, sun_elevation, twilight, time_of_day]
    let time_of_day = environment.time_params.w;
    let daylight = environment.time_params.x;

    // Stars visibility (fade in at dusk, fade out at dawn)
    // Night is roughly 0.0-0.25 and 0.75-1.0
    var star_visibility = 0.0;
    if (time_of_day < 0.25) {
        star_visibility = (0.25 - time_of_day) / 0.25;
    } else if (time_of_day > 0.75) {
        star_visibility = (time_of_day - 0.75) / 0.25;
    }
    star_visibility = clamp(star_visibility * (1.0 - daylight), 0.0, 1.0);

    // Generate stars using noise-like pattern
    if (star_visibility > 0.01 && input.uv.y > 0.3) {
        let star_coord = input.uv * environment.screen_params.xy * 0.5;
        let star_hash = fract(sin(dot(floor(star_coord), vec2<f32>(12.9898, 78.233))) * 43758.5453);

        if (star_hash > 0.998) {
            let local_pos = fract(star_coord);
            let dist = length(local_pos - vec2<f32>(0.5, 0.5));
            let star_intensity = (1.0 - smoothstep(0.0, 0.15, dist)) * star_visibility;

            // Twinkling effect (use time_of_day for animation)
            let twinkle = 0.7 + 0.3 * sin(time_of_day * 50.0 + star_hash * 6.28);
            sky += vec3<f32>(star_intensity * twinkle);
        }
    }

    // Moon rendering
    if (star_visibility > 0.01) {
        // Moon position based on time (opposite to sun)
        let moon_angle = (time_of_day + 0.5) * 3.14159 * 2.0;
        let moon_x = 0.5 + cos(moon_angle) * 0.35;
        let moon_y = 0.5 + sin(moon_angle) * 0.35;

        // Only render moon when it's above horizon
        if (moon_y > 0.3) {
            let moon_center = vec2<f32>(moon_x, moon_y);
            let moon_dist = length(input.uv - moon_center);
            let moon_radius = 0.04;

            if (moon_dist < moon_radius) {
                let moon_intensity = (1.0 - smoothstep(moon_radius * 0.7, moon_radius, moon_dist));
                let moon_color = vec3<f32>(0.95, 0.95, 0.85) * moon_intensity * star_visibility;
                sky = mix(sky, moon_color, moon_intensity * star_visibility);
            }
        }
    }

    return vec4<f32>(sky, 1.0);
}
