#!/usr/bin/env python3
"""Bake image-based-lighting cubemaps for Bevy 0.16 from CC0 Poly Haven HDRIs.

    python3 tools/bake_ibl.py            # download (cached), bake, validate, preview
    python3 tools/bake_ibl.py snow_field # one map only

Needs only python3 + numpy + pillow. Source files are cached in ~/stage/ibl
(never /tmp); outputs go to assets/environment/<id>_diffuse.ktx2 and
<id>_specular.ktx2, plus tonemapped previews in ~/stage/ibl/preview_*.png.

What is written (all of it matches what bevy_image 0.16.1 ktx2.rs and
bevy_pbr light_probe/environment_map.wgsl expect):

* KTX2, vkFormat R16G16B16A16_SFLOAT (-> TextureFormat::Rgba16Float),
  uncompressed (no zstd needed), 6 faces in the order +X -X +Y -Y +Z -Z,
  all mips stored in a standard level index (smallest mip first in the file).
* Faces use the usual Vulkan/wgpu cube orientation (row 0 = top of the face).
  Bevy's shader samples the cube with (x, y, -z) of the world direction
  ("cube maps are left-handed"), so we bake the texel whose lookup direction
  is d as world direction (d.x, d.y, -d.z). Net effect in game: the sky is up
  (+Y) and the panorama's centre is world -Z with +X to its right.
* Diffuse: one 32 px mip holding cosine-weighted MEAN radiance (irradiance / pi),
  which is what Bevy multiplies by albedo with no further pi.
* Specular: 256 px base, full mip chain (9 mips); Bevy picks mip =
  perceptual_roughness * (mips - 1), so mip i is prefiltered with GGX at
  perceptual roughness i/(mips-1), alpha = roughness^2.
* Both maps are normalised so the mean diffuse radiance over the whole sphere
  is 1.0. Bevy's `EnvironmentMapLight::intensity` is then directly
  "the brightness a uniform AmbientLight would have", in the same cd/m^2 units.
* The sun disc is clamped out of the sky (the game has its own DirectionalLight
  for that); its direction is printed so the Bevy side can rotate the map.
"""
import math
import os
import struct
import sys
import urllib.request

import numpy as np
from PIL import Image

STAGE = os.path.expanduser("~/stage/ibl")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "assets", "environment")

SOURCES = {
    # id -> (author, note)
    "snow_field": "Sergej Majboroda",
    "rural_winter_roadside": "Sergej Majboroda",
}
URL = "https://dl.polyhaven.org/file/ph-assets/HDRIs/hdr/2k/{id}_2k.hdr"

DIFFUSE_SIZE = 32
SPECULAR_SIZE = 256
SPEC_SAMPLES = 160
FACES = ["+X", "-X", "+Y", "-Y", "+Z", "-Z"]
VK_R16G16B16A16_SFLOAT = 97


# --------------------------------------------------------------------------
# Radiance .hdr reader
# --------------------------------------------------------------------------
def read_hdr(path):
    with open(path, "rb") as f:
        data = f.read()
    pos = 0
    header_end = data.index(b"\n\n") + 2
    header = data[:header_end].decode("ascii", "replace")
    assert "rgbe" in header.lower(), "not an RGBE file"
    pos = header_end
    nl = data.index(b"\n", pos)
    res = data[pos:nl].decode().split()
    pos = nl + 1
    assert res[0] == "-Y" and res[2] == "+X", f"unsupported orientation {res}"
    h, w = int(res[1]), int(res[3])
    out = np.empty((h, w, 4), np.uint8)
    for y in range(h):
        assert data[pos] == 2 and data[pos + 1] == 2, "expected new-style RLE scanlines"
        assert (data[pos + 2] << 8 | data[pos + 3]) == w
        pos += 4
        for c in range(4):
            x = 0
            while x < w:
                n = data[pos]
                pos += 1
                if n > 128:
                    n -= 128
                    out[y, x : x + n, c] = data[pos]
                    pos += 1
                else:
                    out[y, x : x + n, c] = np.frombuffer(data, np.uint8, n, pos)
                    pos += n
                x += n
    e = out[..., 3].astype(np.int32)
    scale = np.where(e == 0, 0.0, np.ldexp(1.0, e - 136)).astype(np.float32)
    return out[..., :3].astype(np.float32) * scale[..., None]


