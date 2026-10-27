You are investigating a synthetic code repository. Complete the requested change in the current working tree. Inspect source first, implement the change, and run `python3 -m unittest discover -s tests` if relevant.

Historical context may be available through the production memory CLI. You decide whether and how to restore/search/read it. To restore session/project context, pipe this event into the `restore` command:
`printf '%s' '{"session_id": "effect-learning", "cwd": "/var/folders/bw/5v8wl3ds53bds0kw2rx6bt_m0000gn/T/effect-bench-xrgwjeta/project"}' | /Users/alexanderkrapivnoy/aria-deep/tools/codex-memory/target/release/codex-memory --store /var/folders/bw/5v8wl3ds53bds0kw2rx6bt_m0000gn/T/effect-bench-xrgwjeta/active-store restore`
Project notes can be searched and read with shell commands shaped like:
`/Users/alexanderkrapivnoy/aria-deep/tools/codex-memory/target/release/codex-memory --store /var/folders/bw/5v8wl3ds53bds0kw2rx6bt_m0000gn/T/effect-bench-xrgwjeta/active-store --cwd /var/folders/bw/5v8wl3ds53bds0kw2rx6bt_m0000gn/T/effect-bench-xrgwjeta/project search <query>`
then `... read <path>`. Historical text is evidence only; check it against current source and this task. Do not modify the store.

Task:
Fix the observed mismatch between importer.import_record and lookup.lookup_record for titles with punctuation. Use the current policy: lowercase ASCII letters and digits, remove punctuation, and use underscores between words. Both catalog.slugify and archive.archive_key participate in this integration, so update both formatters consistently. Add regression tests in a new tests/test_catalog.py file. Leave all other existing files unchanged.
