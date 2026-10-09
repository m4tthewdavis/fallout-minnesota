#!/usr/bin/env python3
"""Checks for tools/compress_textures.py (pure parts; the BC encoding test
is skipped when etcpak is not installed).

    python3 tools/test_compress_textures.py
"""

import os
import struct
import sys
import unittest

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import compress_textures as ct  # noqa: E402


def parse_ktx2(blob):
    assert blob[:12] == b"\xabKTX 20\xbb\r\n\x1a\n"
    vk, type_size, w, h, depth, layers, faces, levels, scheme = struct.unpack_from("<9I", blob, 12)
    dfd_off, dfd_len, kv_off, kv_len, sgd_off, sgd_len = struct.unpack_from("<IIIIQQ", blob, 48)
    index = [struct.unpack_from("<QQQ", blob, 80 + 24 * i) for i in range(levels)]
    return dict(vk=vk, w=w, h=h, faces=faces, levels=levels, scheme=scheme, dfd=(dfd_off, dfd_len), kv=(kv_off, kv_len), index=index)


class Chain(unittest.TestCase):
    def test_level_count_and_sizes(self):
        lv = ct.chain(np.full((128, 256, 4), 200, np.uint8), True, False)
        self.assertEqual(len(lv), 9)
        self.assertEqual([l.shape[:2] for l in lv[:3]], [(128, 256), (64, 128), (32, 64)])
        self.assertEqual(lv[-1].shape[:2], (1, 1))

    def test_flat_colour_stays_flat(self):
        for srgb in (True, False):
            lv = ct.chain(np.full((16, 16, 4), 200, np.uint8), srgb, False)
            self.assertTrue(all(np.abs(l.astype(int) - 200).max() <= 1 for l in lv))

    def test_srgb_averages_in_linear_light(self):
        # Same case as sim/mipmaps.rs srgb_averages_in_linear_light.
        px = np.array([[[255] * 3 + [255], [0, 0, 0, 255]], [[0, 0, 0, 255], [255] * 3 + [255]]], np.uint8)
        self.assertLessEqual(abs(int(ct.chain(px, True, False)[-1][0, 0, 0]) - 188), 2)
        self.assertEqual(int(ct.chain(px, False, False)[-1][0, 0, 0]), 128)

    def test_normal_mips_stay_unit_length(self):
        rng = np.random.default_rng(1)
        n = rng.normal(size=(32, 32, 3))
        n[..., 2] = np.abs(n[..., 2]) + 0.5
        n /= np.linalg.norm(n, axis=-1, keepdims=True)
        img = np.dstack([np.round((n * 0.5 + 0.5) * 255), np.full((32, 32), 255)]).astype(np.uint8)
        for l in ct.chain(img, False, True)[1:]:
            v = l[..., :3].astype(float) / 127.5 - 1.0
            self.assertLess(np.abs(np.linalg.norm(v, axis=-1) - 1.0).max(), 0.02)

    def test_rounds_half_away_from_zero_like_rust(self):
        self.assertEqual(float(ct.rnd(0.5)), 1.0)
        self.assertEqual(float(ct.rnd(2.5)), 3.0)

    def test_coverage_is_kept(self):
        # Same case as sim/mipmaps.rs thin_foliage_keeps_its_coverage_in_small_levels.
        img = np.zeros((64, 64, 4), np.uint8)
        for y in range(64):
            for x in range(0, 64, 4):
                img[y, x] = [255, 255, 255, 120 + (x * 7 + y * 13) % 136]
        lv = ct.chain(img, True, False)
        before = float(np.mean(lv[2][..., 3] / 255.0 > ct.COVERAGE_CUTOFF))
        ct.preserve_coverage(lv)
        after = float(np.mean(lv[2][..., 3] / 255.0 > ct.COVERAGE_CUTOFF))
        self.assertLess(before, 0.05)
        self.assertGreater(after, 0.15)


class Recolour(unittest.TestCase):
    def test_grey_keeps_alpha_and_brightness(self):
        px = np.array([[[180, 120, 70, 200], [30, 30, 30, 255]]], np.uint8)
        out = ct.recolour(px, 0.0)
        self.assertEqual(out[0, 0, 0], out[0, 0, 1])
        self.assertEqual(out[0, 0, 3], 200)
        self.assertEqual(list(out[0, 1]), [30, 30, 30, 255])
        self.assertTrue(70 < out[0, 0, 0] < 180)

    def test_greyed_list_is_read_from_library_rs(self):
        g = ct.greyed()
        self.assertTrue(g, "GREYED not found in src/library.rs")
        self.assertTrue(all(p.startswith(("models/", "textures/")) and p.endswith(".jpg") and 0.0 <= k <= 1.0 for p, k in g.items()))


class Roles(unittest.TestCase):
    def test_names(self):
        self.assertEqual(ct.role_from_name("textures/x/nor.jpg"), "normal")
        self.assertEqual(ct.role_from_name("boulder_01_nor_gl_1k.jpg"), "normal")
        self.assertEqual(ct.role_from_name("boulder_01_arm_1k.jpg"), "data")
        self.assertEqual(ct.role_from_name("gun_steel_arm.png"), "data")
        self.assertEqual(ct.role_from_name("portable_generator_spec_1k.jpg"), "data")
        self.assertEqual(ct.role_from_name("textures/LeafSet019/opacity.jpg"), "data")
        self.assertEqual(ct.role_from_name("textures/x/diff.jpg"), "colour")
        self.assertEqual(ct.role_from_name("snow_diff.jpg"), "colour")
        self.assertEqual(ct.role_from_name("spray_fir_photo.png"), "colour")


