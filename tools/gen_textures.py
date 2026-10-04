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
    """Tileable fractal value noise in 0..1."""
    out = np.zeros((size, size))
    amp, total = 1.0, 0.0
    for o in range(octaves):
        c = cells * 2**o
        grid = rng.random((c, c))
        grid = np.vstack([grid, grid[:1]])
        grid = np.hstack([grid, grid[:, :1]])
        img = Image.fromarray((grid * 255).astype(np.uint8)).resize((size + size // c, size + size // c), Image.BICUBIC)
        out += amp * np.asarray(img, dtype=float)[:size, :size] / 255.0
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


def signs_only():
    for spec in SIGNS:
        sign(*spec)


# Groups that can be regenerated on their own: `gen_textures.py signs vehicles`.
GROUPS = {"signs": signs_only, "vehicles": vehicle_textures, "snow": snow_textures, "tracks": track_textures}

if __name__ == "__main__":
    import sys

    if sys.argv[1:]:
        for name in sys.argv[1:]:
            print("generating", name)
            GROUPS[name]()
    else:
        main()
