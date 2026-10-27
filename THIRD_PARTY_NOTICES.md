# Confucius provenance

Rust adaptation based on facebookresearch/cca-swebench commit
`bdeb74b3d828bfbb2c61030c6201fa9da44e69d0` (MIT; see LICENSE-META).

- `confucius/orchestrator/extensions/plan/llm.py`: planner trigger, first-message
  retention, assistant-boundary tail selection, replacement summary and first step.
- `confucius/orchestrator/extensions/plan/prompts.py`: six-part coding summary and
  next-step guidance, adapted in `prompts/prompt-summary.md`.
- `confucius/analects/note_taker/tasks.py`: selective discoveries, existing-note
  inspection/update, hierarchical topics and Markdown metadata; adapted in
  `prompts/prompt-notes.md`.
- `confucius/orchestrator/extensions/memory/hierarchical/{extension,types,utils}.py`:
  Markdown tree, frontmatter tags, collapsed directories, path/content/tag search,
  and write/edit operations.
- `confucius/orchestrator/extensions/file/utils.py`: numbered read ranges and
  whitespace-normalized exact replacements.

The original repository license and notices are retained. Runtime dependency
versions are pinned in `Cargo.lock`; their licenses are declared in the respective
packages. No dependency source is vendored.
The upstream fixture harness executes selected original functions, not the full
Confucius agent runtime.

## Adaptation boundaries

Codex hooks and the rollout reader replace Analect orchestration. Two Markdown
prompts are composed into one schema-constrained model call with summary and
scoped write/edit operations. Tool execution, arbitrary filesystem access and
unbounded repair retries are disabled in that worker. The original separate
note-taking agent is not reproduced. Token estimates use UTF-8 bytes/4 rather
than the upstream tokenizer. No native Codex compaction or rollout is rewritten.

Notes and summaries use Markdown, while cursor/retained-history state is machine
JSON. Notes are project-scoped rather than namespace/session-scoped. Locks,
private files, anchored Unix I/O and redo recovery belong to this adapter; they
are not guarantees inherited from Confucius's individual file writes. Legacy
JSON remains available during migration. The adapter rejects filesystem-equivalent
paths and session IDs with distinct spellings before publishing an update; it
does not silently rename them. Delete/import tools, periodic reminders, artifact
UI and arbitrary editor/command tools are not included.

## Compatibility limits

PyYAML and serde_yaml_ng differ on unquoted YAML 1.1 boolean/date-looking scalars
and duplicate keys. These differences affect parsed tags; use quoted strings and
unique keys. Generated tags are always quoted in a YAML-compatible JSON flow list.
Note-taker id/title/description/keywords metadata is preserved independently of
hierarchy tags, without automatic keyword conversion.

Content regex uses bounded fancy-regex matching. Recognized malformed Python
patterns retain literal-substring fallback; ambiguous or unsupported constructs
return errors. Some Python regex features and Unicode case behavior differ;
this is not a complete Python regex implementation. Path matching follows the
upstream extension removal and slash-crossing fnmatch behavior. Directory reads
list stored Markdown documents only. Edits preserve exact-match decisions and
whitespace normalization; fuzzy diagnostic suggestions are not reproduced.

No performance, token-saving or long-horizon benchmark improvement is claimed.
