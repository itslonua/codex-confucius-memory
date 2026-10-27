import unittest

from archive import archive_key
from policy import CURRENT_SEPARATOR


class ArchiveKeyPolicyTests(unittest.TestCase):
    def test_uses_current_separator_for_spaces_and_punctuation(self):
        self.assertEqual(archive_key("  Hello,   World!  "), f"hello{CURRENT_SEPARATOR}world")

    def test_collapses_runs_of_punctuation_and_spaces(self):
        self.assertEqual(archive_key("A---B...C"), f"a{CURRENT_SEPARATOR}b{CURRENT_SEPARATOR}c")

    def test_strips_leading_and_trailing_punctuation(self):
        self.assertEqual(archive_key("!?Title?!"), "title")


if __name__ == "__main__":
    unittest.main()
