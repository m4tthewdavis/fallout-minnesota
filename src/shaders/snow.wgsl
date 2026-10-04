// Snow ground for Fallout: Minnesota. Extends Bevy's StandardMaterial with:
//  - powder, wind-packed and icy-crust areas (colour, roughness, ripples),
//  - wind ripples (sastrugi) and scattered pits bent into the normal,
//  - a second, larger sample of the snow texture to hide the 4 m tiling,
//  - sun glints that sparkle as you move.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    pbr_bindings,
    mesh_view_bindings::view,
}

struct Snow {
    // Direction towards the sun (xyz) and how bright it is (w, 0 at night).
    sun: vec4<f32>,
    // Wind direction in xz (xy), ripple strength (z), sparkle strength (w).
    wind: vec4<f32>,
}

@group(2) @binding(100) var<uniform> snow: Snow;

fn hash2(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(123.34, 456.21));
    q = q + dot(q, q + 45.32);
    return fract(q.x * q.y);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < 4; i = i + 1) {
        v = v + amp * value_noise(q);
        q = q * 2.03 + vec2<f32>(1.7, 9.2);
        amp = amp * 0.5;
    }
    return v;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let wp = in.world_position.xz;
    let dist = distance(view.world_position.xyz, in.world_position.xyz);
    let N0 = pbr_input.N;
    let flat_ground = smoothstep(0.75, 0.95, N0.y);

    // ---- Kinds of snow ----
    let packed = smoothstep(0.42, 0.62, fbm(wp * 0.045 + vec2<f32>(11.0, 3.0)));
    let icy = smoothstep(0.64, 0.74, fbm(wp * 0.028 + vec2<f32>(37.0, 71.0))) * flat_ground;

    // ---- Wind ripples: asymmetric waves across the wind, warped by noise ----
    let wind = normalize(snow.wind.xy + vec2<f32>(1e-4, 0.0));
    let warp = fbm(wp * 0.12) * 3.0;
    let wavelength = mix(0.45, 0.9, value_noise(wp * 0.05 + vec2<f32>(5.0, 1.0)));
    let k = 6.2831853 / wavelength;
    let u = dot(wp, wind) + warp;
    let ripple_h = sin(u * k) + 0.35 * sin(2.0 * u * k + 1.3);
    let ripple_d = k * (cos(u * k) + 0.7 * cos(2.0 * u * k + 1.3));
    let far = 1.0 - smoothstep(35.0, 80.0, dist);
    let amp = snow.wind.z * (0.25 + 0.75 * packed) * (1.0 - icy) * flat_ground * far * 0.025;
    var grad = wind * ripple_d * amp;

    // ---- Scattered pits (old tracks, dropped clumps) ----
    let cell_p = wp * 1.1;
    let cell = floor(cell_p);
    let f = fract(cell_p) - vec2<f32>(hash2(cell + 3.0), hash2(cell + 7.0)) * 0.6 - 0.2;
    let pit = step(0.9, hash2(cell + 13.0)) * (1.0 - icy) * far;
    // A shallow bowl 5 cm deep and 16 cm across.
    let r = 0.16;
    let inside = step(dot(f, f), r * r);
    grad = grad + f * (2.0 * 0.05 / (r * r)) * pit * inside;

    pbr_input.N = normalize(N0 - vec3<f32>(grad.x, 0.0, grad.y));

    // ---- Colour ----
    var color = pbr_input.material.base_color.rgb;
    // A second, much larger sample of the snow texture hides the tiling.
    let big = textureSample(pbr_bindings::base_color_texture, pbr_bindings::base_color_sampler, in.uv * 0.21 + vec2<f32>(0.37, 0.71)).rgb;
    color = color * mix(1.0, dot(big, vec3<f32>(0.3333)) / 0.9, 0.35);
    color = color * (0.95 + 0.07 * fbm(wp * 0.012 + vec2<f32>(2.0, 8.0)));
    // Packed snow is a touch greyer; ripple crests catch a little more light.
    color = color * mix(1.0, 0.95, packed) * (1.0 + 2.5 * ripple_h * amp);
    // Icy crust: bluer and darker, like old glazed snow.
    color = mix(color, color * vec3<f32>(0.78, 0.87, 0.98), icy * 0.85);
    pbr_input.material.base_color = vec4<f32>(color, pbr_input.material.base_color.a);

    var rough = pbr_input.material.perceptual_roughness;
    rough = rough * mix(1.0, 0.82, packed);
    rough = mix(rough, 0.22, icy);
    pbr_input.material.perceptual_roughness = rough;

    // ---- Glints: tiny crystal facets catching the sun ----
    let sp = wp * 20.0;
    let sid = floor(sp);
    let rare = step(0.9, hash2(sid + 3.1));
    let facet = normalize(N0 + vec3<f32>(hash2(sid) - 0.5, 0.0, hash2(sid + 17.0) - 0.5) * 1.6);
    let refl = reflect(-pbr_input.V, facet);
    let spec = pow(max(dot(refl, normalize(snow.sun.xyz)), 0.0), 40.0);
    let dot_shape = 1.0 - smoothstep(0.12, 0.3, length(fract(sp) - vec2<f32>(0.5)));
    let near = 1.0 - smoothstep(18.0, 40.0, dist);
    let glint = spec * dot_shape * rare * near * snow.wind.w * snow.sun.w * (1.0 - 0.6 * icy);
    pbr_input.material.emissive = pbr_input.material.emissive + vec4<f32>(vec3<f32>(5.0, 5.3, 6.0) * glint, 0.0);

    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