def download(id_):
    os.makedirs(STAGE, exist_ok=True)
    path = os.path.join(STAGE, f"{id_}_2k.hdr")
    if not os.path.exists(path) or os.path.getsize(path) < 1_000_000:
        print(f"downloading {id_} ...")
        urllib.request.urlretrieve(URL.format(id=id_), path)
    return path


def luminance(rgb):
    return rgb @ np.array([0.2126, 0.7152, 0.0722], np.float32)


# --------------------------------------------------------------------------
# Equirect sampling. World frame: +Y up, panorama centre = -Z, +X on the right.
# --------------------------------------------------------------------------
def dir_to_uv(d):
    u = 0.5 + np.arctan2(d[..., 0], -d[..., 2]) / (2 * np.pi)
    v = np.arccos(np.clip(d[..., 1], -1, 1)) / np.pi
    return u, v


def uv_to_dir(u, v):
    lon = (u - 0.5) * 2 * np.pi
    lat = (0.5 - v) * np.pi
    return np.stack([np.cos(lat) * np.sin(lon), np.sin(lat), -np.cos(lat) * np.cos(lon)], -1)


def sample_bilinear(img, d):
    h, w, _ = img.shape
    u, v = dir_to_uv(d)
    px = u * w - 0.5
    py = np.clip(v * h - 0.5, 0, h - 1)
    x0 = np.floor(px).astype(np.int64)
    y0 = np.floor(py).astype(np.int64)
    fx = (px - x0)[..., None].astype(np.float32)
    fy = (py - y0)[..., None].astype(np.float32)
    x1 = (x0 + 1) % w
    x0 = x0 % w
    y1 = np.minimum(y0 + 1, h - 1)
    top = img[y0, x0] * (1 - fx) + img[y0, x1] * fx
    bot = img[y1, x0] * (1 - fx) + img[y1, x1] * fx
    return top * (1 - fy) + bot * fy


def pyramid(img):
    levels = [img]
    while levels[-1].shape[0] > 8:
        a = levels[-1]
        levels.append(0.25 * (a[0::2, 0::2] + a[1::2, 0::2] + a[0::2, 1::2] + a[1::2, 1::2]))
    return levels


def sample_lod(levels, d, lod):
    """Trilinear-ish lookup: lod is a float array shaped like d[..., 0]."""
    lod = np.clip(lod, 0, len(levels) - 1)
    lo = np.floor(lod).astype(np.int64)
    out = np.zeros(d.shape, np.float32)
    frac = (lod - lo)[..., None].astype(np.float32)
    for l in range(len(levels)):
        m_lo = lo == l
        m_hi = (lo + 1 == l) & (frac[..., 0] > 0)
        if m_lo.any():
            out[m_lo] += sample_bilinear(levels[l], d[m_lo]) * (1 - frac[m_lo])
        if m_hi.any():
            out[m_hi] += sample_bilinear(levels[l], d[m_hi]) * frac[m_hi]
    return out


# --------------------------------------------------------------------------
# Cube geometry (Vulkan / wgpu convention)
# --------------------------------------------------------------------------
def face_dirs(size):
    """Lookup direction (unnormalised ray) for every texel centre of all 6 faces:
    returns (6, size, size, 3), row 0 = top of the face."""
    t = (np.arange(size) + 0.5) / size * 2 - 1
    sc, tc = np.meshgrid(t, t)  # sc varies with column, tc with row
    one = np.ones_like(sc)
    # (sc, tc, ma) -> direction, from the Vulkan spec cube-face table
    dirs = [
        np.stack([one, -tc, -sc], -1),   # +X: sc=-z, tc=-y
        np.stack([-one, -tc, sc], -1),   # -X: sc=+z, tc=-y
        np.stack([sc, one, tc], -1),     # +Y: sc=+x, tc=+z
        np.stack([sc, -one, -tc], -1),   # -Y: sc=+x, tc=-z
        np.stack([sc, -tc, one], -1),    # +Z: sc=+x, tc=-y
        np.stack([-sc, -tc, -one], -1),  # -Z: sc=-x, tc=-y
    ]
    return np.stack(dirs).astype(np.float64)


