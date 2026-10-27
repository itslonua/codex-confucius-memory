import sys

sys.path.insert(0, sys.argv[1])

from archive import archive_key
from policy import CURRENT_SEPARATOR

assert CURRENT_SEPARATOR == "_"
assert archive_key("A, B!") == "a_b"
assert archive_key("A_B") == "ab"