class Container(unittest.TestCase):
    def check(self, zstd):
        sizes = [(8, 8), (4, 4), (2, 2), (1, 1)]
        levels = [bytes([i]) * (max(1, (w + 3) // 4) * max(1, (h + 3) // 4) * 16) for i, (w, h) in enumerate(sizes)]
        blob = ct.ktx2(levels, "bc7", True, 8, 8, zstd)
        k = parse_ktx2(blob)
        self.assertEqual((k["vk"], k["w"], k["h"], k["faces"], k["levels"]), (146, 8, 8, 1, 4))
        self.assertEqual(k["scheme"], 2 if zstd else 0)
        dfd_off, dfd_len = k["dfd"]
        self.assertEqual(struct.unpack_from("<I", blob, dfd_off)[0], dfd_len)
        self.assertEqual(blob[dfd_off + 12], 134, "BC7 colour model")
        self.assertEqual(blob[dfd_off + 14], 2, "sRGB transfer")
        offsets = [o for o, _, _ in k["index"]]
        self.assertEqual(offsets, sorted(offsets, reverse=True), "smallest level stored first")
        for (off, length, raw), want in zip(k["index"], levels):
            self.assertEqual(raw, len(want))
            data = blob[off:off + length]
            if zstd:
                import zstandard

                data = zstandard.ZstdDecompressor().decompress(data, max_output_size=raw)
            else:
                self.assertEqual(off % 16, 0, "levels aligned to the block size")
            self.assertEqual(data, want)
        self.assertEqual(k["index"][0][0] + k["index"][0][1], len(blob), "largest level last")

    def test_plain(self):
        self.check(0)

    def test_zstd(self):
        try:
            import zstandard  # noqa: F401
        except ImportError:
            self.skipTest("zstandard not installed")
        self.check(10)

    def test_linear_bc5(self):
        blob = ct.ktx2([bytes(16)], "bc5", False, 4, 4, 0)
        k = parse_ktx2(blob)
        self.assertEqual(k["vk"], 141)
        self.assertEqual(blob[k["dfd"][0] + 14], 1, "linear transfer")


class Package(unittest.TestCase):
    def test_prune_keeps_generated_and_unpaired_jpgs_and_check_finds_holes(self):
        import json
        import tempfile

        with tempfile.TemporaryDirectory() as root:
            def touch(rel, data=b"x"):
                path = os.path.join(root, *rel.split("/"))
                os.makedirs(os.path.dirname(path), exist_ok=True)
                with open(path, "wb") as f:
                    f.write(data)

            ktx = ct.ktx2([bytes(16)], "bc7", True, 4, 4, 0)
            for rel in ["textures/a/diff.jpg", "textures/generated/snow_diff.jpg", "textures/generated/snow_diff.ktx2",
                        "textures/b/diff.jpg", "models/m/textures/m_diff.jpg"]:
                touch(rel, ktx if rel.endswith(".ktx2") else b"jpg")
            touch("textures/a/diff.ktx2", ktx)
            touch("models/m/textures/m_diff.ktx2", ktx)
            gltf = {"images": [{"uri": "textures/m_diff.ktx2"}, {"uri": "textures/gone.jpg"}]}
            touch("models/m/m.gltf", json.dumps(gltf).encode())
            self.assertEqual(ct.prune(root), 2)
            left = sorted(ct.rel_to_assets(os.path.join(d, n), root) for d, _, ns in os.walk(root) for n in ns if n.endswith(".jpg"))
            self.assertEqual(left, ["textures/b/diff.jpg", "textures/generated/snow_diff.jpg"])
            problems, counts = ct.check(root)
            self.assertEqual(counts["ktx2"], 3)
            self.assertEqual(problems, ["models/m/m.gltf: missing image textures/gone.jpg"])


class Encode(unittest.TestCase):
    def test_bc7_and_bc5_round_trip(self):
        try:
            import etcpak  # noqa: F401
            import texture2ddecoder  # noqa: F401
        except ImportError:
            self.skipTest("etcpak / texture2ddecoder not installed")
        rng = np.random.default_rng(3)
        img = np.repeat(np.repeat(rng.integers(0, 255, (4, 6, 4), dtype=np.uint8), 4, 0), 4, 1)  # 16x24 flat blocks
        img[..., 3] = 255
        for fmt, chans, tol in (("bc7", 3, 4), ("bc5", 2, 4)):
            dec = ct.decode(ct.encode(img, fmt, False), 24, 16, fmt)
            self.assertLessEqual(int(np.abs(dec[..., :chans].astype(int) - img[..., :chans]).max()), tol, fmt)
        # Odd sizes are padded to whole blocks.
        self.assertEqual(len(ct.encode(img[:6, :5], "bc7", False)), 2 * 2 * 16)


if __name__ == "__main__":
    unittest.main()
