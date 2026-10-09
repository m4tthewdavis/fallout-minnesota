#!/usr/bin/env python3
"""Cut the heaviest CC0 scans down to game triangle budgets, in place.

Run from the repository root, after `tools/fetch_assets.py`:

    python3 tools/decimate_models.py            # decimate everything in TARGETS
    python3 tools/decimate_models.py --dry-run  # just print counts and budgets
    python3 tools/decimate_models.py boulder_01 # one model
    python3 tools/decimate_models.py --root=DIR # a copy of assets/models elsewhere

Poly Haven ships its scans at 30k-100k triangles each, which is fine for a
render but not for a dozen logs and boulders in view at once. Each model in
TARGETS is rewritten as `assets/models/<id>/<id>.gltf` + `.bin`, keeping its
nodes, materials and original textures (only the meshes change).

The simplifier is meshoptimizer's gltfpack (MIT, by Arseny Kapoulkine). It
collapses edges onto existing vertices, so UVs and normals are the scan's own,
it respects UV seams and normal creases, and it stops early rather than move
the surface more than the model's error limit (a fraction of its size), so a
model may end up under its budget's ratio but never misshapen. Found as
`$GLTFPACK`, then `gltfpack` on PATH (native builds:
https://github.com/zeux/meshoptimizer/releases), then `npx gltfpack@1.3.0`
(needs Node.js; a 350 KB WebAssembly build, no compiler).

Already decimated (written by gltfpack) or under budget: skipped, so running
it twice changes nothing. To
get an original back, delete the model's .gltf and .bin and run
`tools/fetch_assets.py` again (it only downloads missing files).
"""

import json
import os
import shutil
import subprocess
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "models")
GLTFPACK_VERSION = "1.3.0"

# id: (triangle budget, error limit as a fraction of the model's size, where it is used).
# The error limit wins over the budget: 0.01 = the surface moves at most 1% of the
# model's extent, i.e. under 2 cm on a boulder before it is scaled. Thin wires and
# cables need less (they kink or break up first).
TARGETS = {
    # Outdoors, many of each in view at once.
    "dead_tree_trunk": (8000, 0.01, "12 fallen logs (dressing.rs)"),
    "boulder_01": (10000, 0.01, "14 glacial boulders, scaled 1.4-3x (dressing.rs)"),
    "tree_stump_02": (6000, 0.01, "10 stumps in the woods (dressing.rs)"),
    "concrete_road_barrier": (3000, 0.01, "10 jersey barriers at the raider camps (dressing.rs)"),
    "rock_face_02": (10000, 0.01, "4 outcrops at the vault portal, scaled 2.2-2.6x (dressing.rs)"),
    "dry_branches_medium_01": (6000, 0.01, "14 deadfall piles (dressing.rs)"),
    "portable_generator": (10000, 0.01, "generators at the shelters and camps (dressing.rs)"),
    "rock_07": (5000, 0.01, "30 boulders, scaled 8-16x (nature.rs)"),
    "tree_stump_01": (6000, 0.01, "cut stumps at the 4 shelters (landmarks.rs)"),
    "street_lamp_01": (8000, 0.005, "11 lamp posts on the road and vault approach (props.rs)"),
    # Indoors.
    "old_military_compressor": (12000, 0.005, "2 compressors in the reactor room"),
    # Not vintage_radio_transceiver (42k, one on the Overseer's desk): its
    # millimetre-thin cables break up even at a 0.5% error limit.
    "Lantern_01": (8000, 0.003, "lanterns in the 4 fish houses (thin wire handle)"),
}


def triangles(g):
    """Triangles drawn by the default scene (meshes counted once per node that uses them)."""
    total = 0

    def walk(i):
        nonlocal total
        n = g["nodes"][i]
        if "mesh" in n:
            for p in g["meshes"][n["mesh"]]["primitives"]:
                acc = g["accessors"][p["indices"]] if "indices" in p else g["accessors"][p["attributes"]["POSITION"]]
                total += acc["count"] // 3
        for c in n.get("children", []):
            walk(c)

    for r in g["scenes"][g.get("scene", 0)]["nodes"]:
        walk(r)
    return total


def node_names(g):
    return sorted(n.get("name", "") for n in g["nodes"])


def primitive_count(g):
    return sum(len(g["meshes"][n["mesh"]]["primitives"]) for n in g["nodes"] if "mesh" in n)


def bounds(g):
    """Axis-aligned bounds of every POSITION accessor, in mesh space (enough to catch a lost part)."""
    lo, hi = [1e9] * 3, [-1e9] * 3
    for m in g["meshes"]:
        for p in m["primitives"]:
            a = g["accessors"][p["attributes"]["POSITION"]]
            lo = [min(x, y) for x, y in zip(lo, a["min"])]
            hi = [max(x, y) for x, y in zip(hi, a["max"])]
    return lo, hi


def material_images(g):
    """Each material's name and the image files its texture slots point at."""
    def uri(t):
        return g["images"][g["textures"][t["index"]]["source"]]["uri"]

    out = []
    for m in g.get("materials", []):
        slots = {}
        pbr = m.get("pbrMetallicRoughness", {})
        for k in ("baseColorTexture", "metallicRoughnessTexture"):
            if k in pbr:
                slots[k] = uri(pbr[k])
        for k in ("normalTexture", "occlusionTexture", "emissiveTexture"):
            if k in m:
                slots[k] = uri(m[k])
        out.append((m.get("name", ""), sorted(slots.items()), sorted(m.get("extensions", {}))))
    return sorted(out)