def lookup_to_world(d):
    """Bevy flips z when sampling the cube, so lookup dir (x,y,z) is world (x,y,-z)."""
    d = d / np.linalg.norm(d, axis=-1, keepdims=True)
    return np.stack([d[..., 0], d[..., 1], -d[..., 2]], -1)


def texel_weights(size):
    t = (np.arange(size) + 0.5) / size * 2 - 1
    a, b = np.meshgrid(t, t)
    return (1.0 / (a * a + b * b + 1) ** 1.5)  # solid angle ~ per texel


def cube_sample(faces, d):
    """Sample a (6,S,S,C) cube (nearest) with lookup direction d (Vulkan rules)."""
    s = faces.shape[1]
    ax, ay, az = np.abs(d[..., 0]), np.abs(d[..., 1]), np.abs(d[..., 2])
    face = np.zeros(d.shape[:-1], np.int64)
    sc = np.zeros_like(ax)
    tc = np.zeros_like(ax)
    ma = np.maximum(np.maximum(ax, ay), az)
    x, y, z = d[..., 0], d[..., 1], d[..., 2]
    mx = (ax >= ay) & (ax >= az)
    my = (~mx) & (ay >= az)
    mz = ~(mx | my)
    for mask, pos, neg, fp, fn, fsc, ftc in [
        (mx, x > 0, x <= 0, 0, 1, (-z, z), (-y, -y)),
        (my, y > 0, y <= 0, 2, 3, (x, x), (z, -z)),
        (mz, z > 0, z <= 0, 4, 5, (x, -x), (-y, -y)),
    ]:
        for sel, f, i in ((mask & pos, fp, 0), (mask & neg, fn, 1)):
            face[sel] = f
            sc[sel] = fsc[i][sel]
            tc[sel] = ftc[i][sel]
    u = np.clip(((sc / ma + 1) / 2 * s).astype(np.int64), 0, s - 1)
    v = np.clip(((tc / ma + 1) / 2 * s).astype(np.int64), 0, s - 1)
    return faces[face, v, u]


# --------------------------------------------------------------------------
# Baking
# --------------------------------------------------------------------------
def remove_sun(img):
    """Clamp the sun disc (the game's DirectionalLight supplies direct sun).
    Returns the image and the (x, y, z) world direction of the brightest blob."""
    lum = luminance(img)
    h, w = lum.shape
    lat = (0.5 - (np.arange(h) + 0.5) / h) * np.pi
    cap = float(np.percentile(lum, 99.0))
    top = lum >= np.percentile(lum, 99.97)
    ys, xs = np.nonzero(top)
    d = uv_to_dir((xs + 0.5) / w, (ys + 0.5) / h)
    wgt = lum[ys, xs] * np.cos(lat[ys])
    sun = (d * wgt[:, None]).sum(0)
    sun /= np.linalg.norm(sun)
    k = np.minimum(1.0, cap / np.maximum(lum, 1e-6))
    return img * k[..., None], sun, cap


