import unittest

from archive import archive_key
from catalog import slugify
from importer import import_record
from lookup import lookup_record


class CatalogKeyTests(unittest.TestCase):
    def test_formatters_match_for_punctuation(self):
        title = "Sprocket, Widget!"

        self.assertEqual(slugify(title), "sprocket_widget")
        self.assertEqual(archive_key(title), "sprocket_widget")

    def test_imported_record_can_be_looked_up(self):
        title = "Sprocket, Widget!"
        records = {}

        import_record(title, records)

        self.assertEqual(lookup_record(title, records), title)

    def test_only_ascii_letters_and_digits_remain(self):
        title = " Crème brûlée: Model #42? "

        self.assertEqual(slugify(title), "crme_brle_model_42")
        self.assertEqual(archive_key(title), "crme_brle_model_42")


if __name__ == "__main__":
    unittest.main()
