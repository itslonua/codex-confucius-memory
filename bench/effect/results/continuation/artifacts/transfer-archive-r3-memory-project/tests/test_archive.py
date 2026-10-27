import unittest

from archive import archive_key


class ArchiveKeyTests(unittest.TestCase):
    def test_lowercases_and_removes_punctuation(self):
        self.assertEqual(archive_key("The, Archive!"), "the_archive")

    def test_uses_underscores_between_words(self):
        self.assertEqual(archive_key("  Hello   World  "), "hello_world")


if __name__ == "__main__":
    unittest.main()
