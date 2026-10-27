import unittest

from archive import archive_key
from catalog import slugify
from importer import import_record
from lookup import lookup_record


class CatalogKeyTests(unittest.TestCase):
    def test_formatters_remove_punctuation_and_use_underscores(self):
        title = "Sprocket, Widget!"

        self.assertEqual(slugify(title), "sprocket_widget")
        self.assertEqual(archive_key(title), "sprocket_widget")

    def test_imported_record_can_be_found_with_punctuation(self):
        records = {}
        title = "Sprocket, Widget!"

        self.assertEqual(import_record(title, records), "sprocket_widget")
        self.assertEqual(lookup_record(title, records), title)

    def test_only_ascii_letters_and_digits_are_kept(self):
        title = "Café—Gear 42!"

        self.assertEqual(slugify(title), "caf_gear_42")
        self.assertEqual(archive_key(title), "caf_gear_42")


if __name__ == "__main__":
    unittest.main()
