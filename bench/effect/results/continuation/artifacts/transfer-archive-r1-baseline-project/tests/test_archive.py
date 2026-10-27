import unittest

from archive import archive_key


class ArchiveKeyTests(unittest.TestCase):
    def test_lowercases_and_removes_punctuation_between_words(self):
        self.assertEqual(archive_key("The, Archive!"), "the_archive")

    def test_uses_ascii_word_characters_and_underscores(self):
        self.assertEqual(archive_key("  Café & ARCHIVE-2024  "), "caf_archive_2024")


if __name__ == "__main__":
    unittest.main()
