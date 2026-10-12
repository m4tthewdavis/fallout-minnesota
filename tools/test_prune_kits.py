#!/usr/bin/env python3
"""Checks tools/prune_kits.py on a tiny synthetic kit (no assets needed)."""

import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import prune_kits  # noqa: E402


def kit():
    """Three one-triangle pieces; piece "b" has a child "b_bolt"; uv2 on every piece."""
    buf = bytearray()
    views, accessors, meshes = [], [], []

    def add(data, fmt, ctype, typ, count):
        while len(buf) % 4:
            buf.append(0)
        views.append({"buffer": 0, "byteOffset": len(buf), "byteLength": len(data) * struct.calcsize(fmt)})
        buf.extend(b"".join(struct.pack("<" + fmt, *(d if isinstance(d, tuple) else (d,))) for d in data))
        accessors.append({"bufferView": len(views) - 1, "componentType": ctype, "type": typ, "count": count})
        return len(accessors) - 1

    for k in range(4):
        pos = add([(k, 0.0, 0.0), (k + 1.0, 0.0, 0.0), (k, 1.0, 0.0)], "3f", 5126, "VEC3", 3)
        nor = add([(0.0, 0.0, 1.0)] * 3, "3f", 5126, "VEC3", 3)
        uv0 = add([(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)], "2f", 5126, "VEC2", 3)
        uv2 = add([(0.5, 0.5)] * 3, "2f", 5126, "VEC2", 3)
        idx = add([0, 1, 2], "H", 5123, "SCALAR", 3)
        meshes.append({"primitives": [{"attributes": {"POSITION": pos, "NORMAL": nor, "TEXCOORD_0": uv0, "TEXCOORD_2": uv2}, "indices": idx, "material": k % 2}]})
    nodes = [
        {"name": "kit_a", "mesh": 0},
        {"name": "kit_b", "mesh": 1, "children": [3], "rotation": [0, 0.7071068, 0, 0.7071068]},
        {"name": "kit_c", "mesh": 2},
        {"name": "kit_b_bolt", "mesh": 3, "translation": [0, 1, 0]},
    ]
    g = {
        "asset": {"version": "2.0"},
        "scene": 0,
        "scenes": [{"nodes": [0, 1, 2]}],
        "nodes": nodes,
        "materials": [{"name": "m0"}, {"name": "glass"}],
        "meshes": meshes,
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{"uri": "kit.bin", "byteLength": len(buf)}],
    }
    return g, bytes(buf)


def test_keeps_named_nodes_children_and_materials():
    g, buf = kit()
    new_g, new_buf, maps = prune_kits.prune(g, buf, ["kit_b"], {"TEXCOORD_2"})
    assert [n["name"] for n in new_g["nodes"]] == ["kit_b", "kit_b_bolt"]
    assert new_g["nodes"][0]["children"] == [1]
    assert new_g["scenes"][0]["nodes"] == [0]
    assert new_g["materials"] == g["materials"], "material indices kept"
    assert [m["primitives"][0]["material"] for m in new_g["meshes"]] == [1, 1]
    assert all("TEXCOORD_2" not in m["primitives"][0]["attributes"] for m in new_g["meshes"])
    assert len(new_buf) < len(buf) and new_g["buffers"][0]["byteLength"] == len(new_buf)
    prune_kits.check(g, buf, new_g, new_buf, maps, {"TEXCOORD_2"})


def test_keep_all_only_strips_attributes_and_is_idempotent():
    g, buf = kit()
    new_g, new_buf, maps = prune_kits.prune(g, buf, None, {"TEXCOORD_2"})
    assert len(new_g["nodes"]) == 4
    prune_kits.check(g, buf, new_g, new_buf, maps, {"TEXCOORD_2"})
    again, again_buf, _ = prune_kits.prune(new_g, new_buf, None, {"TEXCOORD_2"})
    assert again == new_g and again_buf == new_buf


def test_refuses_to_orphan_a_kept_child():
    g, buf = kit()
    try:
        prune_kits.prune(g, buf, ["kit_b_bolt"], set())
    except SystemExit as e:
        assert "kept children" in str(e)
    else:
        raise AssertionError("dropping a kept node's parent must fail")


if __name__ == "__main__":
    for name, fn in list(globals().items()):
        if name.startswith("test_"):
            fn()
            print("ok", name)
