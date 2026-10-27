import sys

sys.path.insert(0, sys.argv[1])

from catalog import slugify
from importer import import_record
from lookup import lookup_record

assert slugify("  Hello,   World!  ") == "hello_world"
assert slugify("R&D 2026") == "rd_2026"
assert slugify("already-clean") == "alreadyclean"
records = {}
import_record("Sprocket, Widget!", records)
assert lookup_record("Sprocket, Widget!", records) == "Sprocket, Widget!"
