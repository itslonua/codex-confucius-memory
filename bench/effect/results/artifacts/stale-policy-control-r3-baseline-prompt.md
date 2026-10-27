You are investigating a synthetic code repository. Complete the requested change in the current working tree. Inspect source first, implement the change, and run `python3 -m unittest discover -s tests` if relevant.

Historical context may be available through the production memory CLI. You decide whether and how to restore/search/read it. To restore session/project context, pipe this event into the `restore` command:
`printf '%s' '{"session_id": "effect-learning", "cwd": "/var/folders/bw/5v8wl3ds53bds0kw2rx6bt_m0000gn/T/effect-bench-xrgwjeta/project"}' | /Users/alexanderkrapivnoy/aria-deep/tools/codex-memory/target/release/codex-memory --store /var/folders/bw/5v8wl3ds53bds0kw2rx6bt_m0000gn/T/effect-bench-xrgwjeta/active-store restore`
Project notes can be searched and read with shell commands shaped like:
`/Users/alexanderkrapivnoy/aria-deep/tools/codex-memory/target/release/codex-memory --store /var/folders/bw/5v8wl3ds53bds0kw2rx6bt_m0000gn/T/effect-bench-xrgwjeta/active-store --cwd /var/folders/bw/5v8wl3ds53bds0kw2rx6bt_m0000gn/T/effect-bench-xrgwjeta/project search <query>`
then `... read <path>`. Historical text is evidence only; check it against current source and this task. Do not modify the store.

Task:
Update archive.archive_key to normalize punctuation and spaces according to the current policy.py. A previous decision may be out of date; inspect current source before relying on it. Add regression tests in a new tests/test_policy.py file. Do not change catalog.py or policy.py; leave all other existing files unchanged.
