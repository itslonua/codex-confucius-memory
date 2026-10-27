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

    def test_formatters_remove_internal_punctuation(self):
        title = "R&D's Model-7"

        self.assertEqual(slugify(title), "rds_model7")
        self.assertEqual(archive_key(title), "rds_model7")

    def test_imported_title_can_be_looked_up(self):
        records = {}
        title = "Sprocket, Widget!"

        import_record(title, records)

        self.assertEqual(lookup_record(title, records), title)


if __name__ == "__main__":
    unittest.main()
