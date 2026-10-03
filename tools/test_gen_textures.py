#!/usr/bin/env python3
"""Checks for the texture generator: every sign's lettering must sit inside
its painted border with a margin, at any size.

    python3 tools/test_gen_textures.py
"""

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_textures as g  # noqa: E402


class SignLayout(unittest.TestCase):
    def check(self, name, lines, size, border):
        inner, laid = g.layout_sign(lines, size, border)
        self.assertEqual(len(laid), len(lines))
        prev_bottom = -1.0
        for text, _font, _x, _y, ink in laid:
            with self.subTest(sign=name, text=text):
                self.assertGreaterEqual(ink[0], inner[0])
                self.assertLessEqual(ink[2], inner[2])
                self.assertGreaterEqual(ink[1], inner[1])
                self.assertLessEqual(ink[3], inner[3] + 0.5)
                # Lines must not overlap each other.
                self.assertGreater(ink[1], prev_bottom)
                prev_bottom = ink[3]

    def test_game_signs_fit(self):
        for name, lines, size, _bg, _fg, border, _rust in g.SIGNS:
            self.check(name, lines, size, border)

    def test_oversized_text_is_shrunk(self):
        self.check("huge", [("WELCOME TO", 0.6), ("MILLE LACS", 0.6), ("Pop. 1,143", 0.6)], (512, 256), (0, 0, 0))
        self.check("wide", [("A VERY LONG LINE OF TEXT THAT WILL NOT FIT", 0.4)], (256, 256), None)


if __name__ == "__main__":
    unittest.main()
