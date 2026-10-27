import unittest

from archive import archive_key


class ArchiveKeyTests(unittest.TestCase):
    def test_removes_punctuation_and_separates_words(self):
        self.assertEqual(archive_key("The, Archive!"), "the_archive")

    def test_normalizes_case_and_whitespace(self):
        self.assertEqual(archive_key("  Hello\tWORLD  "), "hello_world")

    def test_uses_only_lowercase_ascii_characters(self):
        result = archive_key("Café — Archive")

        self.assertTrue(result.isascii())
        self.assertEqual(result, "caf_archive")


if __name__ == "__main__":
    unittest.main()
