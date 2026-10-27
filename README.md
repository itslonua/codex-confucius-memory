# Confucius Memory for Codex

**Keep the thread of your work.**

[English](README.md) · [Deutsch](README.de.md) · [Français](README.fr.md)

Long coding sessions accumulate more than code: decisions, dead ends, subtle
constraints, and the reason a particular fix finally worked. Confucius Memory
helps carry that context into the next stage of the conversation.

A small Rust companion for Codex, it automatically creates task summaries and
selective project notes. The main agent does not need to write checkpoints.
The memory workflow is adapted from Meta's Confucius planner and note-taker.

## What it remembers

- **The task:** requirements, user corrections, decisions, and current progress.
- **The difficult parts:** failed approaches and discoveries worth keeping for
  future work, with references to their evidence.
- **What comes next:** outstanding work and the first concrete next action.

Task summaries belong to an individual session. Project notes can be reused by
other sessions in the same working directory. Separate worktrees and
subdirectories keep separate memory.

## How it fits into Codex

Native hooks do the work: `Stop` processes new history and summarizes when a
budget is reached; `PreCompact` summarizes pending history before compaction;
`SessionStart` restores the task summary and an index of project notes.

This supplements Codex's built-in compaction. It does not replace that algorithm
or rewrite the original conversation. Each successful summary is saved with the
exact history position it covers, so a later timeout can resume from saved
progress. Unchanged history does not trigger another model call.

## Get started

You need **Rust 1.88+**, `make`, and an authenticated Codex CLI with compatible
native hooks and worker flags. Tested with **Codex CLI 0.161.0 on macOS**.

```sh
git clone https://github.com/itslonua/codex-confucius-memory.git
cd codex-confucius-memory
make check
make build
./target/release/codex-memory --help
```

Generate hook entries using the release binary:

```sh
./target/release/codex-memory --codex /absolute/path/to/codex hooks > /tmp/confucius-hooks.json
```

Review the generated file and merge its entries into your project's
`.codex/hooks.json`, preserving existing hooks. In Codex, use `/hooks` to review
and trust the entries; the project configuration must also be trusted.
Keep the built binary at the path used when generating the configuration.

See the [usage guide](docs/usage.md) for configuration, budgets, manual commands,
troubleshooting, and removal.

## Your data and model usage

Memory is stored locally in `$CODEX_HOME/confucius-memory`, or
`$HOME/.codex/confucius-memory` when `CODEX_HOME` is unset. New directories are
private, snapshots are replaced atomically, and writers use a project lock.

**Summarization sends selected conversation history and notes to the model
service used by the isolated Codex CLI worker.** It uses your existing Codex
authentication and normal usage allowance. The worker does not inherit custom
user configuration, including custom provider settings. Model calls can add
latency; this is not an offline summarizer.

## Status and limits

**Experimental v0.1.** A practical companion for long sessions, with tests for
incremental processing, recovery, isolation, and subprocess failures. Summaries
are lossy and may be incomplete or stale; verify important facts against the
original conversation and code. The Codex rollout format is not a stable API.

There is no demonstrated token saving or long-horizon benchmark improvement over
Codex's native compaction. The aim is to preserve useful context; the actual
benefit depends on the task and the quality of the summaries.

## Credits

Adapted from [Meta's Confucius](https://github.com/facebookresearch/cca-swebench/tree/main/confucius).
See [provenance and changes](THIRD_PARTY_NOTICES.md) and the
[upstream MIT license](LICENSE-META). This is a community project, unaffiliated
with Meta or OpenAI.
