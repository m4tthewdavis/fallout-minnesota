#!/usr/bin/env python3
"""Compress the game's textures to GPU block formats in KTX2 files.

Today every JPG/PNG is decoded to RGBA8 and given a mip chain on the main
thread as it loads (src/assets.rs add_mipmaps): about 1.5 GB of RGBA8 for
the ~330 textures loaded at start-up. A KTX2 file holding BC7 (colour,
AO/roughness/metal) or BC5 (normal maps) with its mips already built is a
quarter of that in VRAM and needs no decoding or mip work at load time.
Bevy 0.16 reads these files with its default features (`ktx2` and `zstd`
come with `tonemapping_luts`); no new Cargo features.

The mips are built the same way the game builds them (2x2 box filter, colour
averaged in linear light), the scanned stone listed in src/library.rs GREYED
is drained of colour first, and foliage cards keep their alpha coverage, so
a converted texture looks like the runtime one. Normal-map mips are also
renormalised (the runtime chain is not).

    pip install numpy pillow etcpak texture2ddecoder   # + zstandard for --zstd
    # a few files, with a quality report (PSNR of the base level):
    python3 tools/compress_textures.py --out ~/stage/ktx --report \\
        assets/textures/rusty_metal_02/diff.jpg assets/textures/rusty_metal_02/nor.jpg
    # one model: its textures, plus a copy of the .gltf pointing at .ktx2
    python3 tools/compress_textures.py --out ~/stage/ktx --gltf assets/models/boulder_01/boulder_01.gltf
    # everything (about 280 textures, a few minutes on 4 cores):
    python3 tools/compress_textures.py --out ~/stage/ktx --all
    # package time (CI): convert a copied assets folder in place, drop the
    # replaced JPGs (not textures/generated) and check the result:
    python3 tools/compress_textures.py --assets dist/assets --out dist/assets --all --prune --check

Roles come from the glTF materials with --gltf, otherwise from the file
name: *nor* = normal (BC5), *arm*/*rough*/*spec*/*metal*/*ao*/*opacity* = linear data
(BC7, or BC4 for one-channel images), anything else = sRGB colour (BC7).
Override with --role. Output goes to --out, mirroring the path under
assets/; it never writes into assets/ unless --out points there.

Check a file with Khronos' `ktx validate` (KTX-Software release tarball).
"""

import argparse
import json
import math
import os
import re
import shutil
import struct
import sys
import time
from concurrent.futures import ProcessPoolExecutor

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, ".."))
ASSETS = os.path.join(REPO, "assets")

# Same list as src/assets.rs add_mipmaps.
FOLIAGE = ["spray_", "twigs", "grass_tuft", "reed_plume", "snow_clumps"]
COVERAGE_CUTOFF = 0.35

# Vulkan formats and Khronos data-format-descriptor colour models.
VK = {"bc1": 131, "bc1_srgb": 132, "bc4": 139, "bc5": 141, "bc7": 145, "bc7_srgb": 146}
DF_MODEL = {"bc1": 128, "bc4": 131, "bc5": 132, "bc7": 134}
BLOCK_BYTES = {"bc1": 8, "bc4": 8, "bc5": 16, "bc7": 16}

# ---------------------------------------------------------------- colour


def _lin_table():
    c = np.arange(256, dtype=np.float64) / 255.0
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4).astype(np.float32)


LIN = _lin_table()


def rnd(x):
    """Rust's f32::round (half away from zero) for the non-negative values here."""
    return np.floor(np.asarray(x) + 0.5)


def to_srgb(l):
    l = np.asarray(l, dtype=np.float32)
    c = np.where(l <= 0.0031308, l * 12.92, 1.055 * np.power(np.maximum(l, 0.0031308), 1.0 / 2.4) - 0.055)
    return np.clip(rnd(c * 255.0), 0, 255).astype(np.uint8)


def greyed():
    """(path under assets/, saturation kept) from src/library.rs GREYED."""
    src = open(os.path.join(REPO, "src", "library.rs"), encoding="utf-8").read()
    block = re.search(r"pub const GREYED[^=]*=\s*&\[(.*?)\];", src, re.S)
    if not block:
        return {}
    return {p: float(k) for p, k in re.findall(r'\(\s*"([^"]+)"\s*,\s*([0-9.]+)\s*\)', block.group(1))}


