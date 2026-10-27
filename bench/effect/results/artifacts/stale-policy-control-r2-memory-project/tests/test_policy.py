import unittest

from archive import archive_key


class ArchiveKeyPolicyTests(unittest.TestCase):
    def test_removes_punctuation_and_joins_words_with_current_separator(self):
        self.assertEqual(archive_key("Sprocket, Widget!"), "sprocket_widget")

    def test_collapses_whitespace_and_strips_outer_punctuation(self):
        self.assertEqual(archive_key("  (Hello)   WORLD?  "), "hello_world")


if __name__ == "__main__":
    unittest.main()
