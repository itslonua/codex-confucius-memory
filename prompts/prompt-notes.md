# Persistent notes

Fill the `notes` array with new or updated persistent notes. Record ONLY discoveries that
required substantial debugging/trial-and-error, non-obvious connections across
components, or technical details expensive to rediscover. Do not document things
obvious from source code, standard practices, routine progress, secrets, credentials,
or personal data. Prefer zero notes to low-value notes. At most 8 changed notes.
Compare existing_notes first. Reuse the same id to correct/extend an existing note,
not a second id for the same insight. Return only new or updated notes, not copies
of unchanged notes. Each note needs concrete evidence (paths, commands, observed
results) from the trajectory. Keep notes specific, actionable and reproducible.
Use hierarchical ids such as projects/parser/utf8-boundary or shared/debugging/race.
IDs use alphanumerics, underscore, hyphen, and slash only. Project-specific notes
should be especially selective. All notes remain scoped to this workspace.
Each note has id, title, body, and a nonempty evidence array. Each text field is
1..6000 UTF-8 bytes. Entire JSON response must be under 24000 UTF-8 bytes.
