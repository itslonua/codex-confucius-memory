import unittest

from archive import archive_key
from policy import CURRENT_SEPARATOR


class ArchiveKeyPolicyTests(unittest.TestCase):
    def test_uses_the_current_separator_for_whitespace(self):
        self.assertEqual(archive_key("  Hello\tWorld  "), f"hello{CURRENT_SEPARATOR}world")

    def test_removes_punctuation(self):
        self.assertEqual(archive_key("Sprocket, Widget!"), f"sprocket{CURRENT_SEPARATOR}widget")

    def test_normalizes_mixed_punctuation_and_whitespace(self):
        self.assertEqual(archive_key("  A—B:  C. "), f"ab{CURRENT_SEPARATOR}c")


if __name__ == "__main__":
    unittest.main()
