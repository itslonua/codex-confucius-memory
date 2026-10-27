import unittest

from archive import archive_key


class ArchiveKeyTests(unittest.TestCase):
    def test_lowercases_and_joins_words(self):
        self.assertEqual(archive_key("Hello World"), "hello_world")

    def test_removes_punctuation_between_words(self):
        self.assertEqual(archive_key("The, Archive!"), "the_archive")

    def test_collapses_separators_and_trims_edges(self):
        self.assertEqual(archive_key("  A... B__C  "), "a_b_c")

    def test_output_is_ascii(self):
        self.assertEqual(archive_key("Café 東京"), "caf")
