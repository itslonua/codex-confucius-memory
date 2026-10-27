import unittest

from archive import archive_key
from catalog import slugify
from importer import import_record
from lookup import lookup_record


class CatalogKeyTests(unittest.TestCase):
    def test_formatters_remove_punctuation_and_use_underscores(self):
        title = "Sprocket, Widget!  Model-2"
        expected = "sprocket_widget_model2"

        self.assertEqual(slugify(title), expected)
        self.assertEqual(archive_key(title), expected)

    def test_imported_record_can_be_found_using_punctuated_title(self):
        title = "Sprocket, Widget!"
        records = {}

        self.assertEqual(import_record(title, records), "sprocket_widget")
        self.assertEqual(lookup_record(title, records), title)


if __name__ == "__main__":
    unittest.main()
