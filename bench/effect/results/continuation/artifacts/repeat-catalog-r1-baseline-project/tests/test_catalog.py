import unittest

from archive import archive_key
from catalog import slugify
from importer import import_record
from lookup import lookup_record


class CatalogFormatterTests(unittest.TestCase):
    def test_formatters_remove_punctuation_and_use_underscores(self):
        title = "  Sprocket, Widget! v2.0  "
        expected = "sprocket_widget_v20"

        self.assertEqual(slugify(title), expected)
        self.assertEqual(archive_key(title), expected)

    def test_imported_punctuated_title_can_be_looked_up(self):
        records = {}
        title = "Sprocket, Widget!"

        self.assertEqual(import_record(title, records), "sprocket_widget")
        self.assertEqual(lookup_record(title, records), title)


if __name__ == "__main__":
    unittest.main()
