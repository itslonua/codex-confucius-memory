# Setup and usage

Automatic context summaries and selective persistent notes, adapted from Meta's
Confucius planner and note-taker. The main agent does not need to write checkpoints.

## What is adapted

`memory.rs` implements the planner's threshold-driven compaction, initial-message
retention, assistant-boundary tail retention, complete structured summary, and
first next action. `prompts/prompt-summary.md` and `prompts/prompt-notes.md` contain the
summary and note-taking instructions: preserve requirements, decisions, failures, progress and
technical details; record only expensive-to-rediscover insights; update existing
topics instead of duplicating them.

Upstream uses a planner extension and a separate note-taking agent. This adaptation
combines the two Markdown instruction files into one schema-constrained Codex
request, producing a summary and scoped Markdown write/edit operations. Splitting
the files does not add a second model call. Both outputs are parsed before memory is changed, so a failed
request leaves that batch available for retry. Python/LangChain orchestration is
replaced by Rust. See
`THIRD_PARTY_NOTICES.md` for the pinned source revision and `LICENSE-META` for
upstream attribution. This is an adaptation of the memory workflow, not the full SDK.

## Lifecycle

1. `Stop` reads new completed rollout records and persists incremental task state.
   It invokes the model only when the input budget/message threshold is reached.
2. `PreCompact` also flushes pending history into a summary and selective notes.
3. `SessionStart(startup|resume|compact)` injects the task summary and a bounded
   project-note index. Full notes and directories are available through the
   injected read/search commands.
4. The summarizer runs in an isolated temporary directory using `codex exec
   --ignore-user-config --ephemeral`, a read-only sandbox, and disabled hooks,
   plugins, shell tools, multi-agent tools and skill search.

Hooks supplement native Codex compaction. They **cannot replace its algorithm or
edit its active message array**. First-message/tail retention applies to this
adaptation's own rolling summarizer input; the hook injects the compact summary, not
another full copy of retained tool output. No original rollout is rewritten.

Task state is isolated by canonical working directory and session ID. Project
notes are shared only within that directory, including files under `shared/`.
Different worktrees/subdirectories intentionally have separate memory scopes.
Memory is historical evidence, not instructions or authorization.

## Build and checks

Requires macOS or Linux, Rust >=1.88 and a Codex CLI supporting the flags in
`src/worker.rs`.
Validated with Codex CLI 0.161.0 on macOS. Uses existing Codex CLI authentication;
no extra API key or direct API implementation is required. Custom providers in
user config are not inherited by the isolated worker. `--model` overrides the
model; otherwise hooks pass their current model, then the CLI default is used.

```sh
# From the root of your codex-confucius-memory checkout:
make check
cargo build --release --locked
./target/release/codex-memory --help
```

## Install project hooks

Generate config **from the release binary**, so the command points to the release
binary's absolute path. The following assumes no existing project hooks file:

```sh
./target/release/codex-memory --codex /absolute/path/to/codex hooks > /tmp/confucius-hooks.json
# Review /tmp/confucius-hooks.json, then copy or merge its hooks into:
# <your-repository>/.codex/hooks.json
```

Do not overwrite other hooks. In Codex, open `/hooks` and review/trust the new
entries. Project-local hooks also need a trusted project configuration layer.
Configuration changes require trust review again. This tool never bypasses that
review. Disable/remove these three hook entries to uninstall; stored memory is
left available for recovery. Rebuild the release binary after source changes.

Default memory directory: `$CODEX_HOME/confucius-memory`, falling back to
`$HOME/.codex/confucius-memory`. Override with absolute `--store PATH`.
Each canonical working directory has a project hash. Documents use Markdown:

```text
<store>/<project-hash>/
  notes/shared/.../*.md
  notes/projects/.../*.md
  sessions/<session-id>/summary.md
  runtime.json
```

`runtime.json` holds continuation data (retained messages, cursor, pending state,
revision), not summary or note text. Notes are ordinary Markdown with metadata;
the hierarchy is reconstructed from their paths rather than a separate index.

