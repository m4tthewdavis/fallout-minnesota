// The sky dome for Fallout: Minnesota. The dome's vertex colours carry the
// scattering model's gradient (painted in sky.rs); this shader lays over it,
// back to front:
//  - the stars (hidden by cloud),
//  - the sun's disc and glare, and the moon,
//  - high cirrus streaks and a deck of drifting cumulus lit by the sun (bright
//    towards it, grey underneath, a silver lining where it shines through),
//  - three ranges of far wooded hills fading into the haze at the horizon.
// Everything is unlit and ignores fog: it *is* what the fog fades into.

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::{view, globals},
}

struct Sky {
    // Direction to the sun (xyz) and how much of its disc shows (w, 0..1).
    sun: vec4<f32>,
    // Colour of whatever lights the clouds: sunlight by day, moonlight by night.
    light: vec4<f32>,
    // Direction the cloud light comes from (xyz); w is unused.
    light_dir: vec4<f32>,
    // Direction to the moon (xyz) and how much of it shows (w).
    moon: vec4<f32>,
    // The sky at the horizon (rgb, what the hills fade into) and how overcast it is (w).
    horizon: vec4<f32>,
    // The sky overhead (rgb) and how much radioactive green is in the air (w).
    zenith: vec4<f32>,
    // Cloud cover 0..1 (x), cirrus amount (y), wind scroll in cloud space (zw).
    cloud: vec4<f32>,
    // Star brightness (x), star-field turn in radians (y), cloud brightness (z),
    // how far the hills are lost in haze 0..1 (w).
    misc: vec4<f32>,
}

@group(2) @binding(0) var<uniform> sky: Sky;
@group(2) @binding(1) var stars_texture: texture_2d<f32>;
@group(2) @binding(2) var stars_sampler: sampler;

const PI: f32 = 3.14159265;

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

fn fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var v = 0.0;
    var amp = 0.5;
    var q = p;
    var norm = 0.0;
    for (var i = 0; i < octaves; i = i + 1) {
        v = v + amp * value_noise(q);
        norm = norm + amp;
        // Turn each octave so the lattice never lines up.
        q = vec2<f32>(q.x * 1.62 - q.y * 1.21, q.x * 1.21 + q.y * 1.62) + vec2<f32>(1.7, 9.2);
        amp = amp * 0.5;
    }
    return v / norm;
}

// Rounded bumps with sharp creases between them: the cauliflower tops of cumulus.
fn billow(p: vec2<f32>, octaves: i32) -> f32 {
    var v = 0.0;
    var amp = 0.5;
    var q = p;
    var norm = 0.0;
    for (var i = 0; i < octaves; i = i + 1) {
        v = v + amp * abs(2.0 * value_noise(q) - 1.0);
        norm = norm + amp;
        q = vec2<f32>(q.x * 1.62 - q.y * 1.21, q.x * 1.21 + q.y * 1.62) + vec2<f32>(5.3, 2.8);
        amp = amp * 0.55;
    }
    return v / max(norm, 1e-3);
}

// The cumulus deck at a point in cloud space: how far inside a cloud the
// point is (negative in clear sky, about 0.1 at a soft edge, 0.3 and more deep
// in a big one). `detail` octaves of billows crinkle the edges (0 for a
// cheap, smooth answer).
fn cumulus(uv: vec2<f32>, cover: f32, detail: i32) -> f32 {
    var n = fbm(uv, 3);
    if (detail > 0) {
        n = n + (billow(uv * 3.7 + vec2<f32>(3.1, 8.4), detail) - 0.42) * 0.22;
    }
    // Big slow patches open and close the deck, so cover is never even.
    let patches = value_noise(uv * 0.16 + vec2<f32>(31.0, 7.0));
    let c = clamp(cover + (patches - 0.5) * 0.55 * (1.0 - cover), 0.0, 1.0);
    let lo = 0.5 + (0.5 - c) * 0.34;
    return n - lo;
}

