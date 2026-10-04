//! Mipmap chains for RGBA8 textures.
//!
//! Bevy uploads PNG/JPEG textures with a single mip level, so tiled textures
//! (snow, bark, rust) shimmer and sparkle in the distance. This builds the
//! smaller levels with a 2x2 box filter, averaging colour textures in linear
//! light so they don't darken.

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

fn to_srgb(l: f32) -> u8 {
    let c = if l <= 0.0031308 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 };
    (c * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Returns every level, largest first, concatenated (the layout wgpu expects
/// for one layer), plus the level count. `rgba` is `width * height * 4` bytes.
pub fn build_chain(width: u32, height: u32, rgba: &[u8], srgb: bool) -> (Vec<u8>, u32) {
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
    let coverage = |px: &[u8], scale: f32| -> f32 {
        let n = px.len() / 4;
        let pass = px.chunks_exact(4).filter(|p| p[3] as f32 / 255.0 * scale > cutoff).count();
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
                // Binary search the alpha scale that restores the coverage.
                let (mut lo, mut hi) = (0.5f32, 8.0f32);
                for _ in 0..12 {
                    let mid = (lo + hi) * 0.5;
                    if coverage(level, mid) < t {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                // Err towards keeping the foliage rather than losing it.
                let scale = hi;
                for p in level.chunks_exact_mut(4) {
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
}
