import unittest

from archive import archive_key
from policy import CURRENT_SEPARATOR


class ArchiveKeyPolicyTests(unittest.TestCase):
    def test_spaces_and_punctuation_use_current_separator(self):
        self.assertEqual(
            archive_key("  Hello, World!  "),
            CURRENT_SEPARATOR.join(("hello", "world")),
        )

    def test_repeated_separators_collapse_and_edges_are_removed(self):
        self.assertEqual(archive_key("...Hello---  World?!"), "hello_world")

    def test_empty_punctuation_title_becomes_empty(self):
        self.assertEqual(archive_key("?!..."), "")


if __name__ == "__main__":
    unittest.main()