// One range of hills: how far the ridge line rises above the horizon (as the
// tangent of the angle) along the compass direction `around` (unit, xz).
fn ridge(around: vec2<f32>, freq: f32, seed: f32, base: f32, amp: f32) -> f32 {
    let p = around * freq + vec2<f32>(seed, seed * 1.7);
    let rolling = fbm(p, 4);
    return base + amp * rolling;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let d = normalize(in.world_position.xyz - view.world_position.xyz);
#ifdef VERTEX_COLORS
    var col = in.color.rgb;
#else
    var col = sky.zenith.rgb;
#endif
    let up = max(d.y, 0.0);
    let overcast = sky.horizon.w;
    let haze_col = sky.horizon.rgb;
    let light = sky.light.rgb;
    let light_dir = normalize(sky.light_dir.xyz);
    let sun_dir = normalize(sky.sun.xyz);
    let ambient = mix(sky.zenith.rgb, haze_col, 0.5);

    // ---- Clouds: where and how thick (needed first: they hide what's behind) ----
    // The deck is a flat sheet overhead; the small term keeps it from
    // stretching to infinity at the horizon.
    let plane = d.xz / (up + 0.11);
    let scroll = sky.cloud.zw;
    var uv = plane * 0.8 + scroll;
    let warp = vec2<f32>(fbm(uv * 0.55 + vec2<f32>(7.3, 1.1), 3), fbm(uv * 0.55 + vec2<f32>(19.1, 4.7), 3));
    uv = uv + (warp - 0.5) * 0.35;
    let cover = sky.cloud.x;
    let depth = cumulus(uv, cover, 4);
    let dens = smoothstep(0.0, 0.10, depth);
    // Fade the deck out into the horizon haze.
    let low_fade = smoothstep(0.0, 0.11, up);
    let cloud_a = dens * low_fade * step(0.0, d.y);

    // Thin high cirrus, combed out along the wind.
    let cuv = vec2<f32>(plane.x * 0.9659 - plane.y * 0.2588, plane.x * 0.2588 + plane.y * 0.9659) * vec2<f32>(0.035, 0.16) + scroll * 0.3 + vec2<f32>(4.0, 9.0);
    let streak = fbm(cuv + (warp - 0.5) * 0.25, 5);
    let cirrus_a = smoothstep(0.50, 0.82, streak) * sky.cloud.y * low_fade * (1.0 - overcast) * step(0.0, d.y);

    let hidden = clamp(cloud_a + cirrus_a * 0.35, 0.0, 1.0);

    // ---- Stars ----
    if (sky.misc.x > 0.001 && d.y > -0.02) {
        // Wheel the field slowly round a tilted pole.
        let a = sky.misc.y;
        let tilt = vec3<f32>(d.x, d.y * 0.9394 - d.z * 0.3429, d.y * 0.3429 + d.z * 0.9394);
        let r = vec3<f32>(tilt.x * cos(a) + tilt.z * sin(a), tilt.y, -tilt.x * sin(a) + tilt.z * cos(a));
        let suv = vec2<f32>(atan2(r.z, r.x) / (2.0 * PI) + 0.5, acos(clamp(r.y, -1.0, 1.0)) / PI);
        let s = textureSampleLevel(stars_texture, stars_sampler, suv, 0.0);
        // Stars dim into the thicker air near the horizon.
        col = col + s.rgb * s.a * sky.misc.x * (1.0 - hidden) * smoothstep(0.0, 0.25, up + 0.02);
    }

    // ---- Sun: a small hot disc and the glare round it ----
    let sun_ang = length(cross(d, sun_dir));
    let facing_sun = step(0.0, dot(d, sun_dir));
    let sun_col = light / max(max(light.r, max(light.g, light.b)), 1e-3);
    let through = (1.0 - cloud_a) * (1.0 - cloud_a);
    let disc = (1.0 - smoothstep(0.0105, 0.0135, sun_ang)) * facing_sun;
    let glare = (exp(-sun_ang * 22.0) * 0.9 + exp(-sun_ang * 6.0) * 0.26 + exp(-sun_ang * 2.2) * 0.10) * facing_sun;
    col = col + sun_col * sky.sun.w * (disc * 30.0 * through * through + glare * mix(0.35, 1.0, through)) * step(-0.01, d.y);

    // ---- Moon ----
    let moon_dir = normalize(sky.moon.xyz);
    let moon_ang = length(cross(d, moon_dir));
    let facing_moon = step(0.0, dot(d, moon_dir));
    let moon_disc = (1.0 - smoothstep(0.0150, 0.0165, moon_ang)) * facing_moon;
    if (sky.moon.w > 0.001) {
        // Grey seas on a bright face.
        let seas = fbm((d.xz - moon_dir.xz) * 90.0 + vec2<f32>(3.0, 8.0), 3);
        let face = vec3<f32>(2.0, 2.1, 2.4) * (0.72 + 0.4 * smoothstep(0.35, 0.65, seas));
        let moon_glow = exp(-moon_ang * 16.0) * 0.10 * facing_moon;
        col = mix(col, face, moon_disc * sky.moon.w * through) + vec3<f32>(0.55, 0.65, 0.9) * moon_glow * sky.moon.w * mix(0.4, 1.0, through);
    }

    // ---- Paint the clouds ----
    // How much cloud lies between this point and the light: a little way
    // towards the sun, and a longer way. The side of a cloud facing the sun
    // is bright; the far side and the thick middle are in its own shade.
    let to_light = normalize(light_dir.xz / (max(light_dir.y, 0.0) + 0.11) - plane + vec2<f32>(1e-4, 0.0));
    let near_light = max(cumulus(uv + to_light * 0.07, cover, 2), 0.0);
    let far_light = max(cumulus(uv + to_light * 0.22, cover, 0), 0.0);
    let thickness = max(depth, 0.0) * 0.9 + near_light * 1.6 + far_light * 1.3;
    let lit = exp(-thickness * 3.4);
    let bright = sky.misc.z;
    let mu = max(dot(d, light_dir), 0.0);
    // Sunlit white, and a base the colour of the sky's own light on it.
    let lit_col = (light * 1.30 + ambient * 0.50) * bright;
    let dark_col = (ambient * 0.95 + light * 0.34) * bright;
    var cloud_col = mix(dark_col, lit_col, lit);
    // Silver lining: thin edges glow when the light is behind them.
    cloud_col = cloud_col + light * bright * pow(mu, 10.0) * (1.0 - dens * 0.8) * 1.6 * lit;
    // A storm deck is one flat grey sheet, with only a little shape left.
    let sheet = haze_col * (0.74 + 0.22 * lit);
    cloud_col = mix(cloud_col, sheet, overcast * 0.85);
    // Far clouds sink into the haze.
    let cloud_haze = exp(-up * 7.0);
    cloud_col = mix(cloud_col, haze_col, cloud_haze * 0.85);

    let cirrus_col = mix((light * 1.0 + ambient * 0.8) * bright + light * bright * pow(mu, 6.0), haze_col, cloud_haze * 0.9);
    col = mix(col, cirrus_col, cirrus_a * 0.55);
    col = mix(col, cloud_col, cloud_a);

    // ---- Far hills: three ranges, each paler and bluer than the one in front ----
    let flat_len = max(length(d.xz), 1e-4);
    let around = d.xz / flat_len;
    let rise = d.y / flat_len;
    // Lower towards the north, where the big lake lies open to the horizon.
    let north = smoothstep(0.35, 0.95, -around.y);
    let lift = mix(1.0, 0.28, north);
    let lost = sky.misc.w;
    let sunward = 0.5 + 0.5 * dot(around, normalize(light_dir.xz + vec2<f32>(1e-4, 0.0)));
    // Dark spruce with snow in it, lit by the sky and a little by the sun.
    let wood = vec3<f32>(0.17, 0.21, 0.24) * (ambient * 1.5 + light * (0.25 + 0.35 * (1.0 - sunward)));
    for (var i = 0; i < 3; i = i + 1) {
        let fi = f32(i);
        // Far to near: taller and hazier at the back.
        let freq = 2.2 + fi * 2.6;
        let base = (0.030 - fi * 0.009) * lift;
        let amp = (0.050 - fi * 0.012) * lift;
        var top = ridge(around, freq, 11.0 + fi * 23.0, base, amp);
        // Treetops along the nearest ridge.
        top = top + (value_noise(around * (140.0 + fi * 60.0)) - 0.5) * 0.0022 * fi;
        let fog = clamp(mix(0.90, 0.60, fi / 2.0) + lost, 0.0, 1.0);
        // Mist pools at the foot of each range.
        let foot = smoothstep(top, top - 0.05, rise) * 0.35;
        let ridge_col = mix(wood, haze_col, clamp(fog + foot * (1.0 - fog), 0.0, 1.0));
        let edge = fwidth(rise) * 1.2 + 1e-5;
        col = mix(col, ridge_col, smoothstep(top + edge, top - edge, rise));
    }
    // Below the horizon: haze over far snow.
    col = mix(col, haze_col * 0.96, smoothstep(0.0, -0.02, rise));

    var out: FragmentOutput;
    out.color = vec4<f32>(max(col, vec3<f32>(0.0)), 1.0);
    return out;
}
