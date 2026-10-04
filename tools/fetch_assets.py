#!/usr/bin/env python3
"""Download the CC0 Poly Haven models and textures the game uses into assets/.

Run from the repository root:

    python3 tools/fetch_assets.py

Every file comes from https://polyhaven.com (CC0, no attribution required, but
we credit the authors anyway in assets/CREDITS.md, which this script rewrites).
Already-downloaded files are skipped, so it is safe to run again.
"""

import json
import os
import sys
import urllib.request

API = "https://api.polyhaven.com"
UA = {"User-Agent": "fallout-minnesota-asset-fetch/1.0"}
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets")

# glTF models at 1k texture resolution: (Poly Haven id, what the game uses it for).
MODELS = [
    ("barrel_stove", "fire barrels at the fish-house shelters"),
    ("barrel_03", "scattered oil drums"),
    ("old_tyre", "junk around the wrecks"),
    ("rusted_wheel_rim_01", "junk around the wrecks"),
    ("metal_jerrycan", "scattered pre-war junk"),
    ("wooden_crate_02", "scattered supply crates"),
    ("old_military_crate", "military crates at the silos"),
    ("utility_box_01", "utility boxes on the power-line poles"),
    ("covered_car", "a tarp-covered car in the Bullseye-Mart lot"),
    ("ammo_box", "pipe-rifle ammo loot"),
    ("medical_box", "Stimpak loot"),
    ("can_rusted", "scattered rusted cans"),
    ("russian_food_cans_01", "hotdish ration tins (loot)"),
    ("rock_07", "boulders poking through the snow"),
    ("tree_stump_01", "cut stumps near the shelters"),
    ("stone_fire_pit", "abandoned campfires"),
    ("street_lamp_01", "lamp posts along the road and vault approach"),
    ("metal_toolbox", "tool chests at the shelter workbenches"),
    ("wooden_bucket_01", "ice-fishing buckets and camp gear"),
    ("tool_cart", "workbench carts at the shelters"),
    ("metal_trash_can", "dumpsters and bins around the Bullseye-Mart"),
    ("boombox", "a boombox left at an abandoned camp"),
    ("rusted_spade_01", "a spade left in the snow at camp"),
    ("worn_metal_rack", "shelving in the Bullseye-Mart ruin"),
]

# PBR texture sets at 1k: (id, maps, use). "arm" packs AO/roughness/metal, which
# matches glTF's occlusion + metallic-roughness channel layout.
TEXTURES = [
    ("pine_bark", ["Diffuse", "nor_gl"], "pine trunks"),
    ("rusty_metal_02", ["Diffuse", "nor_gl", "arm"], "wrecked cars"),
    ("rusty_corrugated_iron", ["Diffuse", "nor_gl", "arm"], "Golden Atomic Mills silos"),
    ("weathered_plank_siding", ["Diffuse", "nor_gl", "arm"], "fish-house walls"),
    ("cracked_concrete_wall", ["Diffuse", "nor_gl", "arm"], "Bullseye-Mart ruin"),
    ("rock_wall_02", ["Diffuse", "nor_gl", "arm"], "Vault 143 hillside"),
    ("metal_plate", ["Diffuse", "nor_gl", "arm"], "Vault 143 gear door"),
    ("asphalt_snow", ["Diffuse", "nor_gl", "arm"], "the old US-169 highway"),
]

MAP_SUFFIX = {"Diffuse": "diff", "nor_gl": "nor", "arm": "arm"}


def get_json(path):
    with urllib.request.urlopen(urllib.request.Request(API + path, headers=UA)) as r:
        return json.load(r)


def download(url, dest):
    if os.path.exists(dest) and os.path.getsize(dest) > 0:
        return
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    with urllib.request.urlopen(urllib.request.Request(url, headers=UA)) as r:
        data = r.read()
    with open(dest, "wb") as f:
        f.write(data)
    print(f"  {os.path.relpath(dest, ROOT)}  ({len(data) // 1024} KB)")


def authors(info):
    return ", ".join(info.get("authors", {}).keys()) or "Poly Haven"


def main():
    credits = []

    for asset_id, use in MODELS:
        print(asset_id)
        info = get_json(f"/info/{asset_id}")
        gltf = get_json(f"/files/{asset_id}")["gltf"]["1k"]["gltf"]
        folder = os.path.join(ROOT, "models", asset_id)
        download(gltf["url"], os.path.join(folder, f"{asset_id}.gltf"))
        for rel, f in gltf["include"].items():
            download(f["url"], os.path.join(folder, rel))
        credits.append((f"models/{asset_id}/", info["name"], asset_id, authors(info), use))

    for asset_id, maps, use in TEXTURES:
        print(asset_id)
        info = get_json(f"/info/{asset_id}")
        files = get_json(f"/files/{asset_id}")
        for m in maps:
            url = files[m]["1k"]["jpg"]["url"]
            download(url, os.path.join(ROOT, "textures", asset_id, f"{MAP_SUFFIX[m]}.jpg"))
        credits.append((f"textures/{asset_id}/", info["name"], asset_id, authors(info), use))

    write_credits(credits)


def write_credits(credits):
    path = os.path.join(ROOT, "CREDITS.md")
    lines = [
        "# Asset credits",
        "",
        "Every asset in this folder is either made for this project or released under",
        "**CC0 1.0** (public domain). No Fallout/Bethesda assets are used.",
        "",
        "## Downloaded (Poly Haven, CC0 1.0)",
        "",
        "Fetched by `tools/fetch_assets.py`. License: <https://polyhaven.com/license>.",
        "",
        "| Path | Asset | Source | Author(s) | Used for |",
        "| --- | --- | --- | --- | --- |",
    ]
    for path_, name, asset_id, who, use in credits:
        lines.append(f"| `{path_}` | {name} | <https://polyhaven.com/a/{asset_id}> | {who} | {use} |")
    lines.append("")
    marker = "<!-- generated-above -->"
    tail = ""
    if os.path.exists(path):
        old = open(path, encoding="utf-8").read()
        if marker in old:
            tail = old.split(marker, 1)[1]
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + marker + tail)
    print("wrote", os.path.relpath(path, ROOT))


if __name__ == "__main__":
    sys.exit(main())
