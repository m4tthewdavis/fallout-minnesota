//! Mipmap chains for RGBA8 textures.
//!
//! Bevy uploads PNG/JPEG textures with a single mip level, so tiled textures
//! (snow, bark, rust) shimmer and sparkle in the distance. This builds the
//! smaller levels with a 2x2 box filter, averaging colour textures in linear
//! light so they don't darken.

use super::mathx;

/// Number of levels down to 1x1.
pub fn level_count(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

fn to_linear_table() -> [f32; 256] {
    let mut t = [0.0; 256];
    for (i, v) in t.iter_mut().enumerate() {
        let c = i as f32 / 255.0;
        *v = if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    }
    t
}

/// The exact conversion: linear light to an sRGB byte. Slow (a `powf`), so
/// the hot loops use [`SrgbTable`], which gives the same byte for every input.
fn to_srgb(l: f32) -> u8 {
    let c = if l <= 0.0031308 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 };
    (c * 255.0).round().clamp(0.0, 255.0) as u8
}

/// `to_srgb` without the `powf`. `to_srgb` only ever changes value at 255
/// points on the number line; this finds each of them once (by bisecting the
/// original function over the bit patterns of `f32`, which are ordered like
/// the numbers) and then looks a value up among them. A 4096-entry index
/// narrows the search to at most two edges, which are counted without a
/// branch (the answers of neighbouring texels are not predictable).
struct SrgbTable {
    /// `edge[k]` is the smallest `l` for which `to_srgb(l) > k`; one infinite
    /// sentinel on the end so the lookup never needs a bounds test.
    edge: [f32; 256],
    /// How many edges lie at or below `i / BUCKETS`.
    first: [u8; BUCKETS + 1],
}

const BUCKETS: usize = 4096;

impl SrgbTable {
    fn build() -> SrgbTable {
        let mut edge = [f32::INFINITY; 256];
        for (k, e) in edge.iter_mut().take(255).enumerate() {
            // Bisect for the first bit pattern whose byte is above k. The byte
            // never reaches 256, and 1.0 (bits 0x3F80_0000) gives 255.
            let (mut lo, mut hi) = (0u32, 0x3F80_0000u32);
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if to_srgb(f32::from_bits(mid)) as usize > k {
                    hi = mid;
                } else {
                    lo = mid + 1;
                }
            }
            *e = f32::from_bits(lo);
        }
        let mut first = [0u8; BUCKETS + 1];
        for (i, f) in first.iter_mut().enumerate() {
            let at = i as f32 / BUCKETS as f32;
            *f = edge[..255].partition_point(|&e| e <= at) as u8;
        }
        // The lookup below steps over at most two edges inside one bucket.
        for w in first.windows(2) {
            assert!(w[1] - w[0] <= 2, "edges too dense for the lookup");
        }
        SrgbTable { edge, first }
    }

    fn get() -> &'static SrgbTable {
        static TABLE: std::sync::OnceLock<SrgbTable> = std::sync::OnceLock::new();
        TABLE.get_or_init(SrgbTable::build)
    }

    /// Same result as `to_srgb(l)` for every `f32`.
    #[inline]
    fn byte(&self, l: f32) -> u8 {
        // Negative, zero and NaN are all black; 1 and above are all white.
        if l.is_nan() || l <= 0.0 {
            return 0;
        }
        if l >= 1.0 {
            return 255;
        }
        let mut k = self.first[(l * BUCKETS as f32) as usize] as usize;
        k += (self.edge[k] <= l) as usize;
        k += (self.edge[k] <= l) as usize;
        k as u8
    }
}

/// Drains the colour out of an sRGB RGBA8 texture, in place, keeping `keep`
/// of its saturation (0 = grey), then multiplies by `tint` in linear light.
/// Brightness (linear luminance) is kept before the tint, so the scan's
/// detail survives. Used to turn warm scanned sandstone into cold granite:
/// a plain multiply can't do that, it only darkens the orange.
pub fn recolour(rgba: &mut [u8], keep: f32, tint: [f32; 3]) {
    let lin = to_linear_table();
    let srgb = SrgbTable::get();
    for px in rgba.as_chunks_mut::<4>().0 {
        let c = [lin[px[0] as usize], lin[px[1] as usize], lin[px[2] as usize]];
        let y = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        for k in 0..3 {
            px[k] = srgb.byte((y + (c[k] - y) * keep) * tint[k]);
        }
    }
}

