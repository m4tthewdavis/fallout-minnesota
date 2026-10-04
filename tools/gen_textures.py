#!/usr/bin/env python3
"""Generate the game's procedural textures into assets/textures/generated and
assets/ui. Everything here is made from code (noise, Voronoi cracks, text), so
it is original work released with the project.

    pip install numpy pillow
    python3 tools/gen_textures.py
    python3 tools/test_gen_textures.py   # checks the sign layouts
"""

import math
import os

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
ASSETS = os.path.join(HERE, "..", "assets")
GEN = os.path.join(ASSETS, "textures", "generated")
UI = os.path.join(ASSETS, "ui")
FONT = os.path.join(ASSETS, "fonts", "ShareTechMono-Regular.ttf")
rng = np.random.default_rng(143)


def save(img, folder, name):
    os.makedirs(folder, exist_ok=True)
    img.save(os.path.join(folder, name), optimize=True)
    print("  ", os.path.relpath(os.path.join(folder, name), ASSETS))


def value_noise(size, cells, octaves=4):
    """Tileable fractal value noise in 0..1: random values on a wrapping grid,
    smoothly interpolated, so the left edge meets the right and top meets
    bottom exactly."""
    out = np.zeros((size, size))
    amp, total = 1.0, 0.0
    for o in range(octaves):
        c = cells * 2**o
        grid = rng.random((c, c))
        t = np.arange(size) * c / size
        i0 = np.floor(t).astype(int) % c
        i1 = (i0 + 1) % c
        f = t - np.floor(t)
        f = f * f * (3 - 2 * f)
        # Interpolate along x, then y.
        top = grid[i0][:, i0] * (1 - f)[None, :] + grid[i0][:, i1] * f[None, :]
        bottom = grid[i1][:, i0] * (1 - f)[None, :] + grid[i1][:, i1] * f[None, :]
        out += amp * (top * (1 - f)[:, None] + bottom * f[:, None])
        total += amp
        amp *= 0.5
    return out / total


def voronoi_edges(size, points):
    """Distance to the nearest Voronoi cell border (tileable), in pixels."""
    pts = rng.random((points, 2)) * size
    tiles = np.concatenate([pts + np.array([dx, dy]) * size for dx in (-1, 0, 1) for dy in (-1, 0, 1)])
    yy, xx = np.mgrid[0:size, 0:size].astype(float)
    d1 = np.full((size, size), 1e9)
    d2 = np.full((size, size), 1e9)
    for px, py in tiles:
        d = np.hypot(xx - px, yy - py)
        d2 = np.where(d < d1, d1, np.minimum(d2, d))
        d1 = np.minimum(d1, d)
    return d2 - d1


