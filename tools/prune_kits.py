#!/usr/bin/env python3
"""Strip what the game never uses out of the model kits, in place.

Run from the repository root, after `tools/fetch_assets.py` and
`tools/decimate_models.py`:

    python3 tools/prune_kits.py            # prune everything in PRUNE
    python3 tools/prune_kits.py --dry-run  # just print what would go
    python3 tools/prune_kits.py --root=DIR # a copy of assets/models elsewhere

Some Poly Haven models are kits: a dozen duct or pipe pieces in one file, of
which `library::Library::piece` spawns one or two by node name. Bevy still
loads, converts, computes tangents for and uploads every piece. This keeps
only the nodes the code names (and their children), the meshes they use, and
the accessors and buffer bytes those meshes read. Materials, textures, images
and samplers are all kept with their indices unchanged (`library::fix_glass`
finds glass by material index). Kept geometry is copied byte for byte.

It also drops vertex attributes Bevy ignores: Bevy 0.16 reads TEXCOORD_0 and
TEXCOORD_1 and warns about (then skips) any other TEXCOORD_n, so those are pure
load cost.

Before pruning a kit, every `"<kit>_..."` string in `src/` is collected, and
the tool refuses to drop a node the code names. After writing, it re-reads
the file and checks every kept primitive's attributes and indices against the
original, value for value. Running it twice changes nothing.
"""

import json
import os
import re
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "assets", "models")
SRC = os.path.join(HERE, "..", "src")

# id: (nodes to keep, or None for all; attribute names to drop, where used).
PRUNE = {
    "mounted_fluorescent_lights": (["mounted_fluorescent_lights_a"], [], "vault and mart strip lights (interiors.rs)"),
    "modular_airduct_rectangular_01": (["modular_airduct_rectangular_01_tripple_01"], [], "vault lobby ducts (interiors.rs)"),
    "modular_industrial_pipes_01": (
        ["modular_industrial_pipes_01_pipe02", "modular_industrial_pipes_01_pipe03", "modular_industrial_pipes_01_pipe05"],
        [],
        "reactor wall pipes (interiors.rs)",
    ),
    "vintage_spacecraft_instrument": (None, [f"TEXCOORD_{n}" for n in range(2, 12)], "reactor desk gauges (whole model)"),
}

COMPONENT = {5120: ("b", 1), 5121: ("B", 1), 5122: ("h", 2), 5123: ("H", 2), 5125: ("I", 4), 5126: ("f", 4)}
WIDTH = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT2": 4, "MAT3": 9, "MAT4": 16}


def names_in_src(kit):
    """Every string literal in src/ that starts with the kit's id and an underscore."""
    found = set()
    pat = re.compile(r'"(' + re.escape(kit) + r'_[A-Za-z0-9_.]+)"')
    for dirpath, _, files in os.walk(SRC):
        for f in files:
            if f.endswith(".rs"):
                with open(os.path.join(dirpath, f), encoding="utf-8") as fh:
                    found.update(pat.findall(fh.read()))
    return found


def read_accessor(g, buf, i):
    """The accessor's elements as tuples of numbers (exactly as stored)."""
    a = g["accessors"][i]
    if "sparse" in a or "bufferView" not in a:
        raise SystemExit(f"accessor {i}: sparse or view-less accessors are not supported")
    v = g["bufferViews"][a["bufferView"]]
    fmt, size = COMPONENT[a["componentType"]]
    n = WIDTH[a["type"]]
    elem = size * n
    stride = v.get("byteStride", elem)
    base = v.get("byteOffset", 0) + a.get("byteOffset", 0)
    return [struct.unpack_from("<" + fmt * n, buf, base + k * stride) for k in range(a["count"])]


def prim_accessors(p):
    out = list(p["attributes"].values())
    if "indices" in p:
        out.append(p["indices"])
    for t in p.get("targets", []):
        out.extend(t.values())
    return out


