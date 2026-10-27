import unittest

from archive import archive_key


class ArchiveKeyTests(unittest.TestCase):
    def test_title_with_punctuation_becomes_underscored_words(self):
        self.assertEqual(archive_key("The, Archive!"), "the_archive")

    def test_collapses_whitespace_and_punctuation_between_words(self):
        self.assertEqual(archive_key("  Hello---  World!!  "), "hello_world")

    def test_result_uses_only_lowercase_ascii(self):
        self.assertEqual(archive_key("Café № 2"), "caf_2")


if __name__ == "__main__":
    unittest.main()
