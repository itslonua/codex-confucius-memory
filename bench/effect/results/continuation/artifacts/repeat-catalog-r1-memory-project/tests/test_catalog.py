import unittest

from archive import archive_key
from catalog import slugify
from importer import import_record
from lookup import lookup_record


class CatalogKeyTests(unittest.TestCase):
    def test_punctuation_titles_import_and_lookup_consistently(self):
        title = "Sprocket, Widget!"
        records = {}

        imported_key = import_record(title, records)

        self.assertEqual(imported_key, "sprocket_widget")
        self.assertEqual(lookup_record(title, records), title)

    def test_both_formatters_remove_punctuation_and_use_underscores(self):
        title = "  Crème brûlée: R&D's #1!  "

        self.assertEqual(slugify(title), "crme_brle_rds_1")
        self.assertEqual(archive_key(title), "crme_brle_rds_1")


if __name__ == "__main__":
    unittest.main()
