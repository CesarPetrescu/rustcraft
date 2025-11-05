@group(0) @binding(0)
var atlas_texture: texture_2d<f32>;

@group(0) @binding(1)
var atlas_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) mode: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) mode: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.color = input.color;
    output.uv = input.uv;
    output.mode = input.mode;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if (input.mode > 0.5) {
        let sample = textureSample(atlas_texture, atlas_sampler, input.uv);
        return vec4<f32>(sample.rgb * input.color.rgb, sample.a * input.color.a);
    }
    return input.color;
}
