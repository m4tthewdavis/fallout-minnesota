#!/usr/bin/env python3
"""Generate the game's procedural textures into assets/textures/generated and
assets/ui. Everything here is made from code (noise, Voronoi cracks, text), so
it is original work released with the project.

    pip install numpy pillow
    python3 tools/gen_textures.py
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


def sign(name, text_lines, size, bg, fg, border=None, rust=0.0):
    """Painted pre-war sign with rust and grime."""
    w, h = size
    img = Image.new("RGB", (w, h), bg)
    d = ImageDraw.Draw(img)
    if border:
        d.rectangle([6, 6, w - 7, h - 7], outline=border, width=8)
    y = h * 0.5 - len(text_lines) * h * 0.18
    for text, scale in text_lines:
        px = int(h * scale)
        font = ImageFont.truetype(FONT, px)
        while d.textlength(text, font=font) > w * 0.86:
            px -= 2
            font = ImageFont.truetype(FONT, px)
        tw = d.textlength(text, font=font)
        d.text(((w - tw) / 2, y), text, font=font, fill=fg)
        y += h * scale * 1.15
    arr = np.asarray(img, float) / 255
    noise = value_noise(max(w, h), 8)[:h, :w]
    stain = np.clip((noise - (1 - rust)) * 4, 0, 1)[..., None]
    arr = arr * (0.85 + 0.15 * noise[..., None])
    arr = arr * (1 - stain) + np.array([0.45, 0.25, 0.12]) * stain
    save(Image.fromarray((np.clip(arr, 0, 1) * 255).astype(np.uint8)), GEN, name)


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
    sign("sign_bullseye.png", [("BULLSEYE", 0.42), ("- MART -", 0.2)], (512, 256), (200, 20, 25), (245, 240, 230), (245, 240, 230), 0.35)
    sign("sign_mille_lacs.png", [("MILLE LACS 5", 0.34), ("VAULT 143  2", 0.34)], (512, 256), (30, 95, 50), (240, 240, 235), (240, 240, 235), 0.25)
    sign("sign_golden_atomic.png", [("GOLDEN ATOMIC MILLS", 0.42), ("Enriched Flour Since 1961", 0.18)], (1024, 256), (40, 35, 30), (230, 180, 40), None, 0.3)
    sign("sign_fallout_shelter.png", [("FALLOUT", 0.3), ("SHELTER", 0.3)], (256, 256), (230, 190, 30), (25, 25, 25), (25, 25, 25), 0.3)
    clean_snow()
    ui_icons()


if __name__ == "__main__":
    main()
