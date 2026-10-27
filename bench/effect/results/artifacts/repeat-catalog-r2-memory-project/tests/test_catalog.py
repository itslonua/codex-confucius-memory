import unittest

from archive import archive_key
from catalog import slugify
from importer import import_record
from lookup import lookup_record


class CatalogKeyTests(unittest.TestCase):
    def test_formatters_remove_punctuation_and_use_underscores(self):
        title = "  Sprocket, Widget! 42  "

        self.assertEqual(slugify(title), "sprocket_widget_42")
        self.assertEqual(archive_key(title), "sprocket_widget_42")

    def test_punctuation_is_removed(self):
        self.assertEqual(slugify("Don't-stop"), "dontstop")
        self.assertEqual(archive_key("Don't-stop"), "dontstop")

    def test_imported_record_can_be_looked_up_with_punctuation(self):
        records = {}
        title = "Sprocket, Widget!"

        import_record(title, records)

        self.assertEqual(lookup_record(title, records), title)


if __name__ == "__main__":
    unittest.main()
