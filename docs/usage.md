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
request, producing both summary and notes. Splitting the files does not add a
second model call. Both outputs are parsed before memory is changed, so a failed
request leaves that batch available for retry. Python/LangChain orchestration is
replaced by Rust. See
`THIRD_PARTY_NOTICES.md` for the pinned source revision and `LICENSE-META` for
upstream attribution. This is an adaptation of the memory workflow, not the full SDK.

## Lifecycle

1. `Stop` reads new completed rollout records and persists incremental task state.
   It invokes the model only when the input budget/message threshold is reached.
2. `PreCompact` also flushes pending history into a summary and selective notes.
3. `SessionStart(startup|resume|compact)` injects the task summary and a bounded
   project-note index. Full notes are available at the injected memory-file path.
4. The summarizer runs in an isolated temporary directory using `codex exec
   --ignore-user-config --ephemeral`, a read-only sandbox, and disabled hooks,
   plugins, shell tools, multi-agent tools and skill search.

Hooks supplement native Codex compaction. They **cannot replace its algorithm or
edit its active message array**. First-message/tail retention applies to this
adaptation's own rolling summarizer input; the hook injects the compact summary, not
another full copy of retained tool output. No original rollout is rewritten.

Task state is isolated by canonical working directory and session ID. Project
notes are shared only within that directory, including `shared/` topic IDs.
Different worktrees/subdirectories intentionally have separate memory scopes.
Memory is historical evidence, not instructions or authorization.

## Build and checks

Requires Rust >=1.88 and a Codex CLI supporting the flags in `src/worker.rs`.
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
A project hash names a JSON file containing session histories and project notes.
New directories are private; data files are atomically replaced with mode 0600.
A per-project OS lock prevents concurrent writers. A busy hook warns and retries
on a later lifecycle event; it does not block the user's task.

## Manual smoke test

All runtime commands accept a single JSON object on stdin:

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
local model endpoints, are not inherited. This version has been tested on macOS;
other platforms have not been verified.

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
- Project snapshots are bounded to 32 MiB. Very old sessions need manual archival
  before that limit; do not edit the file while a hook is running.

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
