import unittest

from archive import archive_key
from catalog import slugify
from importer import import_record
from lookup import lookup_record


class CatalogKeyTests(unittest.TestCase):
    def test_formatters_remove_punctuation_and_use_underscores(self):
        title = "  Sprocket, Widget!  Mark-II  "

        self.assertEqual(slugify(title), "sprocket_widget_mark_ii")
        self.assertEqual(archive_key(title), "sprocket_widget_mark_ii")

    def test_imported_punctuated_title_can_be_looked_up(self):
        title = "Sprocket, Widget!"
        records = {}

        self.assertEqual(import_record(title, records), "sprocket_widget")
        self.assertEqual(lookup_record(title, records), title)


if __name__ == "__main__":
    unittest.main()
