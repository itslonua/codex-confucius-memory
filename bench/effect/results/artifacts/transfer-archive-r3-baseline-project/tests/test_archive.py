import unittest

from archive import archive_key


class ArchiveKeyTests(unittest.TestCase):
    def test_removes_punctuation_and_joins_words_with_underscores(self):
        self.assertEqual(archive_key("The, Archive!"), "the_archive")

    def test_keeps_only_lowercase_ascii_letters_and_digits(self):
        self.assertEqual(archive_key(" Café 2024?"), "caf_2024")


if __name__ == "__main__":
    unittest.main()
