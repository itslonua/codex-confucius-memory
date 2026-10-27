---
id: key_mismatch
title: Cross-module Record Lookup Key Mismatch
description: Findings from the earlier integration reproduction in the synthetic project snapshot.
keywords:
  - record lookup
  - slug separator
  - import path
---

# Cross-module record lookup key mismatch

In the investigated snapshot, importing and lookup format record keys independently:

- `importer.import_record` calls `catalog.slugify` and stores under its returned key.
- `catalog.slugify` lowercases and whitespace-splits, then joins with `-`, retaining punctuation.
- `lookup.lookup_record` calls `archive.archive_key`.
- `archive.archive_key` uses `_` between whitespace-split words and also retains punctuation.

For `Sprocket, Widget!`, import stores `sprocket,-widget!`; the current lookup helper requests `sprocket,_widget!`. `policy.CURRENT_SEPARATOR` is `-`, matching the importer but not the actual lookup helper.

The `ingestion.log` miss reports `sprocket_widget`, which matches neither current formatter output and therefore appears to come from a different lookup implementation or earlier state.

Invocation detail: `python3 tests/reproduce_integration.py` failed before exercising the assertion with `ModuleNotFoundError: No module named 'importer'`. `PYTHONPATH=. python3 tests/reproduce_integration.py` reached the integration assertion and failed with `AssertionError`.
