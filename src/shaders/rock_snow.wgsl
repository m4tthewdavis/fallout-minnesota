// Snow lying on rock and concrete for Fallout: Minnesota. Extends Bevy's
// StandardMaterial: wherever the surface faces up enough to hold snow, the
// stone is buried under a layer that follows the model exactly (no separate
// cap mesh to float). The edge is broken up with noise so it reads as
// drifted powder, the snow smooths away the stone's bumps, and steep faces
// keep a light dusting in their cracks.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::view,
}

struct RockSnow {
    // Snow colour (rgb, linear) and how much snow there is (a: 0..1).
    snow: vec4<f32>,
    // Fine stone grain up close (x: strength).
    detail: vec4<f32>,
}

@group(2) @binding(100) var<uniform> rock: RockSnow;

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q = q + dot(q, q.yxz + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x), mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x), u.y);
    let b = mix(mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x), mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x), u.y);
    return mix(a, b, u.z);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let amount = rock.snow.a;
    let wp = in.world_position.xyz;
    // Double-sided scans: face the geometric normal the same way as the
    // (already flipped) mapped one, or the mix below can cancel to zero.
    let n_geo = normalize(in.world_normal) * select(-1.0, 1.0, is_front);
    let n_map = pbr_input.N;

    // How much this spot faces the sky: mostly the shape, a little the
    // normal map (snow sits in the scan's ledges), broken up with noise.
    let wobble = noise3(wp * 2.3) * 0.6 + noise3(wp * 7.1) * 0.4 - 0.5;
    let up = n_geo.y * 0.7 + n_map.y * 0.3 + wobble * 0.3;
    let threshold = mix(0.95, 0.45, amount);
    let cover = smoothstep(threshold, threshold + 0.14, up) * min(amount * 4.0, 1.0);
    // A thin dusting caught in the cracks of steeper faces.
    let dust = smoothstep(0.1, 0.5, up) * smoothstep(0.55, 0.8, noise3(wp * 11.0)) * 0.35 * amount;
    let s = max(cover, dust);

    var color = pbr_input.material.base_color.rgb;
    // Granite grain: speckles of dark mica and pale feldspar a centimetre or
    // two across, and a little mottling, where the scan has gone soft. Faded
    // out by 12 m, before it could shimmer.
    let near = 1.0 - smoothstep(4.0, 12.0, distance(view.world_position.xyz, wp));
    let g = rock.detail.x * near;
    var n_rock = n_map;
    if g > 0.001 {
        let speck = noise3(wp * 55.0);
        let mottle = noise3(wp * 9.0);
        let grain = 1.0 + g * (0.32 * (speck - 0.5) + 0.18 * (mottle - 0.5));
        color = color * grain;
        // The speckles catch a little relief too.
        let e = 0.02;
        let dx = noise3((wp + vec3<f32>(e, 0.0, 0.0)) * 55.0) - speck;
        let dz = noise3((wp + vec3<f32>(0.0, 0.0, e)) * 55.0) - speck;
        let dy = noise3((wp + vec3<f32>(0.0, e, 0.0)) * 55.0) - speck;
        n_rock = normalize(n_map - vec3<f32>(dx, dy, dz) * 0.6 * g);
    }
    let powder = rock.snow.rgb * (0.94 + 0.08 * noise3(wp * 5.0));
    color = mix(color, powder, s);
    pbr_input.material.base_color = vec4<f32>(color, pbr_input.material.base_color.a);
    pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.85, s);
    pbr_input.material.metallic = mix(pbr_input.material.metallic, 0.0, s);
    // Deep snow hides the stone's bumps and is not darkened by its crevices.
    pbr_input.N = normalize(mix(n_rock, n_geo, cover * 0.85));
    pbr_input.diffuse_occlusion = mix(pbr_input.diffuse_occlusion, vec3<f32>(1.0), cover * 0.7);

    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
