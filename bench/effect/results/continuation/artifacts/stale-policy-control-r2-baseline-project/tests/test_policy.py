import unittest

from archive import archive_key
from policy import CURRENT_SEPARATOR


class ArchiveKeyPolicyTests(unittest.TestCase):
    def test_uses_current_separator_for_spaces(self):
        self.assertEqual(archive_key("Hello World"), f"hello{CURRENT_SEPARATOR}world")

    def test_converts_punctuation_to_separator(self):
        self.assertEqual(archive_key("Hello, World!"), f"hello{CURRENT_SEPARATOR}world")

    def test_collapses_mixed_punctuation_and_whitespace(self):
        self.assertEqual(archive_key("  Hello...   World?!  "), f"hello{CURRENT_SEPARATOR}world")

    def test_leaves_alphanumeric_characters_intact(self):
        self.assertEqual(archive_key("Café 42"), f"café{CURRENT_SEPARATOR}42")


if __name__ == "__main__":
    unittest.main()
