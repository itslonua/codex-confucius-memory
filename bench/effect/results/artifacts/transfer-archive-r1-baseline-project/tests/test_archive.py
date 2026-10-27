import unittest

from archive import archive_key


class ArchiveKeyTests(unittest.TestCase):
    def test_lowercases_words_and_replaces_punctuation_with_underscores(self):
        self.assertEqual(archive_key("The, Archive!"), "the_archive")

    def test_removes_punctuation_and_collapses_whitespace(self):
        self.assertEqual(archive_key("  Hello---  WORLD?!  "), "hello_world")

    def test_key_contains_only_ascii_letters_digits_and_underscores(self):
        self.assertEqual(archive_key("Café № 2"), "caf_2")


if __name__ == "__main__":
    unittest.main()