New directories are private and data files use mode 0600. A project lock and a
redo journal coordinate document and runtime updates. Readers report a pending
recovery instead of serving a partial commit; the next writer retries recovery.
Do not remove a recovery journal or edit runtime data. A busy hook warns and
retries on a later lifecycle event without blocking the user's task.

Existing `<project-hash>.json` stores remain readable. The first successful
writer migrates them, retaining the original file and all session continuation
data. Read-only commands never trigger migration. The filesystem boundary uses
Unix directory handles and no-follow opens; Windows is not supported.

Before publishing an update or migration, storage rejects distinct note paths or
session IDs that the filesystem treats as the same name. This includes case and
Unicode-normalization collisions on filesystems that equate those spellings. It
also rejects conflicting spellings already present in the destination tree. A
rejected update leaves existing documents and the legacy JSON intact and does not
publish a recovery journal. Names are never silently renamed or lowercased;
resolve the conflicting names before retrying.

## Find and read saved notes

These commands do not call a model, read stdin, or modify memory:

```sh
./target/release/codex-memory --cwd /absolute/project search "UTF-8 slicing"
./target/release/codex-memory --cwd /absolute/project read projects/parser/utf8
```

`--cwd` defaults to the current directory. Use the exact working directory used
by the hooks; worktrees and subdirectories have separate memory. Keep `--store`
when your hooks use a custom store. SessionStart supplies commands with the
correct executable, store and directory already quoted.

Positional search requires all whitespace-separated words to occur in a path or
raw Markdown, ignoring case. Its JSON `matches` array contains at most 20
objects with a `path` field, in path order, alongside `total_matches` and
`truncated`; narrow a truncated search.

For the Confucius hierarchy filters:

```sh
./target/release/codex-memory --cwd /absolute/project search --path-pattern 'projects/parser/*' --content-pattern 'UTF-8|boundary' --tag rust --max-results 20
./target/release/codex-memory --cwd /absolute/project read projects/parser/utf8_boundary.md --start-line 1 --end-line 40
./target/release/codex-memory --cwd /absolute/project read '/'
```

Filters combine with AND. Repeating `--tag` requires every tag. Path glob matching
removes `.md` before matching, as the original implementation does; `*` can cross
`/`. The JSON `results` array contains `path`, `tags` and `content_preview`
fields and may include directories. `--max-results` defaults to 20; this interface
does not return a total count or truncation flag. Do not combine these filters
with a positional query.
Content regex is case-insensitive and has an execution budget. Some Python regex
features are unsupported and return an explicit error; use a simpler expression
or the literal search command. Recognized malformed patterns use the upstream
literal-substring fallback. See compatibility notes below.

Read returns `path` and `content`: numbered raw Markdown for a file, including
frontmatter, or a sorted one-level directory listing. `--end-line -1` means EOF.
Older IDs without `.md` still resolve to their Markdown file. Missing files are
errors; an empty project has no search results.

Install the bundled skill to teach Codex when to use these commands:

```sh
mkdir -p "${CODEX_HOME:-$HOME/.codex}/skills"
cp -R skills/confucius-memory "${CODEX_HOME:-$HOME/.codex}/skills/"
```

If a skill with that name already exists, review it before replacing it. Reload
skills or start a new Codex session to discover it. The skill uses the commands
provided by the hook, or `codex-memory` on PATH when no hook context is available.
It does not ask the main agent to write checkpoints.

## Manual smoke test

The `compress` and `restore` commands accept a single JSON object on stdin:

```sh
printf '%s\n' '{"session_id":"demo","cwd":"/absolute/workspace","transcript_path":"/absolute/rollout.jsonl"}' |
  ./target/release/codex-memory compress
printf '%s\n' '{"session_id":"demo","cwd":"/absolute/workspace"}' |
  ./target/release/codex-memory restore
```

`compress` failures return nonzero. `hook` failures return a `systemMessage`
warning with exit 0, preserving Codex's normal work. Model responses are parsed
before changing memory. Each successful summary is saved atomically with the
cursor of the history it covers. If a later generation fails or times out, the
next hook resumes from that saved prefix; the failed portion is not consumed.

