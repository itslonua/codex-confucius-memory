import unittest

from archive import archive_key
from catalog import slugify
from importer import import_record
from lookup import lookup_record


class CatalogFormattingTests(unittest.TestCase):
    def test_formatters_remove_punctuation_and_join_words_with_underscores(self):
        title = "  Sprocket, Widget!  MK-2  "

        self.assertEqual(slugify(title), "sprocket_widget_mk_2")
        self.assertEqual(archive_key(title), "sprocket_widget_mk_2")

    def test_formatters_keep_only_ascii_letters_and_digits(self):
        self.assertEqual(slugify("Crème brûlée № 5"), "cr_me_br_l_e_5")
        self.assertEqual(archive_key("Crème brûlée № 5"), "cr_me_br_l_e_5")

    def test_imported_punctuated_title_can_be_looked_up(self):
        title = "Sprocket, Widget!"
        records = {}

        self.assertEqual(import_record(title, records), "sprocket_widget")
        self.assertEqual(lookup_record(title, records), title)


if __name__ == "__main__":
    unittest.main()
