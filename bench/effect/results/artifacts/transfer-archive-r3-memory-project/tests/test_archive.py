import unittest

from archive import archive_key


class ArchiveKeyTests(unittest.TestCase):
    def test_removes_punctuation_and_separates_words_with_underscores(self):
        self.assertEqual(archive_key("The, Archive!"), "the_archive")

    def test_lowercases_and_normalizes_whitespace(self):
        self.assertEqual(archive_key("  Hello   WORLD  "), "hello_world")


if __name__ == "__main__":
    unittest.main()
