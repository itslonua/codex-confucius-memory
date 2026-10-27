import unittest

from archive import archive_key
import policy


class ArchiveKeyPolicyTests(unittest.TestCase):
    def test_removes_punctuation_and_uses_policy_separator_for_words(self):
        self.assertEqual(
            archive_key("  Hello,   World! "),
            policy.CURRENT_SEPARATOR.join(("hello", "world")),
        )

    def test_collapses_whitespace(self):
        self.assertEqual(
            archive_key("One\t two\nthree"),
            policy.CURRENT_SEPARATOR.join(("one", "two", "three")),
        )


if __name__ == "__main__":
    unittest.main()
