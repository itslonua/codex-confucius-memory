# Summary

## Conversation context

```
The user requested a read-only investigation of a failing cross-module record lookup: run `python3 tests/reproduce_integration.py`, inspect `ingestion.log`, trace the key through `importer.py`, `catalog.py`, `lookup.py`, and `archive.py`, check `policy.py`, and report exact causes and values. They explicitly prohibited edits and fixes in the historical investigation. No newer corrections or scope changes are present.
```

## Technical decisions

```
Read-only investigation only; no implementation changes or fix proposals. The import-path failure was bypassed for diagnosis by setting `PYTHONPATH=.`.
```

## Implementation progress

```
The requested command failed before reaching the lookup because executing the script from `tests/` could not import `importer` (`ModuleNotFoundError`). Running `PYTHONPATH=. python3 tests/reproduce_integration.py` reached the assertion and failed with `AssertionError`. No project files were edited in the trajectory. Findings were reported with call sites and values.
```

## Technical details

```
For title `Sprocket, Widget!`, `importer.import_record` at `importer.py:5-8` calls `catalog.slugify` and stores the returned key. `catalog.slugify` at `catalog.py:4-6` lowercases and splits on whitespace, joining with `-`; punctuation remains, yielding `sprocket,-widget!`. `lookup.lookup_record` at `lookup.py:5-6` calls `archive.archive_key`; `archive_key` at `archive.py:4-5` uses the same lowercase whitespace split but joins with `_`, yielding `sprocket,_widget!`. Thus the stored and requested keys differ. `policy.py:3` sets `CURRENT_SEPARATOR = "-"`, matching the importer separator but not the actual lookup helper. `ingestion.log:1-2` records stored key `sprocket,-widget!` and lookup key `sprocket_widget`, which differs from the current archive helper output. The initial command result was `ModuleNotFoundError: No module named 'importer'`; the `PYTHONPATH=.` invocation reached line 6 of the reproduction script and raised `AssertionError`.
```

## Outstanding items

```
The log's lookup key `sprocket_widget` does not match the current helper's computed key `sprocket,_widget!`; the trajectory suggests the log may reflect a different or earlier implementation, but does not establish its provenance. No fix was requested.
```

## Next step

```
No follow-up action was requested; the investigation findings are complete. If reproducing, run `PYTHONPATH=. python3 tests/reproduce_integration.py` from the project root to reach the failing assertion.
```