def nuclear_ice():
    """Cracked lake ice: pale teal sheets with glowing radioactive cracks."""
    n = 512
    edges = voronoi_edges(n, 38)
    fine = voronoi_edges(n, 140)
    cloud = value_noise(n, 4)
    crack = np.clip(1.0 - edges / 3.0, 0, 1) ** 1.5
    hair = np.clip(1.0 - fine / 1.4, 0, 1) * 0.45 * (cloud > 0.45)
    lines = np.clip(crack + hair, 0, 1)

    base = np.stack([0.50 + 0.10 * cloud, 0.74 + 0.08 * cloud, 0.70 + 0.10 * cloud], -1)
    deep = np.stack([0.18, 0.42, 0.36]) * np.ones((n, n, 3))
    ice = base * (1 - 0.35 * lines[..., None]) + deep * 0.35 * lines[..., None]
    save(Image.fromarray((np.clip(ice, 0, 1) * 255).astype(np.uint8)), GEN, "ice_diff.png")

    glow = np.clip(crack * 1.2 + hair * 0.6, 0, 1)
    glow = np.asarray(Image.fromarray((glow * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(1.5)), float) / 255
    em = np.stack([0.15 * glow, glow, 0.35 * glow], -1) + np.stack([0.0, 0.08, 0.03]) * cloud[..., None]
    save(Image.fromarray((np.clip(em, 0, 1) * 255).astype(np.uint8)), GEN, "ice_emissive.png")


def scorch():
    """Radial scorch decal for the radiation craters, with green glowing flecks."""
    n = 512
    yy, xx = np.mgrid[0:n, 0:n].astype(float)
    r = np.hypot(xx - n / 2, yy - n / 2) / (n / 2)
    noise = value_noise(n, 6)
    ring = np.clip(1.0 - (r + (noise - 0.5) * 0.35), 0, 1)
    alpha = np.clip(ring * 1.6, 0, 1)
    dark = 0.10 + 0.12 * noise
    rgb = np.stack([dark * 0.9, dark * 1.15, dark * 0.8], -1)
    flecks = (rng.random((n, n)) > 0.996) & (r < 0.85)
    flecks = np.asarray(Image.fromarray((flecks * 255).astype(np.uint8)).filter(ImageFilter.MaxFilter(3)), float) / 255
    rgb = rgb * (1 - flecks[..., None]) + np.array([0.35, 1.0, 0.3]) * flecks[..., None]
    img = np.concatenate([rgb, alpha[..., None]], -1)
    save(Image.fromarray((np.clip(img, 0, 1) * 255).astype(np.uint8), "RGBA"), GEN, "scorch.png")


def soft_disc():
    """Round soft particle sprite (smoke, breath, snow puffs)."""
    n = 128
    yy, xx = np.mgrid[0:n, 0:n].astype(float)
    r = np.hypot(xx - n / 2 + 0.5, yy - n / 2 + 0.5) / (n / 2)
    noise = value_noise(n, 4)
    a = np.clip(1 - r, 0, 1) ** 1.6 * (0.75 + 0.25 * noise)
    img = np.dstack([np.ones((n, n)), np.ones((n, n)), np.ones((n, n)), a])
    save(Image.fromarray((img * 255).astype(np.uint8), "RGBA"), GEN, "soft.png")


def flash():
    """Star-shaped muzzle flash sprite."""
    n = 128
    yy, xx = np.mgrid[0:n, 0:n].astype(float)
    dx, dy = xx - n / 2 + 0.5, yy - n / 2 + 0.5
    r = np.hypot(dx, dy) / (n / 2)
    ang = np.arctan2(dy, dx)
    spikes = 0.45 + 0.55 * np.abs(np.cos(ang * 3.5)) ** 6
    a = np.clip(1 - r / spikes, 0, 1) ** 1.3
    core = np.clip(1 - r * 3, 0, 1)
    rgb = np.stack([np.ones_like(a), 0.75 + 0.25 * core, 0.35 + 0.65 * core], -1)
    img = np.concatenate([rgb, a[..., None]], -1)
    save(Image.fromarray((np.clip(img, 0, 1) * 255).astype(np.uint8), "RGBA"), GEN, "flash.png")


def stars():
    """Equirectangular starfield (alpha = brightness) for the night sky dome."""
    w, h = 2048, 1024
    img = np.zeros((h, w))
    count = 2600
    xs = rng.integers(0, w, count)
    # Uniform on the sphere: sample v from cos(latitude).
    ys = (np.arccos(rng.uniform(-1, 1, count)) / math.pi * h).astype(int).clip(0, h - 1)
    mags = rng.power(5, count)
    img[ys, xs] = mags
    # A faint Milky Way band.
    yy, xx = np.mgrid[0:h, 0:w].astype(float)
    band = np.exp(-(((yy - h * 0.38 - 120 * np.sin(xx / w * 2 * math.pi)) / 70) ** 2))
    dust = value_noise(1024, 8)
    dust = np.asarray(Image.fromarray((dust * 255).astype(np.uint8)).resize((w, h)), float) / 255
    milky = band * dust * 0.22
    bright = (img > 0.9).astype(np.uint8) * 255
    bright = np.asarray(Image.fromarray(bright).filter(ImageFilter.MaxFilter(3)), float) / 255 * 0.8
    a = np.clip(np.maximum(img, bright) + milky, 0, 1)
    rgb = np.dstack([np.full((h, w), 0.85), np.full((h, w), 0.9), np.ones((h, w))])
    out = np.dstack([rgb, a])
    save(Image.fromarray((out * 255).astype(np.uint8), "RGBA"), GEN, "stars.png")


def aurora():
    """Vertical curtain strip for the aurora ribbons (alpha fades top and bottom)."""
    w, h = 512, 256
    yy, xx = np.mgrid[0:h, 0:w].astype(float)
    v = yy / h
    streaks = value_noise(512, 32, 3)[:h, :w]
    streaks = np.repeat(streaks[:1], h, 0) * 0.6 + 0.4
    # Sharp bright lower edge, fading out towards the top.
    fade = v**1.6 * np.clip((1 - v) * 10, 0, 1)
    a = streaks * fade
    # Green at the bottom edge, violet at the top, like real aurora.
    g = np.stack([0.65 - 0.45 * v, 0.35 + 0.65 * v, 0.75 - 0.2 * v], -1)
    out = np.concatenate([g, a[..., None]], -1)
    save(Image.fromarray((np.clip(out, 0, 1) * 255).astype(np.uint8), "RGBA"), GEN, "aurora.png")


def layout_sign(text_lines, size, border):
    """Fit the lines inside the sign's inner frame. Returns the frame and, per
    line, (text, font, x, y, ink box) with the ink box in image pixels. Lines
    are sized by their real ink height, then shrunk together until the whole
    block fits both ways with a margin."""
    w, h = size
    inset = 6 + 8 + max(6, int(min(w, h) * 0.05)) if border else max(8, int(min(w, h) * 0.06))
    inner = (inset, inset, w - inset, h - inset)
    avail_w, avail_h = inner[2] - inner[0], inner[3] - inner[1]
    shrink = 1.0
    while True:
        lines = []
        for text, scale in text_lines:
            px = max(6, int(h * scale * shrink))
            font = ImageFont.truetype(FONT, px)
            while font.getlength(text) > avail_w and px > 6:
                px -= 1
                font = ImageFont.truetype(FONT, px)
            lines.append((text, font, font.getbbox(text)))
        heights = [b[3] - b[1] for _, _, b in lines]
        gap = 0.32 * sum(heights) / len(heights)
        total = sum(heights) + gap * (len(lines) - 1)
        if total <= avail_h or shrink < 0.2:
            break
        shrink *= 0.95
    out = []
    y = inner[1] + (avail_h - total) / 2
    for (text, font, b), hgt in zip(lines, heights):
        tw = b[2] - b[0]
        x = inner[0] + (avail_w - tw) / 2 - b[0]
        out.append((text, font, x, y - b[1], (x + b[0], y, x + b[2], y + hgt)))
        y += hgt + gap
    return inner, out


def sign(name, text_lines, size, bg, fg, border=None, rust=0.0):
    """Painted pre-war sign with rust and grime."""
    w, h = size
    img = Image.new("RGB", (w, h), bg)
    d = ImageDraw.Draw(img)
    if border:
        d.rectangle([6, 6, w - 7, h - 7], outline=border, width=8)
    inner, lines = layout_sign(text_lines, size, border)
    for text, font, x, y, ink in lines:
        assert inner[0] <= ink[0] and ink[2] <= inner[2] and inner[1] <= ink[1] and ink[3] <= inner[3] + 0.5, (name, text, ink, inner)
        d.text((x, y), text, font=font, fill=fg)
    arr = np.asarray(img, float) / 255
    noise = value_noise(max(w, h), 8)[:h, :w]
    stain = np.clip((noise - (1 - rust)) * 4, 0, 1)[..., None]
    arr = arr * (0.85 + 0.15 * noise[..., None])
    arr = arr * (1 - stain) + np.array([0.45, 0.25, 0.12]) * stain
    save(Image.fromarray((np.clip(arr, 0, 1) * 255).astype(np.uint8)), GEN, name)


# Every generated sign: file, lines (text, height as a share of the sign), size, colours, rust.
SIGNS = [
    ("sign_bullseye.png", [("BULLSEYE", 0.42), ("- MART -", 0.2)], (512, 256), (200, 20, 25), (245, 240, 230), (245, 240, 230), 0.35),
    ("sign_mille_lacs.png", [("MILLE LACS 5", 0.34), ("VAULT 143  2", 0.34)], (512, 256), (30, 95, 50), (240, 240, 235), (240, 240, 235), 0.25),
    ("sign_golden_atomic.png", [("GOLDEN ATOMIC MILLS", 0.42), ("Enriched Flour Since 1961", 0.18)], (1024, 256), (40, 35, 30), (230, 180, 40), None, 0.3),
    ("sign_speed.png", [("SPEED", 0.13), ("LIMIT", 0.13), ("55", 0.32)], (256, 320), (240, 240, 235), (20, 20, 20), (20, 20, 20), 0.3),
    ("sign_bait.png", [("BAIT &", 0.22), ("TACKLE", 0.22), ("OPEN 24 HRS", 0.1)], (512, 256), (230, 190, 40), (30, 25, 20), (30, 25, 20), 0.45),
    ("sign_welcome.png", [("WELCOME TO", 0.2), ("MILLE LACS", 0.3), ("Pop. 1,143", 0.14)], (512, 256), (30, 95, 50), (240, 240, 235), (240, 240, 235), 0.3),
    ("plate.png", [("MINNESOTA 2077", 0.15), ("LKS-143", 0.4), ("10,000 LAKES", 0.13)], (256, 128), (30, 42, 88), (235, 225, 190), (235, 225, 190), 0.25),
    ("sign_fallout_shelter.png", [("FALLOUT", 0.3), ("SHELTER", 0.3)], (256, 256), (230, 190, 30), (25, 25, 25), (25, 25, 25), 0.3),
]


def clean_snow():
    """Derived from Poly Haven's CC0 snow_02: remove the dark twig squiggles
    (they read as dirt when tiled across a whole map) and lift the levels."""
    src = os.path.join(ASSETS, "textures", "snow_02", "diff.jpg")
    if not os.path.exists(src):
        print("   (skipping clean snow: run fetch_assets.py first)")
        return
    img = Image.open(src).convert("RGB")
    w, h = img.size
    # Filter a 3x3 tiling so the result still tiles seamlessly.
    big = Image.new("RGB", (w * 3, h * 3))
    for dx in range(3):
        for dy in range(3):
            big.paste(img, (dx * w, dy * h))
    # Never let a pixel be much darker than its blurred neighbourhood: keeps
    # the fine grain and bright sparkle, removes dark strokes.
    def blur(arr, r):
        return np.asarray(Image.fromarray(np.clip(arr, 0, 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(r)), float)

    A = np.asarray(big, float)
    base = blur(A, 14)
    # Second pass: average only the pixels that aren't part of a stroke.
    valid = (A.mean(-1) > base.mean(-1) - 6).astype(float)[..., None]
    num = blur(A * valid, 14)
    den = blur(np.repeat(valid * 255, 3, -1), 14) / 255.0
    base = num / np.maximum(den, 1e-3)
    out = np.maximum(A, base - 3.0)[h : 2 * h, w : 2 * w]
    out = np.clip((out - 120) * 1.15 + 150, 0, 255)
    Image.fromarray(out.astype(np.uint8)).save(os.path.join(ASSETS, "textures", "snow_02", "diff_clean.jpg"), quality=90)
    print("   textures/snow_02/diff_clean.jpg")


def gun_textures():
    """Pipe-rifle materials: worn blued steel with rust gathering in pits and
    seams plus bright scratches, and scratched, grimy wood grain."""
    n = 512
    yy, xx = np.mgrid[0:n, 0:n].astype(float)
    # --- steel ---
    brushed = value_noise(n, 64, 2)
    brushed = np.repeat(brushed[:, :1], n, 1) * 0.5 + brushed * 0.5  # streaks along U
    pits = value_noise(n, 12, 5)
    seams = np.clip(1 - voronoi_edges(n, 14) / 5.0, 0, 1)
    rust = np.clip((pits - 0.64) * 5 + seams * 0.55 * (pits > 0.45), 0, 1)
    rust = np.asarray(Image.fromarray((rust * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(1.2)), float) / 255
    scratches = np.zeros((n, n))
    img = Image.fromarray((scratches * 255).astype(np.uint8))
    d = ImageDraw.Draw(img)
    for _ in range(60):
        x0, y0 = rng.random(2) * n
        ang = rng.normal(0, 0.35)
        length = rng.uniform(15, 90)
        d.line([x0, y0, x0 + math.cos(ang) * length, y0 + math.sin(ang) * length], fill=int(rng.uniform(80, 200)), width=1)
    scratches = np.asarray(img, float) / 255
    blued = np.stack([0.16, 0.17, 0.19]) * (0.85 + 0.3 * brushed[..., None])
    rust_col = np.stack([0.3, 0.15, 0.07]) * (0.7 + 0.5 * pits[..., None])
    steel = blued * (1 - rust[..., None]) + rust_col * rust[..., None]
    steel = steel + scratches[..., None] * 0.18 * (1 - rust[..., None])
    save(Image.fromarray((np.clip(steel, 0, 1) * 255).astype(np.uint8)), GEN, "gun_steel_diff.png")
    rough = 0.35 + 0.15 * brushed + 0.5 * rust - 0.15 * scratches
    metal = 1.0 - 0.9 * rust
    arm = np.stack([1 - 0.4 * seams * rust, np.clip(rough, 0, 1), np.clip(metal, 0, 1)], -1)
    save(Image.fromarray((arm * 255).astype(np.uint8)), GEN, "gun_steel_arm.png")

    # --- wood ---
    warp = value_noise(n, 4, 3)
    knots = value_noise(n, 3, 2)
    grain = np.sin((xx / n * 56 + warp * 4 + knots * 1.5) * math.pi) * 0.5 + 0.5
    fine = value_noise(n, 128, 1)
    fine = np.repeat(fine[:1], n, 0) * 0.5 + fine * 0.5  # fine pores along the grain
    wood_h = grain * 0.7 + fine * 0.3
    light = np.stack([0.45, 0.28, 0.15])
    dark = np.stack([0.22, 0.12, 0.06])
    wood = dark + (light - dark) * wood_h[..., None]
    grime = np.clip((value_noise(n, 6, 3) - 0.5) * 2.5, 0, 1)
    wood = wood * (1 - 0.35 * grime[..., None])
    img = Image.fromarray(np.zeros((n, n), np.uint8))
    d = ImageDraw.Draw(img)
    for _ in range(90):
        x0, y0 = rng.random(2) * n
        ang = rng.uniform(0, math.pi)
        length = rng.uniform(8, 50)
        d.line([x0, y0, x0 + math.cos(ang) * length, y0 + math.sin(ang) * length], fill=int(rng.uniform(100, 255)), width=1)
    nicks = np.asarray(img, float) / 255
    wood = wood * (1 - 0.3 * nicks[..., None]) + np.stack([0.55, 0.42, 0.3]) * 0.25 * nicks[..., None]
    save(Image.fromarray((np.clip(wood, 0, 1) * 255).astype(np.uint8)), GEN, "gun_wood_diff.png")
    # Normal map from the grain and scratch height (OpenGL convention).
    hgt = wood_h * 0.6 - nicks * 0.8
    gy, gx = np.gradient(hgt)
    nx, ny, nz = -gx * 3.0, gy * 3.0, np.ones_like(hgt)
    length = np.sqrt(nx * nx + ny * ny + nz * nz)
    nor = np.stack([nx / length, ny / length, nz / length], -1) * 0.5 + 0.5
    save(Image.fromarray((nor * 255).astype(np.uint8)), GEN, "gun_wood_nor.png")


def normal_from_height(hgt, strength):
    """Tangent-space normal map (OpenGL convention) from a tileable height field."""
    gx = (np.roll(hgt, -1, 1) - np.roll(hgt, 1, 1)) * 0.5
    gy = (np.roll(hgt, -1, 0) - np.roll(hgt, 1, 0)) * 0.5
    nx, ny, nz = -gx * strength, gy * strength, np.ones_like(hgt)
    length = np.sqrt(nx * nx + ny * ny + nz * nz)
    return np.stack([nx / length, ny / length, nz / length], -1) * 0.5 + 0.5


def wrap_blur(arr, r):
    """Gaussian blur of a float array, wrapping at the edges so it still tiles."""
    h, w = arr.shape
    fy = np.fft.fftfreq(h)[:, None]
    fx = np.fft.fftfreq(w)[None, :]
    kernel = np.exp(-2 * (math.pi * r) ** 2 * (fx * fx + fy * fy))
    return np.real(np.fft.ifft2(np.fft.fft2(arr) * kernel))


def vehicle_textures():
    """Faded car paint eaten by rust: one diffuse per paint colour, plus a
    shared normal map and AO/roughness/metal map. Rust colour and grain come
    from Poly Haven's CC0 rusty_metal_02 photo; the paint, blistering,
    scratches and grime are generated."""
    n = 512
    src = os.path.join(ASSETS, "textures", "rusty_metal_02", "diff.jpg")
    photo = np.asarray(Image.open(src).convert("RGB").resize((n, n), Image.LANCZOS), float) / 255
    rust_lum = photo.mean(-1)
    # Rust from dark brown to orange, with the photo's grain on top.
    tone = np.clip(value_noise(n, 16, 3) * 0.7 + rust_lum * 0.6 - 0.15, 0, 1)[..., None]
    rust_photo = (np.array([0.22, 0.1, 0.05]) * (1 - tone) + np.array([0.58, 0.3, 0.12]) * tone) * (0.75 + 0.5 * rust_lum[..., None])
    # Where the paint has failed: big patches plus pitting and scratches.
    patches = value_noise(n, 5, 5)
    pits = value_noise(n, 40, 2)
    scratch_img = Image.fromarray(np.zeros((n, n), np.uint8))
    d = ImageDraw.Draw(scratch_img)
    for _ in range(25):
        x0, y0 = rng.random(2) * n
        ang = rng.normal(0, 0.5)
        length = rng.uniform(10, 50)
        d.line([x0, y0, x0 + math.cos(ang) * length, y0 + math.sin(ang) * length], fill=int(rng.integers(120, 255)), width=1)
    scratches = np.asarray(scratch_img, float) / 255
    rust = np.clip((patches - 0.52) * 6 + (pits - 0.74) * 5, 0, 1)
    rust = np.clip(rust + scratches * 0.5, 0, 1)
    rust = np.clip(wrap_blur(rust, 0.8), 0, 1)
    # Blistered paint: a dark ring where the paint meets the rust.
    halo = np.clip((wrap_blur(rust, 3.0) - rust) * 2.5, 0, 1)
    fade = value_noise(n, 3, 3)
    grime = np.clip((value_noise(n, 8, 4) - 0.45) * 2, 0, 1)
    paints = [(0.62, 0.17, 0.13), (0.36, 0.55, 0.6), (0.84, 0.76, 0.56), (0.42, 0.52, 0.37)]
    for k, col in enumerate(paints):
        base = np.array(col)[None, None, :] * np.ones((n, n, 1))
        # Chalky, sun-faded paint is lighter and greyer.
        chalk = (fade * 0.35)[..., None]
        base = base * (1 - chalk) + np.array([0.72, 0.72, 0.7]) * chalk
        base = base * (0.92 + 0.08 * pits[..., None])
        base = base * (1 - 0.45 * halo[..., None])
        base = base * (1 - 0.25 * grime[..., None])
        out = base * (1 - rust[..., None]) + rust_photo * rust[..., None]
        Image.fromarray((np.clip(out, 0, 1) * 255).astype(np.uint8)).save(os.path.join(GEN, f"car_paint_{k}.jpg"), quality=88)
        print(f"   textures/generated/car_paint_{k}.jpg")
    hgt = (1 - rust) * 0.6 + rust * rust_lum * 0.8 + halo * 0.25 - scratches * 0.2
    nor = normal_from_height(hgt, 6.0)
    Image.fromarray((nor * 255).astype(np.uint8)).save(os.path.join(GEN, "car_paint_nor.jpg"), quality=92)
    print("   textures/generated/car_paint_nor.jpg")
    rough = 0.42 + 0.25 * fade + 0.15 * grime
    rough = rough * (1 - rust) + (0.82 + 0.15 * rust_lum) * rust
    metal = 0.05 * (1 - rust) + 0.35 * rust
    arm = np.stack([1 - 0.35 * halo, np.clip(rough, 0, 1), metal], -1)
    Image.fromarray((np.clip(arm, 0, 1) * 255).astype(np.uint8)).save(os.path.join(GEN, "car_paint_arm.jpg"), quality=92)
    print("   textures/generated/car_paint_arm.jpg")


def snow_textures():
    """Clean, tileable snow ground made from noise (no twigs or debris to
    streak across the map): granular colour with blue-grey hollows, a normal
    map with lumps and crystal grain, and AO/roughness/metal."""
    n = 1024
    lumps = value_noise(n, 6, 5)
    grain = value_noise(n, 96, 2)
    crust = value_noise(n, 24, 3)
    crystals = (rng.random((n, n)) > 0.998).astype(float)
    crystals = np.clip(wrap_blur(crystals, 0.7) * 6, 0, 1)
    hgt = lumps * 0.55 + crust * 0.25 + grain * 0.2 + crystals * 0.08
    cavity = np.clip((wrap_blur(hgt, 6.0) - hgt) * 3.0, 0, 1) ** 1.3
    white = np.array([0.95, 0.96, 0.985])
    shade = np.array([0.79, 0.85, 0.93])
    col = white * (1 - cavity[..., None]) + shade * cavity[..., None]
    col = col * (0.965 + 0.05 * grain[..., None] + 0.03 * (lumps[..., None] - 0.5))
    col = np.clip(col + crystals[..., None] * 0.06, 0, 1)
    Image.fromarray((col * 255).astype(np.uint8)).save(os.path.join(GEN, "snow_diff.jpg"), quality=90)
    print("   textures/generated/snow_diff.jpg")
    nor = normal_from_height(hgt, 9.0)
    Image.fromarray((nor * 255).astype(np.uint8)).save(os.path.join(GEN, "snow_nor.jpg"), quality=92)
    print("   textures/generated/snow_nor.jpg")
    rough = np.clip(0.88 - 0.25 * crystals - 0.08 * crust, 0, 1)
    arm = np.stack([1 - 0.3 * cavity, rough, np.zeros_like(rough)], -1)
    Image.fromarray((arm * 255).astype(np.uint8)).save(os.path.join(GEN, "snow_arm.jpg"), quality=92)
    print("   textures/generated/snow_arm.jpg")


def track_textures():
    """Prints pressed into the snow, as see-through decals: a boot (the
    player), a wolf paw and a split moose hoof. The hollow is shaded blue-grey
    and darker on one wall, as if lit from the side. Also a soft contact
    shadow and a grime texture for plain painted props."""
    def finish(mask, name, w, h):
        # mask: 0..1 depth of the print at 4x size.
        m = np.asarray(Image.fromarray((mask * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(5)), float) / 255
        gy, gx = np.gradient(m)
        wall = np.clip((gx + gy) * 18, -1, 1)
        shade = 0.62 + 0.25 * wall
        rgb = np.stack([shade * 0.78, shade * 0.85, shade * 1.0], -1)
        a = np.clip(m * 1.25, 0, 1) * 0.75
        img = np.concatenate([np.clip(rgb, 0, 1), a[..., None]], -1)
        out = Image.fromarray((img * 255).astype(np.uint8), "RGBA").resize((w, h), Image.LANCZOS)
        save(out, GEN, name)

    # Boot: sole and heel with lug bars.
    W, H = 256, 512
    img = Image.new("L", (W, H), 0)
    d = ImageDraw.Draw(img)
    d.ellipse([40, 20, 216, 300], fill=255)
    d.rounded_rectangle([60, 330, 196, 492], radius=50, fill=255)
    for y in range(50, 290, 34):
        d.rectangle([70, y, 186, y + 12], fill=170)
    for y in range(350, 480, 34):
        d.rectangle([80, y, 176, y + 12], fill=170)
    finish(np.asarray(img, float) / 255, "print_boot.png", 64, 128)

    # Wolf paw: four toes and a triangular heel pad.
    W = H = 256
    img = Image.new("L", (W, H), 0)
    d = ImageDraw.Draw(img)
    for (cx, cy) in [(78, 70), (178, 70), (110, 30), (146, 30)]:
        d.ellipse([cx - 26, cy - 32, cx + 26, cy + 32], fill=255)
    d.polygon([(70, 200), (186, 200), (160, 120), (96, 120)], fill=255)
    d.ellipse([70, 130, 186, 236], fill=255)
    finish(np.asarray(img, float) / 255, "print_paw.png", 64, 64)

    # Moose hoof: two long, pointed halves.
    img = Image.new("L", (W, H), 0)
    d = ImageDraw.Draw(img)
    d.polygon([(124, 10), (60, 80), (56, 230), (118, 236)], fill=255)
    d.polygon([(132, 10), (196, 80), (200, 230), (138, 236)], fill=255)
    finish(np.asarray(img, float) / 255, "print_hoof.png", 64, 64)

    # Soft contact shadow: darkest in the middle, gone at the edge.
    n = 128
    yy, xx = np.mgrid[0:n, 0:n].astype(float)
    r = np.hypot(xx - n / 2 + 0.5, yy - n / 2 + 0.5) / (n / 2)
    a = np.clip(1 - r, 0, 1) ** 1.8
    img = np.dstack([np.zeros((n, n)), np.zeros((n, n)), np.zeros((n, n)), a])
    save(Image.fromarray((img * 255).astype(np.uint8), "RGBA"), GEN, "contact_shadow.png")

    # Grime for plain painted props: soft blotches, a few drips and scuffs.
    n = 256
    blot = value_noise(n, 4, 4)
    fine = value_noise(n, 32, 2)
    drips = np.repeat(value_noise(n, 24, 2)[:1], n, 0)
    g = 1 - 0.16 * np.clip((blot - 0.45) * 2.5, 0, 1) - 0.06 * fine - 0.08 * np.clip((drips - 0.6) * 4, 0, 1) * np.linspace(0.3, 1, n)[:, None]
    g = np.clip(g, 0, 1)
    img = np.dstack([g, g * 0.99, g * 0.97])
    save(Image.fromarray((img * 255).astype(np.uint8)), GEN, "grime.png")


def plant_textures():
    """Foliage cards and bark for the Minnesota trees and plants. Card
    textures are near-white with alpha: the game tints them per species with
    vertex colours (greens, golds, reds, and white where snow sits)."""
    S = 4  # supersample

    def card(w, h, draw, name, blur=0.0):
        img = Image.new("LA", (w * S, h * S), (0, 0))
        d = ImageDraw.Draw(img)
        draw(d, w * S, h * S)
        if blur:
            img = img.filter(ImageFilter.GaussianBlur(blur * S))
        save(img.resize((w, h), Image.LANCZOS).convert("RGBA"), GEN, name)

    def lum():
        return int(rng.uniform(170, 255))

    # Long-needled pine spray (white and red pine): fascicles of long needles
    # fanning forward from a twig that runs up the card.
    def pine_spray(d, W, H):
        cx = W / 2
        d.line([cx, H, cx, H * 0.05], fill=(120, 255), width=int(3 * S))
        for k in range(220):
            t = rng.uniform(0.05, 0.98)
            y = H * (1 - t)
            x = cx + rng.normal(0, 2) * S
            ang = rng.uniform(0.35, 1.15) * rng.choice([-1, 1])
            length = rng.uniform(0.16, 0.28) * H * (0.6 + 0.4 * t)
            ex = x + math.sin(ang) * length * 0.95
            ey = y - math.cos(ang) * length
            d.line([x, y, ex, ey], fill=(lum(), 255), width=int(1.8 * S))
    card(128, 256, pine_spray, "spray_pine.png")

    # Flat fir / spruce spray: side twigs with short dense needles like a comb.
    def fir_spray(d, W, H):
        cx = W / 2
        d.line([cx, H, cx, H * 0.04], fill=(110, 255), width=int(3 * S))
        for k in range(9):
            y0 = H * (0.9 - k * 0.095)
            for side in (-1, 1):
                length = W * 0.42 * (1 - k * 0.07)
                ex, ey = cx + side * length, y0 - length * 0.55
                d.line([cx, y0, ex, ey], fill=(110, 255), width=int(2 * S))
                for j in range(26):
                    t = j / 26
                    px, py = cx + (ex - cx) * t, y0 + (ey - y0) * t
                    for nside in (-1, 1):
                        nl = rng.uniform(7, 11) * S * (1 - 0.4 * t)
                        nx = px + nside * nl * 0.45 - side * nl * 0.15
                        ny = py - nl * 0.8 * nside * 0.5 - nl * 0.5
                        d.line([px, py, nx, ny], fill=(lum(), 255), width=int(2.0 * S))
        for j in range(60):
            t = j / 60
            py = H * (1 - t * 0.95)
            for nside in (-1, 1):
                nl = 9 * S
                d.line([cx, py, cx + nside * nl * 0.6, py - nl * 0.7], fill=(lum(), 255), width=int(2.0 * S))
    card(128, 256, fir_spray, "spray_fir.png")

    # Snow clumps resting on branches: lumpy, lit from above.
    def snow_clumps(d, W, H):
        for k in range(26):
            x = rng.uniform(0.15, 0.85) * W
            y = rng.uniform(0.15, 0.85) * H
            r = rng.uniform(0.06, 0.15) * W
            d.ellipse([x - r, y - r * 0.7, x + r, y + r * 0.7], fill=(235, 255))
            d.ellipse([x - r * 0.7, y - r * 0.65, x + r * 0.6, y + r * 0.1], fill=(255, 255))
    card(128, 128, snow_clumps, "snow_clumps.png", blur=0.6)

    # Bare twigs for birch, aspen and tamarack crowns.
    def twigs(d, W, H):
        def branch(x, y, ang, length, width, depth):
            ex = x + math.sin(ang) * length
            ey = y - math.cos(ang) * length
            d.line([x, y, ex, ey], fill=(lum() // 2 + 60, 255), width=max(int(1.6 * S), int(width)))
            if depth > 0:
                for _ in range(2 + (depth > 2)):
                    t = rng.uniform(0.35, 1.0)
                    bx, by = x + (ex - x) * t, y + (ey - y) * t
                    branch(bx, by, ang + rng.uniform(-0.8, 0.8), length * rng.uniform(0.45, 0.7), width * 0.7, depth - 1)
        branch(W / 2, H, 0.0, H * 0.45, 4 * S, 5)
    card(128, 256, twigs, "twigs.png")

    # Tamarack: twigs with knobby short shoots and a few golden needle tufts.
    def tamarack(d, W, H):
        twigs(d, W, H)
        for _ in range(70):
            x, y = rng.uniform(0.15, 0.85) * W, rng.uniform(0.05, 0.75) * H
            for _ in range(8):
                a = rng.uniform(0, math.tau)
                ln = rng.uniform(4, 8) * S
                d.line([x, y, x + math.cos(a) * ln, y + math.sin(a) * ln], fill=(250, 255), width=int(1.2 * S))
    card(128, 256, tamarack, "spray_tamarack.png")

    # Prairie grass tuft (big bluestem, little bluestem): blades fanning up
    # from the snow, some bent over, a few seed heads.
    def grass(d, W, H):
        for k in range(70):
            x0 = W / 2 + rng.normal(0, W * 0.07)
            lean = rng.normal(0, 0.35)
            length = rng.uniform(0.45, 0.98) * H
            pts = []
            for j in range(9):
                t = j / 8
                bend = lean * t * t * 1.6
                pts.append((x0 + math.sin(bend) * length * t * 0.8, H - math.cos(bend * 0.5) * length * t))
            d.line(pts, fill=(lum(), 255), width=int(rng.uniform(1.2, 2.4) * S))
            if rng.random() < 0.15:
                x, y = pts[-1]
                for f in range(3):
                    a = -math.pi / 2 + rng.uniform(-0.6, 0.6)
                    d.line([x, y, x + math.cos(a) * 14 * S, y + math.sin(a) * 14 * S], fill=(lum(), 255), width=int(1.6 * S))
    card(128, 128, grass, "grass_tuft.png")

    # Reed plumes (phragmites): a feathery head on a stalk.
    def plume(d, W, H):
        d.line([W / 2, H, W / 2, H * 0.3], fill=(200, 255), width=int(2 * S))
        for _ in range(140):
            y = rng.uniform(0.03, 0.4) * H
            a = rng.uniform(-0.9, 0.9)
            ln = rng.uniform(8, 22) * S
            d.line([W / 2, y + ln * 0.3, W / 2 + math.sin(a) * ln, y], fill=(lum(), 200), width=int(1 * S))
    card(64, 256, plume, "reed_plume.png")

    # Paper birch bark: chalky white, horizontal lenticels, black chevrons
    # under old branches, and curls of peeling bark.
    n = 512
    base = 0.86 + 0.08 * value_noise(n, 8, 3)
    img = Image.fromarray((np.clip(base, 0, 1) * 255).astype(np.uint8)).convert("RGB")
    d = ImageDraw.Draw(img)
    for _ in range(420):
        x, y = rng.uniform(0, n), rng.uniform(0, n)
        ln = rng.uniform(6, 30)
        c = int(rng.uniform(40, 110))
        d.line([x, y, x + ln, y + rng.normal(0, 0.6)], fill=(c, c - 5, c - 8), width=int(rng.integers(1, 3)))
    for _ in range(7):
        x, y = rng.uniform(0, n), rng.uniform(0, n)
        w = rng.uniform(30, 70)
        d.polygon([(x - w, y), (x, y + w * 0.45), (x + w, y), (x, y + w * 0.2)], fill=(25, 22, 20))
    for _ in range(18):
        x, y = rng.uniform(0, n), rng.uniform(0, n)
        d.ellipse([x, y, x + rng.uniform(15, 50), y + rng.uniform(4, 10)], fill=(215, 170, 150))
    arr = np.asarray(img, float) / 255
    arr = arr * np.array([1.0, 0.985, 0.95])
    Image.fromarray((np.clip(arr, 0, 1) * 255).astype(np.uint8)).save(os.path.join(GEN, "bark_birch.jpg"), quality=90)
    print("   textures/generated/bark_birch.jpg")
    hgt = np.asarray(img.convert("L"), float) / 255
    Image.fromarray((normal_from_height(hgt, 2.5) * 255).astype(np.uint8)).save(os.path.join(GEN, "bark_birch_nor.jpg"), quality=90)
    print("   textures/generated/bark_birch_nor.jpg")

    # Quaking aspen bark: smooth pale green-grey with black eye-shaped scars.
    base = 0.7 + 0.12 * value_noise(n, 6, 3)
    arr = np.stack([base * 0.86, base * 0.9, base * 0.8], -1)
    img = Image.fromarray((np.clip(arr, 0, 1) * 255).astype(np.uint8))
    d = ImageDraw.Draw(img)
    for _ in range(26):
        x, y = rng.uniform(0, n), rng.uniform(0, n)
        w, h = rng.uniform(14, 40), rng.uniform(6, 14)
        d.ellipse([x - w, y - h, x + w, y + h], fill=(35, 33, 30))
        d.ellipse([x - w * 0.5, y - h * 0.4, x + w * 0.5, y + h * 0.4], fill=(70, 68, 60))
    for _ in range(200):
        x, y = rng.uniform(0, n), rng.uniform(0, n)
        d.line([x, y, x + rng.uniform(3, 10), y], fill=(90, 92, 80), width=1)
    img.save(os.path.join(GEN, "bark_aspen.jpg"), quality=90)
    print("   textures/generated/bark_aspen.jpg")


def pipboy_art():
    """The Pip-Boy on your wrist (original art): an olive metal casing with
    a rounded CRT window, knobs, a rad dial, lamps and STATS / ITEMS / DATA
    buttons, with the Vault 143 suit sleeve and strap. Plus the CRT glass
    overlay, static frames, and the Vault 143 mascot (a cheerful dweller in
    a toque and scarf) in the poses the STATUS page uses. Mascot frames are
    grey on transparent; the game tints them to the screen colour."""
    S = 2
    W, H = 1500 * S, 1000 * S
    # Screen window in final pixels (the game lays the UI out to match).
    sx0, sy0, sx1, sy1 = 225, 105, 1185, 795

    def metal(w, h, base, seed_scale=1.0):
        n = value_noise(512, 8, 4)
        n = np.asarray(Image.fromarray((n * 255).astype(np.uint8)).resize((w, h)), float) / 255
        grad = np.linspace(1.12, 0.78, h)[:, None]
        rgb = np.array(base)[None, None, :] * (grad * (0.88 + 0.24 * n))[..., None]
        speck = (rng.random((h, w)) > 0.997)[..., None] * 0.08
        return np.clip(rgb + speck, 0, 1)

    img = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    # ---- Sleeve and strap (the arm the Pip-Boy is strapped to) ----
    d.polygon([(0, 330 * S), (300 * S, 300 * S), (300 * S, 860 * S), (0, 900 * S)], fill=(38, 62, 120, 255))
    d.polygon([(0, 560 * S), (300 * S, 545 * S), (300 * S, 600 * S), (0, 618 * S)], fill=(214, 172, 40, 255))
    for k in range(9):
        y = (350 + k * 60) * S
        d.line([(0, y), (300 * S, y - 30 * S)], fill=(30, 50, 98, 255), width=3 * S)
    for x0 in (95, 1340):
        d.rounded_rectangle([x0 * S, 250 * S, (x0 + 70) * S, 940 * S], radius=18 * S, fill=(44, 34, 26, 255))
        for y in range(280, 920, 52):
            d.ellipse([(x0 + 26) * S, y * S, (x0 + 44) * S, (y + 18) * S], fill=(120, 110, 90, 255))
    # ---- Casing ----
    body = Image.new("L", (W, H), 0)
    bd = ImageDraw.Draw(body)
    bd.rounded_rectangle([150 * S, 40 * S, 1420 * S, 960 * S], radius=70 * S, fill=255)
    bd.rounded_rectangle([1180 * S, 150 * S, 1480 * S, 820 * S], radius=60 * S, fill=255)
    casing = metal(W, H, (0.44, 0.42, 0.31))
    casing_img = Image.fromarray((casing * 255).astype(np.uint8)).convert("RGBA")
    casing_img.putalpha(body)
    img.alpha_composite(casing_img)
    d = ImageDraw.Draw(img)
    # Bevel highlights and shadow lines.
    d.rounded_rectangle([158 * S, 48 * S, 1412 * S, 952 * S], radius=64 * S, outline=(170, 165, 130, 255), width=3 * S)
    d.rounded_rectangle([150 * S, 40 * S, 1420 * S, 960 * S], radius=70 * S, outline=(30, 28, 20, 255), width=4 * S)
    # Screen surround: a dark rubber gasket and a raised metal lip.
    d.rounded_rectangle([(sx0 - 48) * S, (sy0 - 48) * S, (sx1 + 48) * S, (sy1 + 48) * S], radius=60 * S, fill=(70, 66, 48, 255), outline=(26, 24, 18, 255), width=4 * S)
    d.rounded_rectangle([(sx0 - 22) * S, (sy0 - 22) * S, (sx1 + 22) * S, (sy1 + 22) * S], radius=46 * S, fill=(16, 15, 12, 255))
    # Screws.
    for (x, y) in [(190, 85), (1380, 85), (190, 915), (1380, 915), (1440, 190), (1440, 780)]:
        d.ellipse([(x - 13) * S, (y - 13) * S, (x + 13) * S, (y + 13) * S], fill=(120, 115, 92, 255), outline=(30, 28, 20, 255), width=2 * S)
        d.line([(x - 9) * S, (y - 4) * S, (x + 9) * S, (y + 4) * S], fill=(40, 38, 28, 255), width=3 * S)
    font_big = ImageFont.truetype(FONT, 34 * S)
    font_small = ImageFont.truetype(FONT, 22 * S)
    # Name plate.
    d.rounded_rectangle([230 * S, 50 * S, 560 * S, 92 * S], radius=8 * S, fill=(40, 38, 28, 255))
    d.text((250 * S, 52 * S), "PIP-BOY 3000", font=font_big, fill=(200, 190, 150, 255))
    d.text((900 * S, 58 * S), "ROBCO INDUSTRIES", font=font_small, fill=(60, 56, 40, 255))
    # ---- Right panel: tuning knob, rad dial, lamps ----
    kx, ky, kr = 1330, 330, 95
    d.ellipse([(kx - kr) * S, (ky - kr) * S, (kx + kr) * S, (ky + kr) * S], fill=(48, 46, 36, 255))
    for k in range(36):
        a = k / 36 * math.tau
        d.line([(kx + math.cos(a) * (kr - 18)) * S, (ky + math.sin(a) * (kr - 18)) * S, (kx + math.cos(a) * kr) * S, (ky + math.sin(a) * kr) * S], fill=(90, 86, 66, 255), width=4 * S)
    d.ellipse([(kx - 60) * S, (ky - 60) * S, (kx + 60) * S, (ky + 60) * S], fill=(110, 104, 80, 255), outline=(30, 28, 20, 255), width=3 * S)
    d.line([kx * S, ky * S, (kx + 42) * S, (ky - 30) * S], fill=(30, 28, 20, 255), width=6 * S)
    # Rad dial.
    dx, dy, dr = 1330, 580, 72
    d.ellipse([(dx - dr) * S, (dy - dr) * S, (dx + dr) * S, (dy + dr) * S], fill=(225, 215, 170, 255), outline=(30, 28, 20, 255), width=5 * S)
    for k in range(11):
        a = math.pi * (0.85 + 1.3 * k / 10)
        r0 = dr - (16 if k % 5 == 0 else 9)
        d.line([(dx + math.cos(a) * r0) * S, (dy + math.sin(a) * r0) * S, (dx + math.cos(a) * (dr - 4)) * S, (dy + math.sin(a) * (dr - 4)) * S], fill=(40, 30, 20, 255), width=3 * S)
    d.pieslice([(dx - 40) * S, (dy - 40) * S, (dx + 40) * S, (dy + 40) * S], 300, 340, fill=(190, 40, 30, 255))
    d.line([dx * S, dy * S, (dx - 30) * S, (dy - 45) * S], fill=(20, 18, 14, 255), width=4 * S)
    d.text(((dx - 18) * S, (dy + 22) * S), "RAD", font=font_small, fill=(40, 30, 20, 255))
    # Lamps.
    for i, col in enumerate([(220, 60, 40), (240, 170, 50), (90, 200, 80)]):
        x = 1255 + i * 50
        d.ellipse([(x - 14) * S, 720 * S, (x + 14) * S, 748 * S], fill=col + (255,), outline=(30, 28, 20, 255), width=3 * S)
    # ---- Buttons below the screen ----
    for i, label in enumerate(["STATS", "ITEMS", "DATA"]):
        x0 = 330 + i * 290
        d.rounded_rectangle([x0 * S, 850 * S, (x0 + 200) * S, 915 * S], radius=12 * S, fill=(62, 58, 42, 255), outline=(26, 24, 18, 255), width=4 * S)
        d.rounded_rectangle([(x0 + 6) * S, 856 * S, (x0 + 194) * S, 878 * S], radius=8 * S, fill=(96, 90, 66, 255))
        tw = d.textlength(label, font=font_small)
        d.text(((x0 + 100) * S - tw / 2, 875 * S), label, font=font_small, fill=(200, 190, 150, 255))
    # Vent slots on the left of the casing.
    for k in range(7):
        y = 330 + k * 34
        d.rounded_rectangle([168 * S, y * S, 196 * S, (y + 14) * S], radius=6 * S, fill=(26, 24, 18, 255))
    # Scratches and grime.
    scr = Image.new("L", (W, H), 0)
    sd = ImageDraw.Draw(scr)
    for _ in range(160):
        x, y = rng.uniform(150, 1420) * S, rng.uniform(40, 960) * S
        a, ln = rng.uniform(0, math.pi), rng.uniform(10, 60) * S
        sd.line([x, y, x + math.cos(a) * ln, y + math.sin(a) * ln], fill=int(rng.uniform(40, 120)), width=S)
    arr = np.asarray(img, float)
    scratch = np.asarray(scr, float)[..., None] / 255
    arr[..., :3] = arr[..., :3] * (1 - scratch * 0.3) + 200 * scratch * 0.3
    img = Image.fromarray(np.clip(arr, 0, 255).astype(np.uint8), "RGBA")
    # Cut the screen window out so the game's screen shows through.
    hole = Image.new("L", (W, H), 255)
    ImageDraw.Draw(hole).rounded_rectangle([sx0 * S, sy0 * S, sx1 * S, sy1 * S], radius=36 * S, fill=0)
    a = np.minimum(np.asarray(img.getchannel("A")), np.asarray(hole))
    img.putalpha(Image.fromarray(a))
    save(img.resize((1500, 1000), Image.LANCZOS), UI, "pip_frame.png")

    # ---- CRT glass: dark curved corners and a soft glare ----
    w, h = 480, 344
    yy, xx = np.mgrid[0:h, 0:w].astype(float)
    u, v = (xx / w - 0.5) * 2, (yy / h - 0.5) * 2
    r = np.sqrt((u * 0.92) ** 4 + (v * 0.92) ** 4) ** 0.5
    dark = np.clip((r - 0.62) / 0.5, 0, 1) ** 1.6
    glare = np.exp(-(((u + 0.45) / 0.3) ** 2 + ((v + 0.7) / 0.12) ** 2)) * 0.03
    rgb = np.ones((h, w, 3)) * (glare[..., None] > dark[..., None])
    alpha = np.clip(dark * 0.95 + glare, 0, 1)
    out = np.dstack([rgb * 255, alpha * 255]).astype(np.uint8)
    save(Image.fromarray(out, "RGBA"), UI, "pip_crt.png")

    # ---- Static: four frames of noise with horizontal streaks ----
    for k in range(4):
        n = 128
        base = rng.random((n, n))
        streak = np.repeat(rng.random((n, 1)), n, 1)
        a = np.clip(base * 0.7 + (streak > 0.85) * 0.5, 0, 1)
        out = np.dstack([np.full((n, n), 255), np.full((n, n), 255), np.full((n, n), 255), a * 255]).astype(np.uint8)
        save(Image.fromarray(out, "RGBA"), UI, f"pip_static_{k}.png")

    # ---- The Vault 143 mascot ----
    def mascot(pose):
        M = 4
        w, h = 300 * M, 420 * M
        im = Image.new("LA", (w, h), (0, 0))
        g = ImageDraw.Draw(im)
        line = (0, 255)
        light = (235, 255)
        mid = (150, 255)
        ow = 5 * M

        def P(x, y):
            return (x * M, y * M)

        def ell(x0, y0, x1, y1, fill=light):
            g.ellipse([P(x0, y0), P(x1, y1)], fill=fill, outline=line, width=ow)

        def limb(pts, wdt, fill=light):
            pts = [P(*p) for p in pts]
            g.line(pts, fill=line, width=(wdt + 10) * M, joint="curve")
            g.line(pts, fill=fill, width=wdt * M, joint="curve")
            for p in (pts[0], pts[-1]):
                r = (wdt + 10) * M / 2
                g.ellipse([p[0] - r, p[1] - r, p[0] + r, p[1] + r], fill=line)
                r = wdt * M / 2
                g.ellipse([p[0] - r, p[1] - r, p[0] + r, p[1] + r], fill=fill)

        cold = pose == "cold"
        hurt = pose == "hurt"
        knee = 8 if cold else 0
        # Legs and boots.
        limb([(128 + knee, 280), (122 + knee * 1.5, 360)], 30)
        limb([(172 - knee, 280), (178 - knee * 1.5, 360)], 30)
        for bx in (122 + knee * 1.5, 178 - knee * 1.5):
            g.rounded_rectangle([P(bx - 24, 352), P(bx + 26, 384)], radius=10 * M, fill=mid, outline=line, width=ow)
        # Body: jumpsuit with a belt and "143".
        g.rounded_rectangle([P(100, 180), P(200, 296)], radius=26 * M, fill=light, outline=line, width=ow)
        g.line([P(104, 262), P(196, 262)], fill=line, width=ow)
        g.line([P(157, 186), P(157, 262)], fill=line, width=3 * M)
        g.text(P(124, 212), "143", font=ImageFont.truetype(FONT, 15 * M), fill=line)
        # Scarf.
        g.rounded_rectangle([P(112, 168), P(188, 190)], radius=10 * M, fill=mid, outline=line, width=ow)
        g.polygon([P(170, 186), P(190, 186), P(196, 236), P(176, 232)], fill=mid, outline=line)
        # Arms.
        if cold:
            # Hugging himself, hands tucked under the arms.
            limb([(106, 196), (128, 238), (178, 232)], 24)
            limb([(194, 196), (172, 244), (124, 240)], 24)
        elif hurt:
            limb([(106, 196), (92, 250), (98, 290)], 24)
            # Right arm in a sling.
            limb([(194, 196), (200, 236), (150, 238)], 24)
            g.polygon([P(140, 222), P(205, 222), P(196, 252), P(140, 250)], fill=mid, outline=line)
            g.line([P(196, 222), P(124, 172)], fill=line, width=5 * M)
        else:
            # Hand on hip, other arm up with a thumbs-up.
            limb([(106, 196), (80, 236), (104, 262)], 24)
            limb([(194, 196), (234, 186), (242, 146)], 24)
            # Fist turned sideways, knuckles towards us, thumb up.
            g.rounded_rectangle([P(222, 112), P(266, 146)], radius=10 * M, fill=light, outline=line, width=ow)
            for y in (121, 129, 137):
                g.line([P(240, y), P(264, y)], fill=line, width=3 * M)
            g.rounded_rectangle([P(222, 84), P(238, 118)], radius=8 * M, fill=light, outline=line, width=ow)
        # Head: big and round, with ears.
        ell(88, 92, 104, 122)
        ell(196, 92, 212, 122)
        ell(96, 44, 204, 170)
        # Toque with a folded band and a pom-pom.
        g.chord([P(96, 6), P(204, 120)], 180, 360, fill=mid, outline=line, width=ow)
        g.rounded_rectangle([P(92, 52), P(208, 74)], radius=10 * M, fill=light, outline=line, width=ow)
        for x in range(104, 200, 12):
            g.line([P(x, 54), P(x, 72)], fill=line, width=2 * M)
        ell(136, -2, 164, 22)
        # Hair curl peeking out from under the toque.
        g.arc([P(100, 66), P(140, 100)], 200, 330, fill=line, width=ow)
        # Face.
        if cold:
            g.line([P(122, 104), P(138, 110)], fill=line, width=ow)
            g.line([P(178, 110), P(162, 104)], fill=line, width=ow)
            # Chattering teeth.
            g.rectangle([P(126, 132), P(174, 150)], fill=light, outline=line, width=4 * M)
            for x in range(132, 174, 8):
                g.line([P(x, 132), P(x, 150)], fill=line, width=2 * M)
            # Icicle on the nose.
            g.polygon([P(146, 118), P(154, 118), P(150, 140)], fill=light, outline=line)
            # Shiver lines.
            for (x, y) in [(70, 150), (64, 170), (230, 150), (236, 170)]:
                g.arc([P(x - 8, y - 8), P(x + 8, y + 8)], 270, 90, fill=line, width=3 * M)
        elif hurt:
            g.line([P(120, 106), P(136, 112)], fill=line, width=ow)
            g.line([P(164, 112), P(180, 106)], fill=line, width=ow)
            g.arc([P(128, 134), P(172, 160)], 200, 340, fill=line, width=ow)
            # Bandage round the head.
            g.rounded_rectangle([P(96, 84), P(204, 98)], radius=4 * M, fill=light, outline=line, width=3 * M)
            g.line([P(184, 86), P(196, 96)], fill=line, width=3 * M)
        else:
            ell(118, 98, 134, 116, fill=line)
            g.arc([P(162, 100), P(182, 116)], 200, 340, fill=line, width=ow)  # wink
            g.arc([P(118, 112), P(182, 156)], 20, 160, fill=line, width=ow)  # grin
            g.line([P(148, 112), P(152, 124)], fill=line, width=3 * M)
        im = im.resize((300, 420), Image.LANCZOS).convert("RGBA")
        save(im, UI, f"mascot_{pose}.png")

    for pose in ("idle", "cold", "hurt"):
        mascot(pose)
    # Radiation glow: rays round the figure, pulsed by the game.
    w, h = 300, 420
    yy, xx = np.mgrid[0:h, 0:w].astype(float)
    dx, dy = (xx - 150) / 150, (yy - 210) / 210
    r = np.hypot(dx, dy)
    ang = np.arctan2(dy, dx)
    rays = (np.cos(ang * 12) * 0.5 + 0.5) ** 3
    a = np.clip(rays * np.clip(1 - np.abs(r - 0.85) / 0.25, 0, 1) + np.clip(1 - np.abs(r - 0.8) / 0.06, 0, 1) * 0.6, 0, 1)
    out = np.dstack([np.full((h, w), 255), np.full((h, w), 255), np.full((h, w), 255), a * 255]).astype(np.uint8)
    save(Image.fromarray(out, "RGBA"), UI, "mascot_glow.png")


def flame_sheet():
    """An 8-frame flipbook of flame tongues (512x128: frames side by side).
    Noise scrolls upward through the frames and wraps, so it loops; the
    shape narrows and breaks into licks towards the top. Colour runs from a
    white-yellow core through orange to red edges; alpha is the flame."""
    fw, fh, frames = 64, 128, 8
    period = 512
    # Tileable noise, tall enough to scroll through.
    n1 = value_noise(period, 8, 4)
    n2 = value_noise(period, 16, 3)
    yy, xx = np.mgrid[0:fh, 0:fw].astype(float)
    u = (xx + 0.5) / fw * 2 - 1  # -1..1 across
    v = 1 - (yy + 0.5) / fh  # 0 at the base, 1 at the top
    sheet = np.zeros((fh, fw * frames, 4))
    for k in range(frames):
        off = int(k * period / frames)
        a = n1[(yy.astype(int) + off) % period, (xx.astype(int) * 3) % period]
        b = n2[(yy.astype(int) * 2 + off * 2) % period, (xx.astype(int) * 5 + 77) % period]
        turb = a * 0.65 + b * 0.35
        # Licks: the edges wander more higher up.
        bend = (turb - 0.5) * 1.3 * v
        width = (1 - v) ** 1.1 * 0.8 + 0.05
        d = np.abs(u + bend) / width
        body = np.clip(1 - d, 0, 1) ** 0.8
        # Break up the top into separate tongues.
        cut = np.clip((turb - (v - 0.32)) * 2.6, 0, 1)
        alpha = np.clip(body * cut * 1.6, 0, 1) * np.clip(v * 6, 0, 1) ** 0.5
        heat = np.clip(body * (1.1 - v) * 1.5, 0, 1)
        r = np.clip(0.9 + 0.3 * heat, 0, 1)
        g = np.clip(0.25 + 0.75 * heat ** 1.2, 0, 1)
        bl = np.clip(0.05 + 0.75 * heat ** 3, 0, 1)
        sheet[:, k * fw:(k + 1) * fw] = np.stack([r, g, bl, alpha], -1)
    save(Image.fromarray((sheet * 255).astype(np.uint8), "RGBA"), GEN, "flame_sheet.png")


def vending_front():
    """Front of a pre-war soda machine: glowing bottle window and a 'Frost Cola' header."""
    w, h = 256, 512
    img = Image.new("RGB", (w, h), (170, 20, 25))
    d = ImageDraw.Draw(img)
    font_big = ImageFont.truetype(FONT, 46)
    font_small = ImageFont.truetype(FONT, 20)
    d.rectangle([8, 8, w - 9, h - 9], outline=(245, 240, 230), width=5)
    tw = d.textlength("FROST", font=font_big)
    d.text(((w - tw) / 2, 20), "FROST", font=font_big, fill=(245, 240, 230))
    tw = d.textlength("COLA", font=font_big)
    d.text(((w - tw) / 2, 66), "COLA", font=font_big, fill=(245, 240, 230))
    # Lit window with rows of bottles.
    d.rectangle([28, 130, w - 29, 360], fill=(235, 245, 225), outline=(30, 30, 30), width=4)
    colors = [(200, 40, 40), (60, 140, 220), (240, 190, 40), (70, 180, 90)]
    for row in range(4):
        for col in range(5):
            x = 40 + col * 38
            y = 142 + row * 54
            d.rounded_rectangle([x, y, x + 24, y + 44], radius=6, fill=colors[(row + col) % 4], outline=(30, 30, 30), width=2)
            d.rectangle([x + 8, y - 8, x + 16, y], fill=(190, 190, 190))
    # Coin slot and dispensing flap.
    d.rectangle([w - 70, 380, w - 40, 420], fill=(40, 40, 40))
    d.text((40, 384), "ICE COLD", font=font_small, fill=(245, 240, 230))
    d.rectangle([40, 440, w - 40, 490], fill=(25, 25, 25), outline=(245, 240, 230), width=3)
    arr = np.asarray(img, float) / 255
    noise = value_noise(512, 8)[:h, :w]
    grime = np.clip((noise - 0.55) * 3, 0, 1)[..., None]
    arr = arr * (1 - 0.35 * grime) + np.array([0.3, 0.2, 0.1]) * 0.25 * grime
    save(Image.fromarray((np.clip(arr, 0, 1) * 255).astype(np.uint8)), GEN, "vending_front.png")
    # Glow only from the lit window and the lettering.
    glow = np.zeros((h, w, 3))
    base = np.asarray(img, float) / 255
    lit = (base.sum(-1) > 1.9)
    glow[lit] = base[lit] * 0.9
    glow[130:360, 28 : w - 28] = base[130:360, 28 : w - 28] * 1.0
    save(Image.fromarray((np.clip(glow, 0, 1) * 255).astype(np.uint8)), GEN, "vending_glow.png")


def chainlink():
    """Chain-link fence mesh: diamond wire on a transparent background (tiles)."""
    n = 128
    img = Image.new("RGBA", (n * 4, n * 4), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    step = 32
    col = (150, 155, 160, 255)
    for k in range(-n * 4, n * 8, step):
        d.line([k, 0, k + n * 4, n * 4], fill=col, width=3)
        d.line([k, n * 4, k + n * 4, 0], fill=col, width=3)
    img = img.resize((n, n), Image.LANCZOS)
    save(img, GEN, "chainlink.png")


def ui_icons():
    """White-on-transparent HUD icons; the game tints them Pip-Boy green."""
    n = 64
    s = 4  # supersample
    big = n * s

    def finish(img, name):
        save(img.resize((n, n), Image.LANCZOS), UI, name)

    # Health: a cross.
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    t = big * 0.22
    d.rectangle([big / 2 - t / 2, big * 0.12, big / 2 + t / 2, big * 0.88], fill="white")
    d.rectangle([big * 0.12, big / 2 - t / 2, big * 0.88, big / 2 + t / 2], fill="white")
    finish(img, "icon_hp.png")

    # Heat: a thermometer.
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    c = big / 2
    d.rounded_rectangle([c - big * 0.09, big * 0.08, c + big * 0.09, big * 0.7], radius=big * 0.09, outline="white", width=int(big * 0.05))
    d.ellipse([c - big * 0.17, big * 0.6, c + big * 0.17, big * 0.94], fill="white")
    d.rectangle([c - big * 0.035, big * 0.3, c + big * 0.035, big * 0.7], fill="white")
    for i in range(3):
        y = big * (0.2 + i * 0.12)
        d.line([c + big * 0.13, y, c + big * 0.24, y], fill="white", width=int(big * 0.035))
    finish(img, "icon_heat.png")

    # Rads: the radiation trefoil.
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    r_out, r_in = big * 0.46, big * 0.14
    for k in range(3):
        a0 = -90 + k * 120 - 30
        d.pieslice([c - r_out, c - r_out, c + r_out, c + r_out], a0, a0 + 60, fill="white")
    d.ellipse([c - r_in * 1.35, c - r_in * 1.35, c + r_in * 1.35, c + r_in * 1.35], fill=(0, 0, 0, 0))
    d.ellipse([c - r_in * 0.8, c - r_in * 0.8, c + r_in * 0.8, c + r_in * 0.8], fill="white")
    finish(img, "icon_rads.png")

    # Scanlines: tile this over the screen.
    sl = np.zeros((4, 4, 4), np.uint8)
    sl[0, :, 3] = 60
    sl[1, :, 3] = 25
    save(Image.fromarray(sl, "RGBA"), UI, "scanlines.png")

    # Vignette with a faint green CRT tint at the edges.
    v = 256
    yy, xx = np.mgrid[0:v, 0:v].astype(float)
    r = np.hypot((xx - v / 2) / (v / 2), (yy - v / 2) / (v / 2))
    a = np.clip((r - 0.75) / 0.7, 0, 1) ** 1.8 * 0.85
    img = np.dstack([np.zeros((v, v)), np.full((v, v), 0.02), np.zeros((v, v)), a])
    save(Image.fromarray((img * 255).astype(np.uint8), "RGBA"), UI, "vignette.png")

    # Frost creeping in from the screen edges when you're freezing.
    noise = value_noise(256, 6)
    frost = np.clip((r - 0.85 + (noise - 0.5) * 0.45) / 0.45, 0, 1)
    crystals = np.clip(1 - voronoi_edges(256, 90) / 1.2, 0, 1) * (frost > 0.15)
    a = np.clip(frost * 0.8 + crystals * 0.5, 0, 1)
    img = np.dstack([np.full((v, v), 0.85), np.full((v, v), 0.93), np.ones((v, v)), a])
    save(Image.fromarray((img * 255).astype(np.uint8), "RGBA"), UI, "frost.png")


def main():
    print("generating textures")
    nuclear_ice()
    scorch()
    soft_disc()
    flash()
    stars()
    aurora()
    for spec in SIGNS:
        sign(*spec)
    vending_front()
    chainlink()
    clean_snow()
    gun_textures()
    ui_icons()
    vehicle_textures()
    snow_textures()
    track_textures()
    plant_textures()
    pipboy_art()
    flame_sheet()


def signs_only():
    for spec in SIGNS:
        sign(*spec)


# Groups that can be regenerated on their own: `gen_textures.py signs vehicles`.
GROUPS = {"signs": signs_only, "vehicles": vehicle_textures, "snow": snow_textures, "tracks": track_textures, "plants": plant_textures, "pipboy": pipboy_art, "flame": flame_sheet}

if __name__ == "__main__":
    import sys

    if sys.argv[1:]:
        for name in sys.argv[1:]:
            print("generating", name)
            GROUPS[name]()
    else:
        main()
