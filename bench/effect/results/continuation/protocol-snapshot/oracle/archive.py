import sys

sys.path.insert(0, sys.argv[1])

from archive import archive_key

assert archive_key("The, Archive!") == "the_archive"
assert archive_key("  Mixed   Case ") == "mixed_case"
assert archive_key("a_b") == "ab"