def recolour(rgba, keep):
    """mipmaps::recolour with a white tint: keep `keep` of the saturation."""
    c = LIN[rgba[..., :3]]
    y = (0.2126 * c[..., 0] + 0.7152 * c[..., 1] + 0.0722 * c[..., 2])[..., None]
    out = rgba.copy()
    out[..., :3] = to_srgb(y + (c - y) * keep)
    return out


# ------------------------------------------------------------------ mips


def half(level, srgb, normal):
    """One 2x2 box-filter step, edge-clamped like mipmaps::build_chain."""
    h, w = level.shape[:2]
    nh, nw = max(1, h // 2), max(1, w // 2)
    ys = np.minimum(np.arange(nh)[:, None] * 2 + np.array([0, 1]), h - 1)  # nh x 2
    xs = np.minimum(np.arange(nw)[:, None] * 2 + np.array([0, 1]), w - 1)
    f = level.astype(np.float32) / 255.0
    if srgb:
        f[..., :3] = LIN[level[..., :3]]
    if normal:
        f[..., :3] = f[..., :3] * 2.0 - 1.0
    acc = np.zeros((nh, nw, 4), np.float32)
    for dy in range(2):
        for dx in range(2):
            acc += f[ys[:, dy]][:, xs[:, dx]]
    acc /= 4.0
    out = np.empty((nh, nw, 4), np.uint8)
    if normal:
        n = acc[..., :3]
        n /= np.maximum(np.linalg.norm(n, axis=-1, keepdims=True), 1e-6)
        out[..., :3] = np.clip(rnd((n * 0.5 + 0.5) * 255.0), 0, 255)
    elif srgb:
        out[..., :3] = to_srgb(acc[..., :3])
    else:
        out[..., :3] = np.clip(rnd(acc[..., :3] * 255.0), 0, 255)
    out[..., 3] = np.clip(rnd(acc[..., 3] * 255.0), 0, 255)
    return out


def chain(base, srgb, normal):
    levels = [base]
    while max(levels[-1].shape[:2]) > 1:
        levels.append(half(levels[-1], srgb, normal))
    return levels


def preserve_coverage(levels, cutoff=COVERAGE_CUTOFF):
    """mipmaps::preserve_coverage: keep the share of texels above the cut-off."""
    cov = lambda a, s: float(np.mean(a.astype(np.float32) / 255.0 * s > cutoff))
    target = cov(levels[0][..., 3], 1.0)
    for lv in levels[1:]:
        lo, hi = 0.5, 8.0
        for _ in range(12):
            mid = (lo + hi) * 0.5
            if cov(lv[..., 3], mid) < target:
                lo = mid
            else:
                hi = mid
        lv[..., 3] = np.clip(rnd(lv[..., 3].astype(np.float32) * hi), 0, 255).astype(np.uint8)


# -------------------------------------------------------------- encoding


def pad4(a):
    h, w = a.shape[:2]
    ph, pw = (-h) % 4, (-w) % 4
    if ph or pw:
        a = np.pad(a, ((0, ph), (0, pw), (0, 0)), mode="edge")
    return np.ascontiguousarray(a)


def encode(level, fmt, perceptual):
    import etcpak

    a = pad4(level)
    h, w = a.shape[:2]
    raw = a.tobytes()
    if fmt == "bc7":
        params = etcpak.BC7CompressBlockParams()
        if perceptual:
            params.init_perceptual_weights()
        return etcpak.compress_bc7(raw, w, h, params)
    if fmt == "bc5":
        return etcpak.compress_bc5(raw, w, h)
    if fmt == "bc4":
        return etcpak.compress_bc4(raw, w, h)
    if fmt == "bc1":
        return etcpak.compress_bc1(raw, w, h)
    raise ValueError(fmt)


def decode(data, w, h, fmt):
    """Base level back to RGBA8 (texture2ddecoder returns BGRA)."""
    import texture2ddecoder as t2d

    pw, ph = w + (-w) % 4, h + (-h) % 4
    bgra = np.frombuffer(getattr(t2d, "decode_" + fmt)(data, pw, ph), np.uint8).reshape(ph, pw, 4)
    return bgra[:h, :w, [2, 1, 0, 3]]


def dfd(fmt, srgb):
    model = DF_MODEL[fmt]
    if fmt == "bc5":
        samples = [(0, 63, 0), (64, 63, 1)]
    elif fmt == "bc7":
        samples = [(0, 127, 0)]
    else:
        samples = [(0, 63, 0)]
    block = 24 + 16 * len(samples)
    out = struct.pack("<IHH", 0, 2, block)  # vendor/type, version 2, block size
    out += bytes([model, 1, 2 if srgb else 1, 0])  # model, BT.709, sRGB/linear, alpha straight
    out += bytes([3, 3, 0, 0])  # 4x4x1x1 texel block
    out += bytes([BLOCK_BYTES[fmt], 0, 0, 0, 0, 0, 0, 0])
    for off, length, chan in samples:
        out += struct.pack("<HBB", off, length, chan) + bytes(4) + struct.pack("<II", 0, 0xFFFFFFFF)
    return struct.pack("<I", 4 + len(out)) + out


def ktx2(levels, fmt, srgb, width, height, zstd):
    """A KTX2 file: header, level index, DFD, key/values, then the levels
    smallest first, as the spec lays them out."""
    vk = VK[fmt + "_srgb"] if srgb and fmt in ("bc1", "bc7") else VK[fmt]
    if zstd:
        import zstandard

        cctx = zstandard.ZstdCompressor(level=zstd)
        stored = [cctx.compress(l) for l in levels]
    else:
        stored = list(levels)
    d = dfd(fmt, srgb)
    kv_key, kv_val = b"KTXwriter", b"fallout-minnesota tools/compress_textures.py"
    kv_body = kv_key + b"\0" + kv_val + b"\0"
    kv = struct.pack("<I", len(kv_body)) + kv_body
    kv += bytes((-len(kv)) % 4)
    n = len(levels)
    dfd_off = 80 + 24 * n
    kv_off = dfd_off + len(d)
    pos = kv_off + len(kv)
    # Uncompressed levels start on a multiple of lcm(block size, 4), exactly.
    align = 1 if zstd else math.lcm(BLOCK_BYTES[fmt], 4)
    offsets = [0] * n
    body = b""
    for i in reversed(range(n)):
        pad = (-(pos + len(body))) % align
        body += bytes(pad)
        offsets[i] = pos + len(body)
        body += stored[i]
    head = b"\xabKTX 20\xbb\r\n\x1a\n"
    head += struct.pack("<IIIIIIIII", vk, 1, width, height, 0, 0, 1, n, 2 if zstd else 0)
    head += struct.pack("<IIIIQQ", dfd_off, len(d), kv_off, len(kv), 0, 0)
    for i in range(n):
        head += struct.pack("<QQQ", offsets[i], len(stored[i]), len(levels[i]))
    return head + d + kv + body


# ----------------------------------------------------------------- roles


def role_from_name(path):
    name = os.path.basename(path).lower()
    if re.search(r"(^|_)nor(_|\.|$)|normal", name):
        return "normal"
    if re.search(r"(^|_)(arm|rough|roughness|spec|metal|ao|opacity|mask)(_|\.|$)", name):
        return "data"
    return "colour"


def gltf_roles(gltf_path):
    """{image uri: role} from the materials (sRGB colour vs linear data)."""
    d = json.load(open(gltf_path, encoding="utf-8"))
    tex_img = {i: t.get("source") for i, t in enumerate(d.get("textures", []))}
    roles = {}
    for m in d.get("materials", []):
        pbr = m.get("pbrMetallicRoughness", {})
        slots = [(pbr.get("baseColorTexture"), "colour"), (m.get("emissiveTexture"), "colour"),
                 (pbr.get("metallicRoughnessTexture"), "data"), (m.get("occlusionTexture"), "data"),
                 (m.get("normalTexture"), "normal")]
        for slot, role in slots:
            if slot and tex_img.get(slot["index"]) is not None:
                roles.setdefault(d["images"][tex_img[slot["index"]]]["uri"], role)
    for img in d.get("images", []):  # textures no material uses: linear
        if img.get("uri") and not img["uri"].startswith("data:"):
            roles.setdefault(img["uri"], "data")
    return roles


# --------------------------------------------------------------- convert


def psnr(a, b):
    mse = float(np.mean((a.astype(np.float64) - b.astype(np.float64)) ** 2))
    return 99.0 if mse == 0 else 10 * np.log10(255.0 ** 2 / mse)


def rel_to_assets(path, root=ASSETS):
    """Path under the assets folder, with forward slashes (as the game names it)."""
    p, root = os.path.abspath(path), os.path.abspath(root)
    return os.path.relpath(p, root).replace("\\", "/") if p.startswith(root + os.sep) else os.path.basename(p)


def convert(src, dst, role, rel=None, zstd=0, perceptual=True, report=False, greys=None, max_size=0):
    t0 = time.time()
    img = Image.open(src)
    one_channel = img.mode in ("L", "I;16", "I")
    alpha = "A" in img.getbands() or img.mode == "P"
    rgba = np.array(img.convert("RGBA"))
    h, w = rgba.shape[:2]
    # Block formats need a base level that is a multiple of 4 on both sides.
    if w % 4 or h % 4:
        nw, nh = max(4, round(w / 4) * 4), max(4, round(h / 4) * 4)
        rgba = np.array(Image.fromarray(rgba).resize((nw, nh), Image.LANCZOS))
        h, w = nh, nw
    rel = rel or rel_to_assets(src)
    keep = (greys or {}).get(rel)
    if keep is not None and role == "colour":
        rgba = recolour(rgba, keep)
    srgb = role == "colour"
    normal = role == "normal"
    fmt = "bc5" if normal else "bc4" if one_channel else "bc7"
    levels = chain(rgba, srgb, normal)
    if alpha and any(k in rel for k in FOLIAGE):
        preserve_coverage(levels)
    runtime_bytes = sum(l.size for l in levels)  # what add_mipmaps uploads today
    # --max-size: drop the largest levels, like FMN_TEX_LOD does at run time
    # (small props don't need 1k textures). The report then compares the new
    # base level with the old level of the same size.
    # (A level whose sides aren't multiples of 4 can't be a block-format base.)
    while (max_size and max(levels[0].shape[:2]) > max_size and min(levels[0].shape[:2]) // 2 >= 64
           and all(d % 4 == 0 for d in levels[1].shape[:2])):
        levels.pop(0)
    rgba = levels[0]
    h, w = rgba.shape[:2]
    data = [encode(l, fmt, perceptual and srgb) for l in levels]
    blob = ktx2(data, fmt, srgb, w, h, zstd)
    os.makedirs(os.path.dirname(dst) or ".", exist_ok=True)
    with open(dst, "wb") as f:
        f.write(blob)
    info = dict(src=rel, dst=dst, role=role, fmt=fmt + ("_srgb" if srgb and fmt == "bc7" else ""), w=w, h=h,
                levels=len(levels), src_bytes=os.path.getsize(src), ktx2_bytes=len(blob),
                vram=sum(len(x) for x in data), rgba_vram=runtime_bytes, secs=time.time() - t0)
    if report:
        dec = decode(data[0], w, h, fmt)
        ref = rgba
        if normal:
            info["psnr_rg"] = psnr(dec[..., :2], ref[..., :2])
            n0 = ref[..., :2].astype(np.float32) / 127.5 - 1.0
            n1 = dec[..., :2].astype(np.float32) / 127.5 - 1.0
            z = lambda n: np.sqrt(np.clip(1.0 - (n ** 2).sum(-1), 0.0, 1.0))
            v0 = np.dstack([n0, z(n0)])
            v1 = np.dstack([n1, z(n1)])
            dot = np.clip((v0 * v1).sum(-1) / (np.linalg.norm(v0, axis=-1) * np.linalg.norm(v1, axis=-1) + 1e-9), -1, 1)
            ang = np.degrees(np.arccos(dot))
            info["angle_mean_deg"], info["angle_p99_deg"] = float(ang.mean()), float(np.percentile(ang, 99))
        elif fmt == "bc4":
            info["psnr_r"] = psnr(dec[..., 0], ref[..., 0])
        else:
            for i, c in enumerate("rgb"):
                info["psnr_" + c] = psnr(dec[..., i], ref[..., i])
            info["psnr_rgb"] = psnr(dec[..., :3], ref[..., :3])
            if alpha:
                info["psnr_a"] = psnr(dec[..., 3], ref[..., 3])
    return info


def gltf_jobs(gltf, out, root=ASSETS):
    """Jobs (src, dst, role, rel) for every image of one model, and a copy of
    the .gltf (and its buffers) in `out` whose image URIs point at the .ktx2
    files. With `out` = the assets folder itself the .gltf is rewritten in
    place; URIs that already point at .ktx2 are left alone (safe to re-run)."""
    d = json.load(open(gltf, encoding="utf-8"))
    roles = gltf_roles(gltf)
    folder = os.path.dirname(os.path.abspath(gltf))
    dst_folder = os.path.join(out, rel_to_assets(folder, root))
    jobs = []
    for img in d.get("images", []):
        uri = img.get("uri")
        if not uri or uri.startswith("data:") or uri.lower().endswith(".ktx2"):
            continue
        new = os.path.splitext(uri)[0] + ".ktx2"
        src = os.path.join(folder, uri)
        jobs.append((src, os.path.join(dst_folder, new), roles.get(uri, "data"), rel_to_assets(src, root)))
        img["uri"] = new
        img["mimeType"] = "image/ktx2"  # Bevy picks the loader by extension; the gltf crate accepts any mime
    os.makedirs(dst_folder, exist_ok=True)
    for buf in d.get("buffers", []):
        if buf.get("uri") and not buf["uri"].startswith("data:"):
            a, b = os.path.join(folder, buf["uri"]), os.path.join(dst_folder, buf["uri"])
            if os.path.abspath(a) != os.path.abspath(b):
                shutil.copy2(a, b)
    with open(os.path.join(dst_folder, os.path.basename(gltf)), "w", encoding="utf-8") as f:
        json.dump(d, f, indent=1)
    return jobs


def _run(job):
    # `rel` travels with the job: on Windows the workers are fresh processes.
    src, dst, role, rel, kw = job
    return convert(src, dst, role, rel=rel, **kw)


def prune(out):
    """Delete the JPGs in `out` that now have a .ktx2 twin, except under
    textures/generated (assets.rs MARKER and the fallbacks live there)."""
    gone = 0
    for dirpath, _, names in os.walk(out):
        for n in names:
            if not n.lower().endswith(".jpg"):
                continue
            path = os.path.join(dirpath, n)
            if rel_to_assets(path, out).startswith("textures/generated/"):
                continue
            if os.path.isfile(os.path.splitext(path)[0] + ".ktx2"):
                os.remove(path)
                gone += 1
    return gone


def check(out):
    """Problems with a converted assets folder: glTF images that don't exist,
    files that aren't KTX2, a missing MARKER. Returns (problems, counts)."""
    problems, ktx, gltfs = [], 0, 0
    for dirpath, _, names in os.walk(out):
        for n in names:
            path = os.path.join(dirpath, n)
            if n.endswith(".ktx2"):
                ktx += 1
                with open(path, "rb") as f:
                    if f.read(12) != b"\xabKTX 20\xbb\r\n\x1a\n":
                        problems.append(f"not a KTX2 file: {rel_to_assets(path, out)}")
            elif n.endswith(".gltf"):
                gltfs += 1
                d = json.load(open(path, encoding="utf-8"))
                for img in d.get("images", []):
                    uri = img.get("uri", "")
                    if uri and not uri.startswith("data:") and not os.path.isfile(os.path.join(dirpath, uri)):
                        problems.append(f"{rel_to_assets(path, out)}: missing image {uri}")
    if not os.path.isfile(os.path.join(out, "textures", "generated", "snow_diff.jpg")):
        problems.append("missing textures/generated/snow_diff.jpg (assets.rs MARKER)")
    if ktx == 0:
        problems.append("no .ktx2 files")
    return problems, dict(ktx2=ktx, gltf=gltfs)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("files", nargs="*", help="images to convert")
    ap.add_argument("--gltf", action="append", default=[], help="convert a model's textures and write a .gltf copy")
    ap.add_argument("--all", action="store_true", help="every model and every JPG under assets/textures (PNGs stay as they are)")
    ap.add_argument("--dry-run", action="store_true", help="list the jobs and stop")
    ap.add_argument("--assets", default=ASSETS, help="assets folder to read (default: the repo's); e.g. dist/assets in CI")
    ap.add_argument("--out", required=True, help="output root (mirrors paths under the assets folder); may equal --assets")
    ap.add_argument("--prune", action="store_true", help="afterwards delete JPGs in --out that have a .ktx2 twin (not textures/generated)")
    ap.add_argument("--check", action="store_true", help="afterwards check --out (glTF image files exist, KTX2 headers); exit 1 on problems")
    ap.add_argument("--role", choices=["colour", "data", "normal"], help="override the role for the listed files")
    # BC data only shrinks 4-8% under Zstandard, and Bevy's decoder (ruzstd,
    # pure Rust) would then cost load time on the one or two IO threads.
    ap.add_argument("--zstd", type=int, default=0, help="Zstandard level for the files (default 0 = none); VRAM is unchanged")
    ap.add_argument("--max-size", type=int, default=0, help="largest side of the base level, e.g. 512 for small props (0 = keep)")
    ap.add_argument("--linear-bc7", action="store_true", help="RGB error weights for colour (default: perceptual)")
    ap.add_argument("--report", action="store_true", help="decode the base level and print PSNR")
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 1)
    args = ap.parse_intermixed_args(argv)
    out = os.path.abspath(os.path.expanduser(args.out))
    root = os.path.abspath(os.path.expanduser(args.assets))
    kw = dict(zstd=args.zstd, perceptual=not args.linear_bc7, report=args.report, greys=greyed(), max_size=args.max_size)
    jobs = []
    image_job = lambda f, role: (f, os.path.join(out, os.path.splitext(rel_to_assets(f, root))[0] + ".ktx2"), role, rel_to_assets(f, root), kw)
    for f in args.files:
        jobs.append(image_job(f, args.role or role_from_name(f)))
    if args.all:
        import glob

        args.gltf += sorted(glob.glob(os.path.join(root, "models", "*", "*.gltf")))
        for f in sorted(glob.glob(os.path.join(root, "textures", "**", "*.jpg"), recursive=True)):
            jobs.append(image_job(f, role_from_name(f)))
    if args.dry_run:
        for g in args.gltf:
            print("model", rel_to_assets(g, root))
        for j in jobs:
            print(j[2], j[3])
        return []
    for g in args.gltf:
        jobs += [(s, d, r, rel, kw) for s, d, r, rel in gltf_jobs(g, out, root)]
    seen = set()  # a glTF may list the same image twice
    jobs = [j for j in jobs if not (j[1] in seen or seen.add(j[1]))]
    if not jobs and not (args.prune or args.check):
        ap.error("nothing to convert")
    t0 = time.time()
    results = []
    if jobs:
        with ProcessPoolExecutor(max_workers=max(1, args.jobs)) as pool:
            results = list(pool.map(_run, jobs))
    mib = 1 << 20
    for r in results:
        q = " ".join(f"{k}={v:.2f}" for k, v in r.items() if k.startswith(("psnr", "angle")))
        print(f"{r['src']}: {r['role']} -> {r['fmt']} {r['w']}x{r['h']} x{r['levels']}  "
              f"file {r['src_bytes'] / 1024:.0f} KiB -> {r['ktx2_bytes'] / 1024:.0f} KiB, "
              f"VRAM {r['rgba_vram'] / mib:.2f} -> {r['vram'] / mib:.2f} MiB, {r['secs']:.1f} s  {q}")
    tv, tr = sum(r["vram"] for r in results), sum(r["rgba_vram"] for r in results)
    print(f"{len(results)} textures: VRAM {tr / mib:.1f} -> {tv / mib:.1f} MiB, "
          f"disk {sum(r['src_bytes'] for r in results) / mib:.1f} -> {sum(r['ktx2_bytes'] for r in results) / mib:.1f} MiB, "
          f"{time.time() - t0:.0f} s")
    if args.prune:
        print(f"pruned {prune(out)} JPGs that have a .ktx2 twin")
    if args.check:
        problems, counts = check(out)
        for p in problems:
            print("PROBLEM:", p)
        print(f"check: {counts['ktx2']} .ktx2, {counts['gltf']} .gltf, {len(problems)} problems")
        if problems:
            sys.exit(1)
    return results


if __name__ == "__main__":
    main()