/// Returns every level, largest first, concatenated (the layout wgpu expects
/// for one layer), plus the level count. `rgba` is `width * height * 4` bytes.
pub fn build_chain(width: u32, height: u32, rgba: &[u8], srgb: bool) -> (Vec<u8>, u32) {
    let (w0, h0) = (width as usize, height as usize);
    // Anything that isn't a well-formed picture takes the plain route (which
    // keeps its old behaviour for empty or ragged input).
    if w0 == 0 || h0 == 0 || rgba.len() != w0 * h0 * 4 {
        return build_chain_reference(width, height, rgba, srgb);
    }
    let levels = level_count(width, height);
    let lin = to_linear_table();
    let to_byte = SrgbTable::get();
    let unit: [f32; 256] = std::array::from_fn(|v| v as f32 / 255.0);
    // What a colour byte is worth when averaged: linear light for sRGB, itself otherwise.
    let rgb_light = if srgb { &lin } else { &unit };
    // The whole chain is one allocation, filled level by level.
    let (mut total, mut tw, mut th) = (w0 * h0 * 4, w0, h0);
    for _ in 1..levels {
        tw = (tw / 2).max(1);
        th = (th / 2).max(1);
        total += tw * th * 4;
    }
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(rgba);
    let (mut w, mut h) = (w0, h0);
    let mut prev_start = 0;
    for _ in 1..levels {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let start = out.len();
        out.resize(start + nw * nh * 4, 0);
        let (done, level) = out.split_at_mut(start);
        let prev = &done[prev_start..];
        for y in 0..nh {
            let (r0, r1) = (y * 2 * w, ((y * 2 + 1).min(h - 1)) * w);
            for x in 0..nw {
                let (c0, c1) = (x * 2, (x * 2 + 1).min(w - 1));
                // The four source pixels, in the order they are summed.
                let taps = [(r0 + c0) * 4, (r0 + c1) * 4, (r1 + c0) * 4, (r1 + c1) * 4];
                let dst = (y * nw + x) * 4;
                let mut acc = [0.0f32; 4];
                for i in taps {
                    let px = &prev[i..i + 4];
                    acc[0] += rgb_light[px[0] as usize];
                    acc[1] += rgb_light[px[1] as usize];
                    acc[2] += rgb_light[px[2] as usize];
                    acc[3] += unit[px[3] as usize];
                }
                for c in 0..3 {
                    let avg = acc[c] / 4.0;
                    level[dst + c] = if srgb { to_byte.byte(avg) } else { mathx::round_u8(avg * 255.0) };
                }
                level[dst + 3] = mathx::round_u8(acc[3] / 4.0 * 255.0);
            }
        }
        prev_start = start;
        w = nw;
        h = nh;
    }
    (out, levels)
}

/// The original, plain `build_chain`: the fallback for degenerate input, and
/// the reference the fast one is tested against (byte for byte).
fn build_chain_reference(width: u32, height: u32, rgba: &[u8], srgb: bool) -> (Vec<u8>, u32) {
    let levels = level_count(width, height);
    let lin = to_linear_table();
    let mut out = rgba.to_vec();
    let (mut w, mut h) = (width as usize, height as usize);
    let mut prev_start = 0;
    for _ in 1..levels {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let start = out.len();
        out.reserve(nw * nh * 4);
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0.0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let sx = (x * 2 + dx).min(w - 1);
                    let sy = (y * 2 + dy).min(h - 1);
                    let i = prev_start + (sy * w + sx) * 4;
                    for c in 0..4 {
                        let v = out[i + c];
                        acc[c] += if srgb && c < 3 { lin[v as usize] } else { v as f32 / 255.0 };
                    }
                }
                for (c, a) in acc.iter().enumerate() {
                    let avg = a / 4.0;
                    out.push(if srgb && c < 3 {
                        to_srgb(avg)
                    } else {
                        (avg * 255.0).round() as u8
                    });
                }
            }
        }
        prev_start = start;
        w = nw;
        h = nh;
    }
    (out, levels)
}

