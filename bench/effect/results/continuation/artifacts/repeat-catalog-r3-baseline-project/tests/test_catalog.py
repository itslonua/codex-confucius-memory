import unittest

from archive import archive_key
from catalog import slugify
from importer import import_record
from lookup import lookup_record


class CatalogSlugTests(unittest.TestCase):
    def test_slugify_removes_punctuation_and_uses_underscores(self):
        self.assertEqual(slugify("  Sprocket, Widget!  "), "sprocket_widget")

    def test_archive_key_matches_catalog_slugify(self):
        title = "Rock & Roll: 101!"
        self.assertEqual(archive_key(title), slugify(title))
        self.assertEqual(archive_key(title), "rock_roll_101")

    def test_imported_punctuated_title_can_be_looked_up(self):
        title = "Sprocket, Widget!"
        records = {}

        self.assertEqual(import_record(title, records), "sprocket_widget")
        self.assertEqual(lookup_record(title, records), title)


if __name__ == "__main__":
    unittest.main()
