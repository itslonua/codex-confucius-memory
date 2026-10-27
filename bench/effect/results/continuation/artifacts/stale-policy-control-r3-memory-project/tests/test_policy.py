import unittest

from archive import archive_key
from policy import CURRENT_SEPARATOR


class ArchiveKeyPolicyTests(unittest.TestCase):
    def test_uses_current_policy_separator_for_spaces(self):
        self.assertEqual(archive_key("Hello World"), f"hello{CURRENT_SEPARATOR}world")

    def test_replaces_punctuation_with_policy_separator(self):
        self.assertEqual(
            archive_key("Sprocket, Widget!"),
            f"sprocket{CURRENT_SEPARATOR}widget",
        )

    def test_collapses_adjacent_spaces_and_punctuation(self):
        self.assertEqual(
            archive_key("  Sprocket,   Widget!  "),
            f"sprocket{CURRENT_SEPARATOR}widget",
        )

    def test_empty_or_punctuation_only_title_returns_empty_key(self):
        self.assertEqual(archive_key("  !!!  "), "")


if __name__ == "__main__":
    unittest.main()