/// Alpha-tested foliage (needle sprays, twigs, grass) thins out and vanishes
/// in the distance because averaging thin strokes drops their alpha below the
/// cut-off. Rescale each smaller level's alpha so the same share of texels
/// passes `cutoff` as in the full-size level (Castano's coverage trick).
pub fn preserve_coverage(chain: &mut [u8], width: u32, height: u32, levels: u32, cutoff: f32) {
    // How many alpha values pass the cut-off after scaling. Alpha is a byte,
    // so a 256-bin count of a level answers this for any scale without
    // scanning the pixels again (the binary search asks 12 times a level).
    let coverage = |hist: &[u32; 256], n: usize, scale: f32| -> f32 {
        let pass: u32 = (0..256).filter(|&a| a as f32 / 255.0 * scale > cutoff).map(|a| hist[a]).sum();
        pass as f32 / n.max(1) as f32
    };
    let histogram = |px: &[u8]| -> ([u32; 256], usize) {
        let mut hist = [0u32; 256];
        for p in px.as_chunks::<4>().0 {
            hist[p[3] as usize] += 1;
        }
        (hist, px.len() / 4)
    };
    let (mut w, mut h) = (width as usize, height as usize);
    let mut offset = 0;
    let mut target = None;
    for _ in 0..levels {
        let len = w * h * 4;
        if offset + len > chain.len() {
            break;
        }
        let level = &mut chain[offset..offset + len];
        let (hist, n) = histogram(level);
        match target {
            None => target = Some(coverage(&hist, n, 1.0)),
            Some(t) => {
                // Binary search the alpha scale that restores the coverage.
                let (mut lo, mut hi) = (0.5f32, 8.0f32);
                for _ in 0..12 {
                    let mid = (lo + hi) * 0.5;
                    if coverage(&hist, n, mid) < t {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                // Err towards keeping the foliage rather than losing it.
                let scale = hi;
                // Only 256 distinct alphas exist: scale each once.
                let scaled: [u8; 256] = std::array::from_fn(|a| (a as f32 * scale).round().min(255.0) as u8);
                for p in level.as_chunks_mut::<4>().0 {
                    p[3] = scaled[p[3] as usize];
                }
            }
        }
        offset += len;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
}

#[cfg(test)]
fn preserve_coverage_reference(chain: &mut [u8], width: u32, height: u32, levels: u32, cutoff: f32) {
    let coverage = |px: &[u8], scale: f32| -> f32 {
        let n = px.len() / 4;
        let pass = px.as_chunks::<4>().0.iter().filter(|p| p[3] as f32 / 255.0 * scale > cutoff).count();
        pass as f32 / n.max(1) as f32
    };
    let (mut w, mut h) = (width as usize, height as usize);
    let mut offset = 0;
    let mut target = None;
    for _ in 0..levels {
        let len = w * h * 4;
        if offset + len > chain.len() {
            break;
        }
        let level = &mut chain[offset..offset + len];
        match target {
            None => target = Some(coverage(level, 1.0)),
            Some(t) => {
                let (mut lo, mut hi) = (0.5f32, 8.0f32);
                for _ in 0..12 {
                    let mid = (lo + hi) * 0.5;
                    if coverage(level, mid) < t {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let scale = hi;
                for p in level.as_chunks_mut::<4>().0 {
                    p[3] = (p[3] as f32 * scale).round().min(255.0) as u8;
                }
            }
        }
        offset += len;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
}

/// Drop the `skip` largest levels of a chain from [`build_chain`] (a cheaper,
/// blurrier texture for low-memory machines). Never drops below 64 pixels.
/// Returns (chain, width, height, levels).
pub fn drop_levels(chain: Vec<u8>, width: u32, height: u32, levels: u32, skip: u32) -> (Vec<u8>, u32, u32, u32) {
    let (mut w, mut h, mut levels, mut offset) = (width, height, levels, 0usize);
    for _ in 0..skip {
        if w.min(h) / 2 < 64 || levels < 2 {
            break;
        }
        offset += (w * h * 4) as usize;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
        levels -= 1;
    }
    if offset == 0 {
        return (chain, width, height, levels);
    }
    (chain[offset..].to_vec(), w, h, levels)
}

#[cfg(test)]
mod tests {
    #[test]
    fn recolour_turns_sandstone_grey_and_keeps_alpha() {
        let mut px = vec![180u8, 120, 70, 200, 30, 30, 30, 255];
        super::recolour(&mut px, 0.0, [1.0, 1.0, 1.0]);
        assert_eq!(px[0], px[1]);
        assert_eq!(px[1], px[2]);
        assert_eq!(px[3], 200, "alpha untouched");
        assert_eq!(&px[4..], &[30, 30, 30, 255], "grey stays the same grey");
        // Brightness is kept: the grey sits between the old channels.
        assert!(px[0] > 70 && px[0] < 180);
        // A cool tint leaves blue on top.
        let mut px = vec![180u8, 120, 70, 255];
        super::recolour(&mut px, 0.1, [0.85, 0.92, 1.05]);
        assert!(px[2] > px[0], "{px:?}");
    }

    #[test]
    fn thin_foliage_keeps_its_coverage_in_small_levels() {
        // One-pixel vertical strokes every 4 pixels (25% coverage), of mixed opacity.
        let (w, h) = (64u32, 64u32);
        let mut data = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in (0..w).step_by(4) {
                let i = ((y * w + x) * 4) as usize;
                let a = 120 + ((x * 7 + y * 13) % 136) as u8;
                data[i..i + 4].copy_from_slice(&[255, 255, 255, a]);
            }
        }
        let (mut chain, levels) = build_chain(w, h, &data, true);
        let level2 = |c: &[u8]| -> f32 {
            let start = ((w * h + w * h / 4) * 4) as usize;
            let len = (w * h / 16 * 4) as usize;
            c[start..start + len].chunks_exact(4).filter(|p| p[3] as f32 / 255.0 > 0.35).count() as f32 / (len / 4) as f32
        };
        assert!(level2(&chain) < 0.05, "plain averaging loses the strokes");
        preserve_coverage(&mut chain, w, h, levels, 0.35);
        let c = level2(&chain);
        assert!(c > 0.15, "coverage restored: {c}");
    }

    #[test]
    fn dropping_levels_keeps_a_valid_smaller_chain() {
        let data = vec![200u8; 256 * 128 * 4];
        let (chain, levels) = build_chain(256, 128, &data, true);
        let (small, w, h, l) = drop_levels(chain.clone(), 256, 128, levels, 1);
        assert_eq!((w, h, l), (128, 64, levels - 1));
        assert_eq!(small.len(), chain.len() - 256 * 128 * 4);
        let (again, _, _) = (build_chain(128, 64, &vec![200u8; 128 * 64 * 4], true).0, 0, 0);
        assert_eq!(small.len(), again.len());
        // Small textures are left alone.
        let (same, w, _, _) = drop_levels(chain.clone(), 256, 128, levels, 3);
        assert_eq!(w, 128, "stops at 64 pixels");
        assert_eq!(same.len(), small.len());
    }

    use super::*;

    #[test]
    fn level_counts() {
        assert_eq!(level_count(1, 1), 1);
        assert_eq!(level_count(4, 4), 3);
        assert_eq!(level_count(1024, 1024), 11);
        assert_eq!(level_count(512, 256), 10);
        assert_eq!(level_count(3, 5), 3);
    }

    #[test]
    fn chain_has_every_level() {
        let src = vec![200u8; 4 * 4 * 4];
        let (out, levels) = build_chain(4, 4, &src, true);
        assert_eq!(levels, 3);
        assert_eq!(out.len(), (16 + 4 + 1) * 4);
        // A flat colour stays the same colour at every level.
        assert!(out.iter().all(|&v| (v as i32 - 200).abs() <= 1));
    }

    #[test]
    fn srgb_averages_in_linear_light() {
        // Black and white checker: the 1x1 level should be ~mid-grey in light
        // (sRGB 188), not 128, which would look too dark.
        let mut src = Vec::new();
        for i in 0..4 {
            let v = if i % 3 == 0 { 255 } else { 0 };
            src.extend_from_slice(&[v, v, v, 255]);
        }
        let (out, _) = build_chain(2, 2, &src, true);
        let last = &out[out.len() - 4..];
        assert!((last[0] as i32 - 188).abs() <= 2, "{}", last[0]);
        assert_eq!(last[3], 255);
        let (lin, _) = build_chain(2, 2, &src, false);
        assert_eq!(lin[lin.len() - 4], 128);
    }

    #[test]
    fn non_square_textures() {
        let src = vec![10u8; 8 * 2 * 4];
        let (out, levels) = build_chain(8, 2, &src, false);
        assert_eq!(levels, 4);
        assert_eq!(out.len(), (16 + 4 + 2 + 1) * 4);
    }

    // ---- audit: recolour ----

    #[test]
    fn recolour_handles_empty_and_ragged_buffers() {
        let mut none: Vec<u8> = Vec::new();
        recolour(&mut none, 0.0, [1.0; 3]);
        assert!(none.is_empty());
        // 7 bytes: one pixel and three stray bytes, which are left alone.
        let mut px = vec![200u8, 100, 50, 255, 9, 8, 7];
        recolour(&mut px, 0.0, [1.0; 3]);
        assert_eq!(px[0], px[1]);
        assert_eq!(&px[3..], &[255, 9, 8, 7]);
        let mut short = vec![1u8, 2, 3];
        recolour(&mut short, 0.0, [1.0; 3]);
        assert_eq!(short, vec![1, 2, 3]);
    }

    #[test]
    fn recolour_with_full_colour_and_no_tint_changes_nothing_visible() {
        let mut px = Vec::new();
        for r in (0..=255).step_by(15) {
            for g in (0..=255).step_by(17) {
                for b in (0..=255).step_by(51) {
                    px.extend_from_slice(&[r as u8, g as u8, b as u8, 77]);
                }
            }
        }
        let before = px.clone();
        recolour(&mut px, 1.0, [1.0; 3]);
        for (a, b) in px.iter().zip(&before) {
            assert!((*a as i32 - *b as i32).abs() <= 1, "{a} vs {b}");
        }
    }

    #[test]
    fn recolour_keeps_black_and_white_and_is_stable_when_repeated() {
        let mut px = vec![0u8, 0, 0, 255, 255, 255, 255, 255];
        recolour(&mut px, 0.0, [1.0; 3]);
        assert_eq!(px, vec![0, 0, 0, 255, 255, 255, 255, 255]);
        let mut a = vec![180u8, 120, 70, 255];
        recolour(&mut a, 0.0, [1.0; 3]);
        let once = a.clone();
        recolour(&mut a, 0.0, [1.0; 3]);
        for k in 0..3 {
            assert!((a[k] as i32 - once[k] as i32).abs() <= 1, "grey stays grey");
        }
    }

    #[test]
    fn recolour_survives_extreme_tints_and_saturation() {
        let src = vec![180u8, 120, 70, 123, 0, 255, 10, 0];
        // A zero tint is black, alpha untouched.
        let mut a = src.clone();
        recolour(&mut a, 1.0, [0.0; 3]);
        assert_eq!(&a[..3], &[0, 0, 0]);
        assert_eq!((a[3], a[7]), (123, 0));
        // A huge tint saturates at white but cannot turn a zero channel on.
        let mut b = src.clone();
        recolour(&mut b, 1.0, [1e6; 3]);
        assert_eq!(&b[..3], &[255, 255, 255]);
        assert_eq!(b[4], 0, "pure zero stays zero");
        // Negative tint and over/under-unit saturation clamp instead of wrapping.
        let mut c = src.clone();
        recolour(&mut c, 1.0, [-3.0, 1.0, 1.0]);
        assert_eq!(c[0], 0);
        for keep in [-5.0f32, 0.0, 1.0, 10.0, f32::INFINITY, f32::NAN] {
            let mut d = src.clone();
            recolour(&mut d, keep, [0.9, 1.0, 1.1]);
            assert_eq!((d[3], d[7]), (123, 0), "alpha untouched at keep={keep}");
        }
    }

    #[test]
    fn recolour_darkens_nothing_when_desaturating_a_grey_scan() {
        // Stone detail survives: brighter input stays brighter output.
        let mut last = 0u8;
        for v in (0..=255).step_by(5) {
            let mut px = vec![v as u8, v as u8, (v / 2) as u8, 255];
            recolour(&mut px, 0.1, [0.9, 0.95, 1.05]);
            assert!(px[1] >= last, "monotonic at {v}");
            last = px[1];
        }
    }

    // ---- audit: chains, coverage, level dropping ----

    #[test]
    fn odd_sizes_still_make_a_full_chain_without_darkening_the_edges() {
        let (out, levels) = build_chain(5, 3, &vec![200u8; 5 * 3 * 4], true);
        assert_eq!(levels, 3);
        assert_eq!(out.len(), (15 + 2 + 1) * 4, "5x3, 2x1, 1x1");
        assert!(out.iter().all(|&v| (v as i32 - 200).abs() <= 1), "edge texels are replicated, not zero-filled");
        let (col, levels) = build_chain(1, 8, &vec![90u8; 8 * 4], false);
        assert_eq!(levels, 4);
        assert_eq!(col.len(), (8 + 4 + 2 + 1) * 4);
        let (one, levels) = build_chain(1, 1, &[1, 2, 3, 4], true);
        assert_eq!((one, levels), (vec![1, 2, 3, 4], 1));
    }

    #[test]
    fn alpha_is_averaged_as_coverage_not_as_light() {
        let src = [255, 255, 255, 0, 255, 255, 255, 255, 255, 255, 255, 0, 255, 255, 255, 255];
        let (out, _) = build_chain(2, 2, &src, true);
        let a = out[out.len() - 1];
        assert!((a as i32 - 128).abs() <= 1, "alpha {a}");
    }

    #[test]
    fn opaque_foliage_stays_opaque_and_empty_stays_empty() {
        let (w, h) = (16u32, 16u32);
        let opaque = vec![255u8; (w * h * 4) as usize];
        let (mut chain, levels) = build_chain(w, h, &opaque, true);
        let before = chain.clone();
        preserve_coverage(&mut chain, w, h, levels, 0.35);
        for px in chain.as_chunks::<4>().0 {
            assert!(px[3] as f32 / 255.0 > 0.35, "still passes the cut-off");
        }
        // Colour is never touched, only alpha.
        for (a, b) in chain.as_chunks::<4>().0.iter().zip(before.as_chunks::<4>().0) {
            assert_eq!(&a[..3], &b[..3]);
        }
        let clear = vec![0u8; (w * h * 4) as usize];
        let (mut chain, levels) = build_chain(w, h, &clear, true);
        preserve_coverage(&mut chain, w, h, levels, 0.35);
        assert!(chain.chunks_exact(4).all(|p| p[3] == 0), "nothing appears from nothing");
    }

    #[test]
    fn coverage_fix_tolerates_short_and_empty_chains() {
        let mut empty: Vec<u8> = Vec::new();
        preserve_coverage(&mut empty, 8, 8, 4, 0.5);
        let (chain, levels) = build_chain(8, 8, &vec![128u8; 8 * 8 * 4], true);
        let mut cut = chain[..(64 + 16) * 4].to_vec();
        preserve_coverage(&mut cut, 8, 8, levels, 0.5);
        assert_eq!(cut.len(), (64 + 16) * 4);
        let mut one = chain.clone();
        preserve_coverage(&mut one, 8, 8, 1, 0.5);
        assert_eq!(one, chain, "a single level has nothing to fix");
    }

    #[test]
    fn dropping_levels_edge_cases() {
        let data = vec![9u8; 512 * 64 * 4];
        let (chain, levels) = build_chain(512, 64, &data, true);
        // The short side is already at the floor: nothing is dropped.
        let (same, w, h, l) = drop_levels(chain.clone(), 512, 64, levels, 2);
        assert_eq!((w, h, l, same.len()), (512, 64, levels, chain.len()));
        // skip = 0 is a no-op; a one-level chain is left alone.
        let (c256, l256) = build_chain(256, 256, &vec![9u8; 256 * 256 * 4], true);
        assert_eq!(drop_levels(c256.clone(), 256, 256, l256, 0).0, c256);
        let (flat, _, _, l) = drop_levels(vec![1, 2, 3, 4], 256, 256, 1, 4);
        assert_eq!((flat.len(), l), (4, 1));
        // The returned chain is exactly the remaining levels.
        let (small, w, h, l) = drop_levels(c256.clone(), 256, 256, l256, 99);
        assert_eq!((w, h), (64, 64));
        assert_eq!(small, build_chain(64, 64, &vec![9u8; 64 * 64 * 4], true).0);
        assert_eq!(l, l256 - 2);
    }

    // ---- Equivalence with the original (slow) implementations ----------------

    fn noise_image(w: usize, h: usize, seed: u64, with_alpha: bool) -> Vec<u8> {
        let mut r = crate::sim::rng::Rng::new(seed);
        let mut px = Vec::with_capacity(w * h * 4);
        for i in 0..w * h {
            // Blocks of smooth and of noisy texels, so both flat areas and
            // hard edges get averaged.
            let smooth = (i / w / 4 + i % w / 4) % 2 == 0;
            for c in 0..4 {
                let v = if smooth { ((i % w) * 255 / w.max(1)) as u8 } else { (r.f32() * 256.0) as u8 };
                px.push(if c == 3 && !with_alpha { 255 } else { v });
            }
        }
        px
    }

    #[test]
    fn the_srgb_table_gives_the_same_byte_as_the_formula_everywhere_we_look() {
        let table = SrgbTable::get();
        let check = |l: f32| assert_eq!(table.byte(l), to_srgb(l), "l = {l:e} (bits {:#x})", l.to_bits());
        // Awkward values.
        for l in [0.0, -0.0, f32::MIN_POSITIVE, 1e-45, 0.0031308, 0.0031309, 0.5, 1.0, 1.0000001, 2.0, 1e30, f32::MAX, f32::INFINITY, f32::NEG_INFINITY, f32::NAN, -1.0, -1e-30] {
            check(l);
        }
        // Every spot where the byte steps, and 40 floats either side of it.
        for &e in &table.edge[..255] {
            let bits = e.to_bits();
            for d in 0..40u32 {
                check(f32::from_bits(bits + d));
                check(f32::from_bits(bits.saturating_sub(d)));
            }
        }
        // Every average of four texels of a smooth ramp, and a million
        // random floats spread over the bit patterns of 0..1.
        for a in (0..=255).step_by(3) {
            for b in (0..=255).step_by(5) {
                let lin = to_linear_table();
                check((lin[a] + lin[b] + lin[255 - a] + lin[b / 2]) / 4.0);
            }
        }
        let mut r = crate::sim::rng::Rng::new(5);
        for _ in 0..1_000_000 {
            check(f32::from_bits((r.next_u64() % 0x3F80_0001) as u32));
            check(r.f32());
        }
    }

    /// The whole range, one float at a time: about a billion, so it is skipped
    /// in normal runs (`cargo test -- --ignored srgb_table_is_exact`).
    #[test]
    #[ignore]
    fn srgb_table_is_exact_for_all_billion_floats_between_0_and_1() {
        let table = SrgbTable::get();
        let threads = 4u32;
        let step = 0x3F80_0001u32.div_ceil(threads);
        std::thread::scope(|s| {
            for t in 0..threads {
                s.spawn(move || {
                    let table = SrgbTable::get();
                    for bits in (t * step)..((t + 1) * step).min(0x3F80_0001) {
                        let l = f32::from_bits(bits);
                        assert_eq!(table.byte(l), to_srgb(l), "bits {bits:#x}");
                    }
                });
            }
        });
        let _ = table;
    }

    #[test]
    fn the_fast_chain_is_byte_for_byte_the_old_chain() {
        // Square, odd, tall, wide, thin and tiny pictures; colour and linear data; with and without alpha.
        let sizes = [(64, 64), (37, 21), (100, 7), (5, 90), (1, 1), (1, 33), (33, 1), (2, 2), (3, 3), (128, 96), (250, 250)];
        for (n, &(w, h)) in sizes.iter().enumerate() {
            for srgb in [true, false] {
                for alpha in [true, false] {
                    let img = noise_image(w, h, 100 + n as u64, alpha);
                    let fast = build_chain(w as u32, h as u32, &img, srgb);
                    let old = build_chain_reference(w as u32, h as u32, &img, srgb);
                    assert!(fast == old, "{w}x{h} srgb={srgb} alpha={alpha}");
                }
            }
        }
    }

    #[test]
    fn a_big_photo_sized_chain_matches_too() {
        let img = noise_image(512, 512, 9, true);
        assert!(build_chain(512, 512, &img, true) == build_chain_reference(512, 512, &img, true));
    }

    #[test]
    fn broken_pictures_still_behave_as_before() {
        // Empty, and a buffer that is too short: the old code's answers are kept.
        for (w, h) in [(0u32, 0u32), (4, 0), (0, 4)] {
            let a = std::panic::catch_unwind(|| build_chain(w, h, &[], true));
            let b = std::panic::catch_unwind(|| build_chain_reference(w, h, &[], true));
            assert_eq!(a.is_ok(), b.is_ok(), "{w}x{h}");
            if let (Ok(a), Ok(b)) = (a, b) {
                assert!(a == b);
            }
        }
        let short = vec![9u8; 10];
        let a = std::panic::catch_unwind(|| build_chain(4, 4, &short, true)).is_ok();
        let b = std::panic::catch_unwind(|| build_chain_reference(4, 4, &short, true)).is_ok();
        assert_eq!(a, b);
    }

    #[test]
    fn the_fast_recolour_is_byte_for_byte_the_old_one() {
        let old = |rgba: &mut [u8], keep: f32, tint: [f32; 3]| {
            let lin = to_linear_table();
            for px in rgba.as_chunks_mut::<4>().0 {
                let c = [lin[px[0] as usize], lin[px[1] as usize], lin[px[2] as usize]];
                let y = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
                for k in 0..3 {
                    px[k] = to_srgb((y + (c[k] - y) * keep) * tint[k]);
                }
            }
        };
        for (n, (keep, tint)) in [(0.0, [1.0, 1.0, 1.0]), (0.3, [0.85, 0.92, 1.05]), (1.0, [1.0; 3]), (2.5, [1.3, 0.2, 1.0]), (-1.0, [0.0, 5.0, 1.0]), (f32::NAN, [1.0; 3])].into_iter().enumerate() {
            let mut a = noise_image(90, 70, 40 + n as u64, true);
            let mut b = a.clone();
            recolour(&mut a, keep, tint);
            old(&mut b, keep, tint);
            assert!(a == b, "keep {keep} tint {tint:?}");
        }
    }

    #[test]
    fn the_fast_coverage_fix_is_byte_for_byte_the_old_one() {
        for (n, &(w, h)) in [(64usize, 64usize), (100, 37), (256, 256), (33, 33)].iter().enumerate() {
            for cutoff in [0.35f32, 0.5, 0.0, 0.99] {
                // Thin strokes of mixed opacity, as in a foliage card.
                let mut img = vec![0u8; w * h * 4];
                let mut r = crate::sim::rng::Rng::new(70 + n as u64);
                for px in img.as_chunks_mut::<4>().0 {
                    px[..3].copy_from_slice(&[200, 220, 200]);
                    px[3] = if r.chance(0.3) { 100 + (r.f32() * 155.0) as u8 } else { 0 };
                }
                let (chain, levels) = build_chain_reference(w as u32, h as u32, &img, true);
                let (mut a, mut b) = (chain.clone(), chain);
                preserve_coverage(&mut a, w as u32, h as u32, levels, cutoff);
                preserve_coverage_reference(&mut b, w as u32, h as u32, levels, cutoff);
                assert!(a == b, "{w}x{h} cutoff {cutoff}");
            }
        }
    }
}
