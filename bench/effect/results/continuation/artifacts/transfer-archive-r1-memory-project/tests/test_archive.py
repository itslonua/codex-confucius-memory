import unittest

from archive import archive_key


class ArchiveKeyTests(unittest.TestCase):
    def test_removes_punctuation_and_separates_words_with_underscores(self):
        self.assertEqual(archive_key("The, Archive!"), "the_archive")

    def test_collapses_whitespace_and_punctuation(self):
        self.assertEqual(archive_key("  A---B\tC  "), "a_b_c")

    def test_returns_ascii_lowercase(self):
        self.assertEqual(archive_key("CAFÉ Résumé"), "caf_rsum")


if __name__ == "__main__":
    unittest.main()