def find_gltfpack():
    if os.environ.get("GLTFPACK"):
        return [os.environ["GLTFPACK"]]
    if shutil.which("gltfpack"):
        return [shutil.which("gltfpack")]
    npx = shutil.which("npx") or shutil.which("npx.cmd")
    if npx:
        return [npx, "--yes", f"gltfpack@{GLTFPACK_VERSION}"]
    sys.exit("needs gltfpack: install Node.js (for npx) or put a gltfpack binary on PATH "
             "(https://github.com/zeux/meshoptimizer/releases), or set GLTFPACK=/path/to/gltfpack")


def restore_materials(before, after, model_id):
    """Put the original materials, textures and samplers back, exactly.

    gltfpack tidies materials as it goes (it turns alpha-blended materials
    whose textures have no alpha into opaque ones, for instance). Those may
    well be good changes, but this script only changes meshes: each primitive
    is pointed back at its original material, by name."""
    names = [m.get("name") for m in before.get("materials", [])]
    if len(set(names)) != len(names):
        raise RuntimeError(f"{model_id}: material names are not unique, can't map them back")
    for mesh in after["meshes"]:
        for p in mesh["primitives"]:
            if "material" in p:
                p["material"] = names.index(after["materials"][p["material"]].get("name"))
    for key in ("materials", "textures", "images", "samplers"):
        if key in before:
            after[key] = before[key]
        else:
            after.pop(key, None)
    used = set(after.get("extensionsUsed", [])) | set(before.get("extensionsUsed", []))
    if used:
        after["extensionsUsed"] = sorted(used)
    if before.get("extensionsRequired"):
        after["extensionsRequired"] = before["extensionsRequired"]


def check(before, after, model_id):
    """The decimated file must keep every node, primitive, material and texture, and its size."""
    problems = []
    if node_names(before) != node_names(after):
        problems.append("nodes changed")
    if primitive_count(before) != primitive_count(after):
        problems.append(f"primitives {primitive_count(before)} -> {primitive_count(after)} (a part vanished)")
    if material_images(before) != material_images(after):
        problems.append("materials or textures changed")
    (lo0, hi0), (lo1, hi1) = bounds(before), bounds(after)
    size = max(h - l for l, h in zip(lo0, hi0))
    drift = max(abs(a - b) for a, b in zip(lo0 + hi0, lo1 + hi1))
    if drift > 0.01 * size:
        problems.append(f"bounds moved {drift:.3f} m (size {size:.2f} m)")
    if problems:
        raise RuntimeError(f"{model_id}: " + "; ".join(problems))


def decimate(gltfpack, model_id, budget, error):
    folder = os.path.join(ROOT, model_id)
    path = os.path.join(folder, f"{model_id}.gltf")
    if not os.path.exists(path):
        print(f"{model_id:28s} not fetched, skipped")
        return
    with open(path, encoding="utf-8") as f:
        before = json.load(f)
    tris = triangles(before)
    if before.get("asset", {}).get("generator", "").startswith("gltfpack"):
        # Simplifying again would add a second error on top of the first.
        print(f"{model_id:28s} {tris:7d} triangles, already decimated")
        return
    if tris <= budget * 1.05:
        print(f"{model_id:28s} {tris:7d} triangles, within its budget of {budget}")
        return
    tmp = f"{model_id}.decimated"
    cmd = gltfpack + [
        "-i", f"{model_id}.gltf", "-o", f"{tmp}.gltf",
        "-noq",  # plain float attributes, no KHR_mesh_quantization
        "-vc", "16",  # vertex colours (rock_07 has them) as 16-bit, not 8-bit
        "-kn", "-km", "-ke",  # keep named nodes, materials and extras
        "-tr",  # keep pointing at the original textures
        "-si", f"{budget / tris:.5f}", "-se", f"{error}",
    ]
    subprocess.run(cmd, cwd=folder, check=True, stdout=subprocess.DEVNULL)
    try:
        with open(os.path.join(folder, f"{tmp}.gltf"), encoding="utf-8") as f:
            after = json.load(f)
        restore_materials(before, after, model_id)
        check(before, after, model_id)
        after["buffers"][0]["uri"] = f"{model_id}.bin"
        os.replace(os.path.join(folder, f"{tmp}.bin"), os.path.join(folder, f"{model_id}.bin"))
        with open(path, "w", encoding="utf-8") as f:
            json.dump(after, f, indent=1)
            f.write("\n")
    finally:
        for ext in (".gltf", ".bin"):
            p = os.path.join(folder, tmp + ext)
            if os.path.exists(p):
                os.remove(p)
    print(f"{model_id:28s} {tris:7d} -> {triangles(after):6d} triangles (budget {budget}, error {error})")


def main():
    global ROOT
    for a in sys.argv[1:]:
        if a.startswith("--root="):
            ROOT = a.split("=", 1)[1]
    args = [a for a in sys.argv[1:] if not a.startswith("-")]
    dry = "--dry-run" in sys.argv
    unknown = [a for a in args if a not in TARGETS]
    if unknown:
        sys.exit(f"not in TARGETS: {', '.join(unknown)}")
    ids = args or list(TARGETS)
    gltfpack = None if dry else find_gltfpack()
    for model_id in ids:
        budget, error, use = TARGETS[model_id]
        if dry:
            path = os.path.join(ROOT, model_id, f"{model_id}.gltf")
            tris = triangles(json.load(open(path, encoding="utf-8"))) if os.path.exists(path) else 0
            print(f"{model_id:28s} {tris:7d} triangles, budget {budget:6d}  {use}")
        else:
            decimate(gltfpack, model_id, budget, error)


if __name__ == "__main__":
    main()