## Privacy and model access

Rolling history includes user and assistant text and tool inputs/outputs. It is
stored on your machine alongside summaries and notes. Selected history and notes
are sent to the Codex model service by the summarizer, using your existing Codex
authentication and usage allowance. Local storage does not mean offline processing.
The instruction to avoid secrets in notes is not automatic secret redaction.
Only enable this for conversations you are comfortable sending through Codex.

The isolated worker ignores user configuration, so custom providers, including
local model endpoints, are not inherited. The secure storage boundary supports
macOS and Linux. This change was tested on macOS; a Linux runtime run is not
implied by that check. Windows is unsupported.

## Budgets and limits

- Defaults mirror the upstream 100,000/50,000 upper/lower thresholds. Estimates
  use UTF-8 bytes/4, not a tokenizer. The rendered prompt/schema and a fixed
  project-note allowance count toward the upper budget. Leave model headroom.
- More than 1,000 messages triggers compaction even below the token budget.
  Retention keeps at most 500 messages, including the initial request, with the
  remaining tail starting at an assistant boundary. Unchanged history does not
  trigger another model call; `PreCompact` still summarizes new pending history.
- `--timeout-seconds` defaults to 120 for the entire invocation's model work.
  Summarization consumes normal Codex usage and can delay compaction or Stop.
- Summary <=12,000 UTF-8 bytes. Restore output is bounded to roughly 16 KiB.
- Notes <=128 entries /32,000 serialized bytes. Capacity-rejected note changes
  produce a warning, but the valid task summary still saves. When configured
  with small input budgets, only fitting existing notes reach the summarizer;
  all notes remain persisted. Note curation/deletion has no dedicated CLI yet.
- Rollouts are streamed; the historical prefix is hashed to detect rewrites.
  One batch <=64 MiB or 4,096 parsed messages; individual JSONL record <=8 MiB.
  Partial trailing records wait for a subsequent hook. Backlog warns explicitly.
- A single message/initial request that cannot fit the configured model budget
  fails explicitly, without silently truncating source evidence. Raise the budget
  within the model's limits, or inspect the source before retrying.
- Project state is bounded to 32 MiB. Very old sessions may require archival;
  do not edit active documents, runtime files or journals while hooks run.

Codex's rollout format is not a stable API. The adapter supports text messages,
function/custom tool calls and results, skips duplicate event records and hidden
reasoning, and labels non-text message content. Format changes, model omissions,
timeouts, backlog or lock contention can leave summaries incomplete or stale.
Use the original rollout/code as the source of truth; this is lossy memory.

Tests exercise two compaction cycles, user state restoration, idempotency,
scope isolation, model failures, recovery after partial compaction, message-count
limits, note saturation, budget growth across sessions,
UTF-8-safe JSON processing, partial records, rewritten history, lifetime rollouts
above 64 MiB, and real subprocess timeouts. Two additional live model smoke cycles
were checked manually on synthetic data, including a user correction and note
update without duplication. This is not a long-horizon benchmark.

## Compatibility with Confucius

This ports selected memory behavior rather than the whole agent runtime. The
note-taker keeps its `id`, `title`, optional `description`/`keywords`, focused
Markdown body and shared/projects paths. Hierarchy `tags` are separate; keywords
are not silently converted into tags. Write replaces a document, while edit
requires one exact match after whitespace-only-line normalization. Fuzzy error
suggestions are not reproduced and fuzzy changes are never applied.

The Rust YAML parser differs from PyYAML for unquoted `yes`/`on` and date-looking
values, and for duplicate mapping keys. Use quoted strings and unique keys for
portable tags. Generated tags are always quoted so PyYAML also reads them as
strings. Unsupported Python regex constructs and Unicode case matching can differ;
this is not a general Python regex emulator. Directory reads expose stored Markdown
documents, not arbitrary files in the directory. Delete/import tools, reminder
scheduling and the original artifact UI are outside this adaptation.
