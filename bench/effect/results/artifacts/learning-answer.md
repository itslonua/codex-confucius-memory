## Findings

Running the requested command, `python3 tests/reproduce_integration.py`, stops before exercising the lookup because it cannot import `importer` from the `tests/` script directory. Running it with the project root on the import path (`PYTHONPATH=. python3 tests/reproduce_integration.py`) reaches the integration assertion, which fails.

For title `"Sprocket, Widget!"`:

- [importer.py](/private/var/folders/bw/5v8wl3ds53bds0kw2rx6bt_m0000gn/T/effect-bench-xrgwjeta/project/importer.py:5), `import_record`, calls `catalog.slugify` and stores the record under its returned key.
- [catalog.py](/private/var/folders/bw/5v8wl3ds53bds0kw2rx6bt_m0000gn/T/effect-bench-xrgwjeta/project/catalog.py:4), `slugify`, lowercases and splits on whitespace, then joins with `"-"`. The stored key is **`sprocket,-widget!`**; punctuation remains.
- [lookup.py](/private/var/folders/bw/5v8wl3ds53bds0kw2rx6bt_m0000gn/T/effect-bench-xrgwjeta/project/lookup.py:5), `lookup_record`, calls `archive.archive_key` to retrieve the record.
- [archive.py](/private/var/folders/bw/5v8wl3ds53bds0kw2rx6bt_m0000gn/T/effect-bench-xrgwjeta/project/archive.py:4), `archive_key`, joins the same lowercase whitespace-split words with `"_"`. Its lookup key is **`sprocket,_widget!`**, which does not match the stored key.

The log in [ingestion.log](/private/var/folders/bw/5v8wl3ds53bds0kw2rx6bt_m0000gn/T/effect-bench-xrgwjeta/project/ingestion.log:1) records the stored key as `sprocket,-widget!`, but its miss entry reports lookup key **`sprocket_widget`**. That logged lookup key does not match the current `archive_key` output, suggesting the log reflects a different lookup implementation or earlier state.

[policy.py](/private/var/folders/bw/5v8wl3ds53bds0kw2rx6bt_m0000gn/T/effect-bench-xrgwjeta/project/policy.py:3) sets `CURRENT_SEPARATOR = "-"`. That matches the separator used by `catalog.slugify`, but **does not match** the actual lookup helper, which uses `"_"`. The cross-module failure is caused by importer and lookup deriving keys with different separators; the log’s `sprocket_widget` also differs from the current helper’s `sprocket,_widget!`.