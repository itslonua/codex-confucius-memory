import unittest

from archive import archive_key
from policy import CURRENT_SEPARATOR


class ArchiveKeyPolicyTests(unittest.TestCase):
    def test_uses_current_policy_separator_for_spaces(self):
        self.assertEqual(archive_key("Hello World"), f"hello{CURRENT_SEPARATOR}world")

    def test_punctuation_becomes_a_separator(self):
        self.assertEqual(
            archive_key("Sprocket, Widget!"),
            f"sprocket{CURRENT_SEPARATOR}widget",
        )

    def test_repeated_spaces_and_punctuation_collapse(self):
        self.assertEqual(
            archive_key("  A---B,   C  "),
            f"a{CURRENT_SEPARATOR}b{CURRENT_SEPARATOR}c",
        )

    def test_empty_and_separator_only_titles_are_empty(self):
        self.assertEqual(archive_key(" \t...!!!  "), "")


if __name__ == "__main__":
    unittest.main()
