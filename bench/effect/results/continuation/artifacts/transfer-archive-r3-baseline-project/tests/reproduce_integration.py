from importer import import_record
from lookup import lookup_record

records = {}
import_record("Sprocket, Widget!", records)
assert lookup_record("Sprocket, Widget!", records) == "Sprocket, Widget!"
