import unittest

from archive import archive_key
from policy import CURRENT_SEPARATOR


class ArchiveKeyPolicyTests(unittest.TestCase):
    def test_uses_current_separator_for_spaces(self):
        self.assertEqual(archive_key("  Hello   World  "), f"hello{CURRENT_SEPARATOR}world")

    def test_normalizes_punctuation(self):
        self.assertEqual(archive_key("Sprocket, Widget!"), f"sprocket{CURRENT_SEPARATOR}widget")

    def test_collapses_adjacent_punctuation_and_spaces(self):
        self.assertEqual(archive_key("A--- B...C"), f"a{CURRENT_SEPARATOR}b{CURRENT_SEPARATOR}c")


if __name__ == "__main__":
    unittest.main()
