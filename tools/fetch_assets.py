#!/usr/bin/env python3
"""Download the CC0 models and textures the game uses into assets/.

Run from the repository root:

    python3 tools/fetch_assets.py

Every file comes from https://polyhaven.com or https://ambientcg.com (both CC0,
no attribution required, but we credit the authors anyway in
assets/CREDITS.md, which this script rewrites). ambientCG sets are repacked
into the same diff/nor/arm layout as Poly Haven's (needs Pillow).
Already-downloaded files are skipped, so it is safe to run again.
"""

import io
import json
import os
import sys
import urllib.request
import zipfile

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
    # Milestone 10: furnishing the rooms.
    ("metal_office_desk", "the Overseer's desk"),
    ("drawer_cabinet", "filing cabinets in the vault lobby"),
    ("Television_01", "the Overseer's monitor"),
    ("vintage_radio_transceiver", "the vault's radio set"),
    ("SchoolChair_01", "chairs in the vault lobby"),
    ("caged_hanging_light", "caged lamps in the vault and the Bullseye-Mart"),
    ("mounted_fluorescent_lights", "strip lights in the vault and the Bullseye-Mart"),
    ("modular_airduct_rectangular_01", "air ducts in the vault lobby"),
    ("power_box_01", "breaker boxes on the vault and reactor walls"),
    ("old_military_compressor", "the coolant compressors in the reactor room"),
    ("vintage_spacecraft_instrument", "gauge panels on the reactor control desk"),
    ("modular_industrial_pipes_01", "pipe runs on the reactor walls"),
    ("hanging_industrial_lamp", "lamps over the reactor floor"),
    ("metal_tool_chest", "the tool chest at the reactor workbench"),
    ("Barrel_01", "red drums in the reactor room and at the silos"),
    ("old_gas_mask", "a gas mask hung by the reactor stair"),
    ("CashRegister_01", "tills on the Bullseye-Mart counters"),
    ("painted_wooden_shelves", "shelves in the Bullseye-Mart and the fish houses"),
    ("cardboard_box_01", "stock boxes in the Bullseye-Mart"),
    ("trashbag", "frozen rubbish bags"),
    ("long_life_food", "ration packs on shelves"),
    ("Lantern_01", "lanterns in the fish houses"),
    ("life_jacket", "life jackets hung in the fish houses"),
    ("Rockingchair_01", "rocking chairs in the fish houses"),
]

# PBR texture sets at 1k: (id, maps, use). "arm" packs AO/roughness/metal, which
# matches glTF's occlusion + metallic-roughness channel layout.
TEXTURES = [
    ("pine_bark", ["Diffuse", "nor_gl"], "pine trunks"),
    ("concrete_floor_worn_001", ["Diffuse", "nor_gl", "arm"], "Bullseye-Mart stockroom floor"),
    ("damaged_concrete_floor_02", ["Diffuse", "nor_gl", "arm"], "reactor room floor"),
    ("dirty_tiles", ["Diffuse", "nor_gl", "arm"], "Bullseye-Mart washroom tiles"),
    ("dark_wooden_planks", ["Diffuse", "nor_gl", "arm"], "fish-house floors"),
    ("metal_grate_rusty", ["Diffuse", "nor_gl", "arm"], "drain grates and catwalks on the reactor level"),
    ("rusty_metal_02", ["Diffuse", "nor_gl", "arm"], "wrecked cars"),
    ("rusty_corrugated_iron", ["Diffuse", "nor_gl", "arm"], "Golden Atomic Mills silos"),
    ("weathered_plank_siding", ["Diffuse", "nor_gl", "arm"], "fish-house walls"),
    ("cracked_concrete_wall", ["Diffuse", "nor_gl", "arm"], "Bullseye-Mart ruin"),
    ("rock_wall_02", ["Diffuse", "nor_gl", "arm"], "Vault 143 hillside"),
    ("metal_plate", ["Diffuse", "nor_gl", "arm"], "Vault 143 gear door"),
    ("asphalt_snow", ["Diffuse", "nor_gl", "arm"], "the old US-169 highway"),
    ("blue_metal_plate", ["Diffuse", "nor_gl", "arm"], "gun steel (worn painted metal)"),
    ("polar_fleece", ["Diffuse", "nor_gl", "arm"], "survivor and raider parkas"),
    ("curly_teddy_natural", ["Diffuse", "nor_gl", "arm"], "fur trim on hoods and cuffs"),
    ("wool_boucle", ["Diffuse", "nor_gl", "arm"], "knit beanies and scarves"),
    ("brown_leather", ["Diffuse", "nor_gl", "arm"], "gun slings, belts, boots and mittens"),
]

