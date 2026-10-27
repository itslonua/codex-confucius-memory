import unittest

from archive import archive_key
from policy import CURRENT_SEPARATOR


class ArchiveKeyPolicyTests(unittest.TestCase):
    def test_uses_current_separator(self):
        self.assertEqual(archive_key("Hello World"), f"hello{CURRENT_SEPARATOR}world")

    def test_punctuation_and_spaces_share_a_separator(self):
        self.assertEqual(archive_key("Sprocket, Widget!"), f"sprocket{CURRENT_SEPARATOR}widget")

    def test_repeated_punctuation_and_whitespace_collapse(self):
        self.assertEqual(archive_key("  Alpha...  Beta?!  Gamma  "), f"alpha{CURRENT_SEPARATOR}beta{CURRENT_SEPARATOR}gamma")


if __name__ == "__main__":
    unittest.main()
