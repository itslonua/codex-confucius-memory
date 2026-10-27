import unittest

from archive import archive_key
from policy import CURRENT_SEPARATOR


class ArchiveKeyPolicyTests(unittest.TestCase):
    def test_uses_current_separator_for_spaces(self):
        self.assertEqual(archive_key("Hello World"), f"hello{CURRENT_SEPARATOR}world")

    def test_punctuation_uses_current_separator(self):
        self.assertEqual(
            archive_key("Sprocket, Widget!"),
            f"sprocket{CURRENT_SEPARATOR}widget",
        )

    def test_repeated_spaces_and_punctuation_make_one_boundary(self):
        self.assertEqual(
            archive_key("  Alpha --  Beta  "),
            f"alpha{CURRENT_SEPARATOR}beta",
        )


if __name__ == "__main__":
    unittest.main()