# ambientCG PBR sets: (id, resolution, use). Fetched as JPG zips and repacked.
AMBIENTCG = [
    ("Tiles140", "1K", "Vault 143 floor tiles"),
    ("PaintedMetal006", "1K", "green painted steel: vault lockers and machinery"),
    ("PaintedMetal016", "1K", "hazard stripes in the reactor room"),
    ("Concrete031", "1K", "concrete panel walls on the reactor level"),
    ("MetalPlates013", "1K", "riveted plating on the reactor core and machinery"),
    ("OfficeCeiling003", "1K", "drop ceiling in the vault lobby"),
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


def fetch_ambientcg(asset_id, res):
    """One ambientCG set, repacked: Color -> diff, NormalGL -> nor,
    AO/Roughness/Metalness -> arm (missing maps: AO 1, rough 0.78, metal 0),
    and Opacity -> opacity for cut-out sets."""
    from PIL import Image

    folder = os.path.join(ROOT, "textures", asset_id)
    if all(os.path.exists(os.path.join(folder, f"{m}.jpg")) for m in ("diff", "nor", "arm")):
        return
    url = f"https://ambientcg.com/get?file={asset_id}_{res}-JPG.zip"
    with urllib.request.urlopen(urllib.request.Request(url, headers=UA)) as r:
        z = zipfile.ZipFile(io.BytesIO(r.read()))
    maps = {}
    for name in z.namelist():
        for key in ("Color", "NormalGL", "AmbientOcclusion", "Roughness", "Metalness", "Opacity"):
            if name.endswith(f"_{key}.jpg"):
                maps[key] = Image.open(io.BytesIO(z.read(name)))
    os.makedirs(folder, exist_ok=True)
    size = maps["Color"].size

    def save(img, name):
        img.convert("RGB").save(os.path.join(folder, name), quality=90)
        print(f"  textures/{asset_id}/{name}")

    save(maps["Color"], "diff.jpg")
    save(maps["NormalGL"], "nor.jpg")
    grey = lambda key, fill: maps[key].convert("L").resize(size) if key in maps else Image.new("L", size, fill)
    save(Image.merge("RGB", (grey("AmbientOcclusion", 255), grey("Roughness", 199), grey("Metalness", 0))), "arm.jpg")
    if "Opacity" in maps:
        save(maps["Opacity"], "opacity.jpg")


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

    for asset_id, res, use in AMBIENTCG:
        print(asset_id)
        fetch_ambientcg(asset_id, res)
        credits.append((f"textures/{asset_id}/", asset_id, asset_id, "ambientCG (Lennart Demes)", use))

    write_credits(credits)


def write_credits(credits):
    path = os.path.join(ROOT, "CREDITS.md")
    lines = [
        "# Asset credits",
        "",
        "Every asset in this folder is either made for this project or released under",
        "**CC0 1.0** (public domain). No Fallout/Bethesda assets are used.",
        "",
        "## Downloaded (Poly Haven and ambientCG, CC0 1.0)",
        "",
        "Fetched by `tools/fetch_assets.py`. Licenses: <https://polyhaven.com/license>,",
        "<https://docs.ambientcg.com/license/>.",
        "",
        "| Path | Asset | Source | Author(s) | Used for |",
        "| --- | --- | --- | --- | --- |",
    ]
    for path_, name, asset_id, who, use in credits:
        if who.startswith("ambientCG"):
            source = f"<https://ambientcg.com/view?id={asset_id}>"
        else:
            source = f"<https://polyhaven.com/a/{asset_id}>"
        lines.append(f"| `{path_}` | {name} | {source} | {who} | {use} |")
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