def bake_diffuse(src, size):
    """Cosine-convolve: mean radiance weighted by max(n.l, 0)/pi (so a uniform
    sky of radiance L gives exactly L)."""
    # 128x256 source keeps the brute-force cost sane (about 200M multiplies).
    lv = pyramid(src)
    small = lv[3] if lv[0].shape[0] == 1024 else lv[len(lv) // 2]
    h, w, _ = small.shape
    lat = (0.5 - (np.arange(h) + 0.5) / h) * np.pi
    dw = (2 * np.pi / w) * (np.pi / h) * np.cos(lat)
    dw = np.repeat(dw[:, None], w, 1).reshape(-1)
    v, u = np.meshgrid((np.arange(h) + 0.5) / h, (np.arange(w) + 0.5) / w, indexing="ij")
    ld = uv_to_dir(u, v).reshape(-1, 3)
    rad = small.reshape(-1, 3) * dw[:, None]
    n = lookup_to_world(face_dirs(size)).reshape(-1, 3)
    out = np.zeros((n.shape[0], 3), np.float64)
    for i in range(0, n.shape[0], 512):
        c = np.maximum(n[i : i + 512] @ ld.T, 0.0)
        out[i : i + 512] = c @ rad
    return (out / np.pi).reshape(6, size, size, 3).astype(np.float32)


def hammersley(n):
    i = np.arange(n, dtype=np.uint32)
    b = i.copy()
    b = (b << 16) | (b >> 16)
    b = ((b & 0x55555555) << 1) | ((b & 0xAAAAAAAA) >> 1)
    b = ((b & 0x33333333) << 2) | ((b & 0xCCCCCCCC) >> 2)
    b = ((b & 0x0F0F0F0F) << 4) | ((b & 0xF0F0F0F0) >> 4)
    b = ((b & 0x00FF00FF) << 8) | ((b & 0xFF00FF00) >> 8)
    return (i + 0.5) / n, b.astype(np.float64) / 4294967296.0


def prefilter_ggx(levels, n, roughness, samples):
    """GGX prefilter (N = V = R) with filtered importance sampling: each sample
    reads the source pyramid at a blur matching its pdf, so 160 samples are enough."""
    a = roughness * roughness
    a2 = a * a
    xi1, xi2 = hammersley(samples)
    cos_t = np.sqrt((1 - xi2) / (1 + (a2 - 1) * xi2))
    sin_t = np.sqrt(1 - cos_t**2)
    phi = 2 * np.pi * xi1
    # half-vector in tangent space; L = reflect(-V, H) with V = N, so
    # L = 2 (V.H) H - V
    hx, hy, hz = sin_t * np.cos(phi), sin_t * np.sin(phi), cos_t
    lz = 2 * hz * hz - 1
    lx = 2 * hz * hx
    ly = 2 * hz * hy
    keep = lz > 0
    lx, ly, lz, hz = lx[keep], ly[keep], lz[keep], hz[keep]
    dd = (hz * hz * (a2 - 1) + 1) ** 2 * np.pi
    pdf = a2 / dd / 4.0  # D * NdotH / (4 VdotH) with N = V
    omega_s = 1.0 / (len(lz) * pdf)
    h0, w0, _ = levels[0].shape
    omega_p = 4 * np.pi / (h0 * w0)
    lod = 0.5 * np.log2(omega_s / omega_p) + 1.0
    wgt = lz / lz.sum()
    nrm = lookup_to_world(face_dirs(n)).reshape(-1, 3)
    up = np.where(np.abs(nrm[:, 1:2]) < 0.99, np.array([[0, 1, 0]]), np.array([[1, 0, 0]]))
    tx = np.cross(up, nrm)
    tx /= np.linalg.norm(tx, axis=1, keepdims=True)
    ty = np.cross(nrm, tx)
    out = np.zeros((nrm.shape[0], 3), np.float32)
    chunk = max(1, 400000 // len(lz))
    for i in range(0, nrm.shape[0], chunk):
        N, X, Y = nrm[i : i + chunk], tx[i : i + chunk], ty[i : i + chunk]
        L = lx[None, :, None] * X[:, None, :] + ly[None, :, None] * Y[:, None, :] + lz[None, :, None] * N[:, None, :]
        lods = np.broadcast_to(lod[None, :], L.shape[:2])
        col = sample_lod(levels, L, lods)
        out[i : i + chunk] = (col * wgt[None, :, None]).sum(1)
    return out.reshape(6, n, n, 3)


def bake_specular(src, size, samples):
    levels = pyramid(src)
    nmips = int(math.log2(size)) + 1
    mips = []
    for m in range(nmips):
        s = size >> m
        r = m / (nmips - 1)
        print(f"  specular mip {m}: {s}px roughness {r:.3f}")
        if m == 0:
            # mirror: 2x2 supersampled lookup from the full-res source
            d = lookup_to_world(face_dirs(size * 2)).reshape(-1, 3)
            col = sample_lod(levels, d, np.zeros(len(d)))
            col = col.reshape(6, size * 2, size * 2, 3)
            mips.append(0.25 * (col[:, 0::2, 0::2] + col[:, 1::2, 0::2] + col[:, 0::2, 1::2] + col[:, 1::2, 1::2]))
        else:
            mips.append(prefilter_ggx(levels, s, r, samples))
    return mips


# --------------------------------------------------------------------------
# KTX2 writer / reader
# --------------------------------------------------------------------------
KTX2_ID = bytes([0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A])


def build_dfd():
    """Basic data format descriptor for linear RGBA half floats."""
    words = [0, 2 | (88 << 16), 1 | (1 << 8) | (1 << 16), 0, 8, 0]
    for ch, ctype in enumerate((0, 1, 2, 15)):  # R G B A
        words += [(ch * 16) | (15 << 16) | (ctype << 24) | (0xC << 28), 0, 0xBF800000, 0x3F800000]
    body = struct.pack(f"<{len(words)}I", *words)
    return struct.pack("<I", len(body) + 4) + body


def write_ktx2(path, mips):
    """mips: list of float arrays (6, s, s, 3), mip 0 first."""
    n = len(mips)
    dfd = build_dfd()
    kvd_entry = b"KTXwriter\0" + b"fallout_minnesota tools/bake_ibl.py\0"
    kvd = struct.pack("<I", len(kvd_entry)) + kvd_entry
    kvd += b"\0" * (-len(kvd) % 4)
    level_index_off = 80
    dfd_off = level_index_off + 24 * n
    kvd_off = dfd_off + len(dfd)
    data_off = kvd_off + len(kvd)
    data_off += -data_off % 8
    blobs = []
    for m in mips:
        s = m.shape[1]
        rgba = np.concatenate([m, np.ones((6, s, s, 1), np.float32)], -1)
        rgba = np.clip(rgba, 0, 60000).astype("<f2")
        blobs.append(rgba.tobytes())  # face-major, row 0 = top: KTX2 order
    # file stores the smallest mip first; the level index lists mip 0 first
    offsets = [0] * n
    pos = data_off
    for i in range(n - 1, -1, -1):
        offsets[i] = pos
        pos += len(blobs[i])
        pos += -pos % 8
    w = mips[0].shape[2]
    hgt = mips[0].shape[1]
    head = KTX2_ID + struct.pack(
        "<9I", VK_R16G16B16A16_SFLOAT, 2, w, hgt, 0, 0, 6, n, 0
    )
    head += struct.pack("<4I2Q", dfd_off, len(dfd), kvd_off, len(kvd), 0, 0)
    assert len(head) == 80
    idx = b"".join(struct.pack("<3Q", offsets[i], len(blobs[i]), len(blobs[i])) for i in range(n))
    buf = bytearray(head + idx + dfd + kvd)
    buf += b"\0" * (data_off - len(buf))
    for i in range(n - 1, -1, -1):
        assert len(buf) == offsets[i]
        buf += blobs[i]
        buf += b"\0" * (-len(buf) % 8)
    with open(path, "wb") as f:
        f.write(buf)
    return len(buf)


def read_ktx2(path):
    """Independent parser used for validation. Returns (header dict, mips)."""
    b = open(path, "rb").read()
    assert b[:12] == KTX2_ID, "bad identifier"
    (fmt, tsize, w, h, d, layers, faces, nlev, sc) = struct.unpack_from("<9I", b, 12)
    dfd_o, dfd_l, kvd_o, kvd_l, sgd_o, sgd_l = struct.unpack_from("<4I2Q", b, 48)
    assert fmt == VK_R16G16B16A16_SFLOAT and tsize == 2
    assert faces == 6 and layers == 0 and d == 0 and sc == 0
    assert w == h and w == 1 << (nlev - 1) or nlev == 1
    assert dfd_l == struct.unpack_from("<I", b, dfd_o)[0], "dfd total size mismatch"
    mips = []
    for i in range(nlev):
        off, ln, uln = struct.unpack_from("<3Q", b, 80 + 24 * i)
        s = max(w >> i, 1)
        assert ln == uln == 6 * s * s * 8, f"mip {i} size {ln}"
        assert off % 8 == 0 and off + ln <= len(b)
        a = np.frombuffer(b, "<f2", 6 * s * s * 4, off).reshape(6, s, s, 4).astype(np.float32)
        mips.append(a)
    # smallest mip must come first in the file
    offs = [struct.unpack_from("<3Q", b, 80 + 24 * i)[0] for i in range(nlev)]
    assert offs == sorted(offs, reverse=True), "mips not stored smallest-first"
    return dict(format=fmt, width=w, height=h, faces=faces, levels=nlev, size=len(b)), mips


# --------------------------------------------------------------------------
# Previews
# --------------------------------------------------------------------------
def tonemap(rgb, exposure=1.0):
    x = np.maximum(rgb * exposure, 0)
    x = x * (1 + x / 16) / (1 + x)  # extended Reinhard
    return (np.clip(x, 0, 1) ** (1 / 2.2) * 255).astype(np.uint8)


def cross_sheet(faces, exposure):
    s = faces.shape[1]
    sheet = np.zeros((3 * s, 4 * s, 3), np.uint8) + 30
    pos = {2: (0, 1), 1: (1, 0), 4: (1, 1), 0: (1, 2), 5: (1, 3), 3: (2, 1)}  # +Y, -X, +Z, +X, -Z, -Y
    for f, (r, c) in pos.items():
        sheet[r * s : (r + 1) * s, c * s : (c + 1) * s] = tonemap(faces[f, :, :, :3], exposure)
    return sheet


def panorama_from_cube(faces, w=1024, h=512):
    """Re-project a cube to equirect the way Bevy's shader would read it."""
    v, u = np.meshgrid((np.arange(h) + 0.5) / h, (np.arange(w) + 0.5) / w, indexing="ij")
    world = uv_to_dir(u, v)
    lookup = np.stack([world[..., 0], world[..., 1], -world[..., 2]], -1)
    return cube_sample(faces, lookup)


def previews(id_, dmips, smips, src):
    S = lambda name: os.path.join(STAGE, f"preview_{id_}_{name}.png")
    Image.fromarray(cross_sheet(smips[0], 1.0)).save(S("specular_mip0_cross") )
    Image.fromarray(cross_sheet(dmips[0], 1.0)).resize((512, 384), Image.NEAREST).save(S("diffuse_cross"))
    # mip strip (roughness ramp), each mip up-scaled to 128 for viewing
    strip = []
    for m in smips:
        s = m.shape[1]
        sh = cross_sheet(m, 1.0)
        strip.append(np.array(Image.fromarray(sh).resize((256, 192), Image.NEAREST)))
    rows = [np.concatenate(strip[i : i + 3], 1) for i in range(0, len(strip) - len(strip) % 3, 3)]
    Image.fromarray(np.concatenate(rows, 0)).save(S("specular_mips"))
    # panoramas rebuilt from the files: source | specular mip0 | mip4 | diffuse
    pans = [
        tonemap(resize_f(src, 1024, 512)),
        tonemap(panorama_from_cube(smips[0][..., :3])),
        tonemap(panorama_from_cube(smips[min(4, len(smips) - 1)][..., :3])),
        tonemap(panorama_from_cube(dmips[0][..., :3])),
    ]
    Image.fromarray(np.concatenate(pans, 0)).save(S("panorama_check"))


def resize_f(img, w, h):
    lv = img
    while lv.shape[1] > w:
        lv = 0.25 * (lv[0::2, 0::2] + lv[1::2, 0::2] + lv[0::2, 1::2] + lv[1::2, 1::2])
    return lv


# --------------------------------------------------------------------------
def bake(id_):
    print(f"== {id_}")
    src = read_hdr(download(id_))
    print(f"  read {src.shape[1]}x{src.shape[0]}, max {src.max():.1f}")
    src, sun, cap = remove_sun(src)
    print(f"  sun direction (world, +Y up, -Z forward): ({sun[0]:.3f}, {sun[1]:.3f}, {sun[2]:.3f}); luminance cap {cap:.2f}")
    azimuth = math.degrees(math.atan2(sun[0], -sun[2]))
    print(f"  sun elevation {math.degrees(math.asin(sun[1])):.1f} deg, azimuth {azimuth:.1f} deg (0 = -Z, +90 = +X)")
    diff = bake_diffuse(src, DIFFUSE_SIZE)
    # normalise: sphere-mean luminance of the diffuse map = 1.0
    wts = texel_weights(DIFFUSE_SIZE)[None, :, :] * np.ones((6, 1, 1))
    mean = float((luminance(diff) * wts).sum() / wts.sum())
    scale = 1.0 / mean
    print(f"  normalising by {scale:.4f} (mean diffuse luminance was {mean:.4f})")
    diff *= scale
    src = src * scale
    smips = bake_specular(src, SPECULAR_SIZE, SPEC_SAMPLES)
    os.makedirs(OUT, exist_ok=True)
    dp = os.path.join(OUT, f"{id_}_diffuse.ktx2")
    sp = os.path.join(OUT, f"{id_}_specular.ktx2")
    dsize = write_ktx2(dp, [diff])
    ssize = write_ktx2(sp, smips)
    print(f"  wrote {dp} ({dsize} B), {sp} ({ssize} B); pair {(dsize + ssize) / 1e6:.2f} MB")
    return id_, src, dp, sp, sun


def validate(id_, src, dp, sp):
    hd, dm = read_ktx2(dp)
    hs, sm = read_ktx2(sp)
    print(f"  read back diffuse {hd}")
    print(f"  read back specular {hs}")
    assert hd["levels"] == 1 and hd["width"] == DIFFUSE_SIZE
    assert hs["levels"] == 9 and hs["width"] == SPECULAR_SIZE
    for m in dm + sm:
        assert np.isfinite(m).all() and (m >= 0).all()
    # orientation: the panorama rebuilt from the cube must resemble the source
    ref = resize_f(src, 1024, 512)[..., :3]
    pano = panorama_from_cube(sm[0][..., :3])
    a = np.log1p(luminance(ref)[::4, ::4])
    b = np.log1p(luminance(pano)[::4, ::4])
    corr = float(np.corrcoef(a.ravel(), b.ravel())[0, 1])
    top, bot = luminance(pano[:64]).mean(), luminance(pano[-64:]).mean()
    print(f"  panorama-vs-source log-luminance correlation {corr:.4f} (sky mean {top:.2f}, ground mean {bot:.2f})")
    assert corr > 0.97, "cube orientation does not match the source"
    # horizon continuity across the four side faces (+X -> -Z -> -X -> +Z ring)
    side = sm[0]
    errs = []
    for (fa, fb) in [(4, 0), (0, 5), (5, 1), (1, 4)]:  # +Z|+X, +X|-Z, -Z|-X, -X|+Z (right edge | left edge)
        ea = side[fa, :, -1, :3]
        eb = side[fb, :, 0, :3]
        errs.append(float(np.abs(np.log1p(ea) - np.log1p(eb)).mean()))
    print(f"  mean log seam error on the 4 side-face joins: {[round(e, 3) for e in errs]}")
    assert max(errs) < 0.15, "side faces do not join up"
    # diffuse of a uniform field is the same field: sphere mean is 1 by construction
    wts = texel_weights(DIFFUSE_SIZE)[None, :, :] * np.ones((6, 1, 1))
    print(f"  diffuse sphere-mean luminance {(luminance(dm[0][..., :3]) * wts).sum() / wts.sum():.3f}")
    up_down = (luminance(dm[0][2, 16, 16, :3]), luminance(dm[0][3, 16, 16, :3]))
    print(f"  diffuse luminance looking up {up_down[0]:.3f}, down {up_down[1]:.3f}")
    return dm, sm


def main():
    ids = sys.argv[1:] or list(SOURCES)
    for id_ in ids:
        id_, src, dp, sp, sun = bake(id_)
        dm, sm = validate(id_, src, dp, sp)
        previews(id_, [m[..., :3] for m in dm], [m[..., :3] for m in sm], src)
        print("  previews:", ", ".join(sorted(f for f in os.listdir(STAGE) if f.startswith(f"preview_{id_}"))))


if __name__ == "__main__":
    main()