def vertex_count(g, mesh_ids):
    return sum(g["accessors"][p["attributes"]["POSITION"]]["count"] for m in mesh_ids for p in g["meshes"][m]["primitives"])


def prune(g, buf, keep_names, drop_attrs):
    """A new (gltf, bin) with only `keep_names` (and descendants) and without `drop_attrs`."""
    for key in ("skins", "animations", "cameras"):
        if g.get(key):
            raise SystemExit(f"{key} are not supported")
    for img in g.get("images", []):
        if "bufferView" in img:
            raise SystemExit("embedded images are not supported")
    nodes = g["nodes"]
    if keep_names is None:
        keep = set(range(len(nodes)))
    else:
        by_name = {n.get("name"): i for i, n in enumerate(nodes)}
        missing = [k for k in keep_names if k not in by_name]
        if missing:
            raise SystemExit(f"nodes not in the file: {missing}")
        keep, todo = set(), [by_name[k] for k in keep_names]
        while todo:
            i = todo.pop()
            if i not in keep:
                keep.add(i)
                todo.extend(nodes[i].get("children", []))
        # A kept node must not hang under a dropped one (it would lose its parent's transform).
        for i, n in enumerate(nodes):
            if i not in keep and any(c in keep for c in n.get("children", [])):
                raise SystemExit(f"node {nodes[i].get('name')} is dropped but has kept children")
    node_map = {old: new for new, old in enumerate(sorted(keep))}
    mesh_old = sorted({nodes[i]["mesh"] for i in keep if "mesh" in nodes[i]})
    mesh_map = {old: new for new, old in enumerate(mesh_old)}

    meshes = []
    for m in mesh_old:
        mesh = json.loads(json.dumps(g["meshes"][m]))
        for p in mesh["primitives"]:
            p["attributes"] = {k: v for k, v in p["attributes"].items() if k not in drop_attrs}
        meshes.append(mesh)
    acc_old = sorted({a for mesh in meshes for p in mesh["primitives"] for a in prim_accessors(p)})
    acc_map = {old: new for new, old in enumerate(acc_old)}
    view_old = sorted({g["accessors"][a]["bufferView"] for a in acc_old})
    view_map = {old: new for new, old in enumerate(view_old)}

    out = bytearray()
    views = []
    for v_old in view_old:
        v = dict(g["bufferViews"][v_old])
        if v.get("buffer", 0) != 0:
            raise SystemExit("only one buffer is supported")
        start = v.get("byteOffset", 0)
        data = buf[start : start + v["byteLength"]]
        while len(out) % 4:
            out.append(0)
        v["byteOffset"] = len(out)
        out.extend(data)
        views.append(v)
    while len(out) % 4:
        out.append(0)

    accessors = []
    for a_old in acc_old:
        a = dict(g["accessors"][a_old])
        a["bufferView"] = view_map[a["bufferView"]]
        accessors.append(a)
    for mesh in meshes:
        for p in mesh["primitives"]:
            p["attributes"] = {k: acc_map[v] for k, v in p["attributes"].items()}
            if "indices" in p:
                p["indices"] = acc_map[p["indices"]]
            for t in p.get("targets", []):
                for k in t:
                    t[k] = acc_map[t[k]]

    new_nodes = []
    for old in sorted(keep):
        n = dict(nodes[old])
        if "mesh" in n:
            n["mesh"] = mesh_map[n["mesh"]]
        if "children" in n:
            n["children"] = [node_map[c] for c in n["children"]]
        new_nodes.append(n)

    out_g = {}
    for key, value in g.items():  # keep the file's key order
        if key == "nodes":
            out_g[key] = new_nodes
        elif key == "meshes":
            out_g[key] = meshes
        elif key == "accessors":
            out_g[key] = accessors
        elif key == "bufferViews":
            out_g[key] = views
        elif key == "buffers":
            out_g[key] = [dict(value[0], byteLength=len(out))]
        elif key == "scenes":
            out_g[key] = [dict(s, nodes=[node_map[i] for i in s.get("nodes", []) if i in keep]) for s in value]
        else:
            out_g[key] = value
    return out_g, bytes(out), {"nodes": node_map, "meshes": mesh_map}


