import unittest

from archive import archive_key
from catalog import slugify
from importer import import_record
from lookup import lookup_record


class CatalogKeyTests(unittest.TestCase):
    def test_formatters_use_ascii_words_and_underscores(self):
        title = "  Sprocket, Widget!  2.0  "

        self.assertEqual(slugify(title), "sprocket_widget_2_0")
        self.assertEqual(archive_key(title), "sprocket_widget_2_0")

    def test_imported_record_can_be_looked_up_with_punctuation(self):
        title = "Sprocket, Widget!"
        records = {}

        self.assertEqual(import_record(title, records), "sprocket_widget")
        self.assertEqual(lookup_record(title, records), title)

    def test_non_ascii_characters_are_not_kept(self):
        self.assertEqual(slugify("Café—Widget"), "caf_widget")
        self.assertEqual(archive_key("Café—Widget"), "caf_widget")


if __name__ == "__main__":
    unittest.main()
