import unittest

from archive import archive_key


class ArchiveKeyTests(unittest.TestCase):
    def test_lowercases_removes_punctuation_and_joins_words(self):
        self.assertEqual(archive_key("The, Archive!"), "the_archive")

    def test_normalizes_whitespace(self):
        self.assertEqual(archive_key("  The   Archive  "), "the_archive")


if __name__ == "__main__":
    unittest.main()