def check(old_g, old_buf, new_g, new_buf, maps, drop_attrs):
    """Every kept primitive reads back the same values; materials etc. untouched."""
    for key in ("materials", "textures", "images", "samplers"):
        assert old_g.get(key) == new_g.get(key), f"{key} changed"
    for old_n, new_n in maps["nodes"].items():
        a, b = old_g["nodes"][old_n], new_g["nodes"][new_n]
        assert {k: v for k, v in a.items() if k not in ("mesh", "children")} == {k: v for k, v in b.items() if k not in ("mesh", "children")}
    for old_m, new_m in maps["meshes"].items():
        pa, pb = old_g["meshes"][old_m]["primitives"], new_g["meshes"][new_m]["primitives"]
        assert len(pa) == len(pb)
        for p, q in zip(pa, pb):
            assert p.get("material") == q.get("material") and p.get("mode") == q.get("mode")
            want = {k: v for k, v in p["attributes"].items() if k not in drop_attrs}
            assert set(want) == set(q["attributes"]), (set(want), set(q["attributes"]))
            for k in want:
                assert read_accessor(old_g, old_buf, want[k]) == read_accessor(new_g, new_buf, q["attributes"][k]), k
                for f in ("componentType", "type", "normalized", "count"):
                    assert old_g["accessors"][want[k]].get(f) == new_g["accessors"][q["attributes"][k]].get(f), (k, f)
            if "indices" in p:
                assert read_accessor(old_g, old_buf, p["indices"]) == read_accessor(new_g, new_buf, q["indices"])


def main():
    args = sys.argv[1:]
    dry = "--dry-run" in args
    root = next((a.split("=", 1)[1] for a in args if a.startswith("--root=")), ROOT)
    for kit, (keep_names, drop_attrs, where) in PRUNE.items():
        gpath = os.path.join(root, kit, f"{kit}.gltf")
        if not os.path.exists(gpath):
            print(f"{kit}: missing, skipped")
            continue
        with open(gpath, encoding="utf-8") as f:
            g = json.load(f)
        bpath = os.path.join(root, kit, g["buffers"][0]["uri"])
        with open(bpath, "rb") as f:
            buf = f.read()
        if keep_names is not None:
            used = names_in_src(kit)
            names = {n.get("name") for n in g["nodes"]}
            lost = sorted((used & names) - set(keep_names))
            if lost:
                raise SystemExit(f"{kit}: src/ names {lost}, add them to PRUNE")
        new_g, new_buf, maps = prune(g, buf, keep_names, set(drop_attrs))
        before = (len(g["nodes"]), vertex_count(g, range(len(g["meshes"]))), len(buf))
        after = (len(new_g["nodes"]), vertex_count(new_g, range(len(new_g["meshes"]))), len(new_buf))
        if before == after and new_g == g:
            print(f"{kit}: nothing to prune")
            continue
        check(g, buf, new_g, new_buf, maps, set(drop_attrs))
        print(f"{kit} ({where}): nodes {before[0]} -> {after[0]}, vertices {before[1]} -> {after[1]}, .bin {before[2]} -> {after[2]} bytes")
        if dry:
            continue
        with open(bpath, "wb") as f:
            f.write(new_buf)
        with open(gpath, "w", encoding="utf-8") as f:
            json.dump(new_g, f, indent=1)
            f.write("\n")
        # Read back what was written and check it once more.
        with open(gpath, encoding="utf-8") as f:
            again = json.load(f)
        with open(bpath, "rb") as f:
            check(g, buf, again, f.read(), maps, set(drop_attrs))


if __name__ == "__main__":
    main()
