"""Regression tests for archive key normalization."""

import unittest

from archive import archive_key


class ArchiveKeyTests(unittest.TestCase):
    def test_uses_policy_separator_for_spaces(self):
        self.assertEqual(archive_key("Hello World"), "hello_world")

    def test_punctuation_becomes_word_separator(self):
        self.assertEqual(archive_key("Sprocket, Widget!"), "sprocket_widget")

    def test_collapses_mixed_punctuation_and_whitespace(self):
        self.assertEqual(archive_key("  Alpha---Beta \t Gamma  "), "alpha_beta_gamma")

    def test_empty_or_punctuation_only_title_is_empty(self):
        self.assertEqual(archive_key(" !!! "), "")


if __name__ == "__main__":
    unittest.main()
