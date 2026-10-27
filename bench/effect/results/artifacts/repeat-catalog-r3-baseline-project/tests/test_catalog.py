import unittest

from archive import archive_key
from catalog import slugify
from importer import import_record
from lookup import lookup_record


class CatalogKeyTests(unittest.TestCase):
    def test_formatters_remove_punctuation_and_use_underscores(self):
        title = "  Sprocket, Widget! v2.0  "
        expected = "sprocket_widget_v20"

        self.assertEqual(slugify(title), expected)
        self.assertEqual(archive_key(title), expected)

    def test_punctuation_does_not_create_word_separators(self):
        self.assertEqual(slugify("rock-and-roll"), "rockandroll")
        self.assertEqual(archive_key("rock-and-roll"), "rockandroll")

    def test_imported_punctuated_title_can_be_looked_up(self):
        records = {}
        title = "Sprocket, Widget!"

        self.assertEqual(import_record(title, records), "sprocket_widget")
        self.assertEqual(lookup_record(title, records), title)


if __name__ == "__main__":
    unittest.main()
