# Confucius Memory for Codex

**Keep the thread of your work.**

[English](README.md) · [Deutsch](README.de.md) · [Français](README.fr.md)

Long coding sessions accumulate more than code: decisions, dead ends, subtle
constraints, and the reason a particular fix finally worked. Confucius Memory
helps carry that context into the next stage of the conversation.

A small Rust companion for Codex, it automatically creates task summaries and
selective project notes. The main agent does not need to write checkpoints.
The memory workflow is adapted from Meta's Confucius planner, note-taker and
hierarchical memory. Notes live in Markdown files organized by topic.

## What it remembers

- **The task:** requirements, user corrections, decisions, and current progress.
- **The difficult parts:** failed approaches and discoveries worth keeping for
  future work, with references to their evidence.
- **What comes next:** outstanding work and the first concrete next action.

Task summaries belong to an individual session. Project notes can be reused by
other sessions in the same working directory. Separate worktrees and
subdirectories keep separate memory.

## How it fits into Codex

Native background hooks do the work: `Stop` processes new history and summarizes
when a budget is reached; `PreCompact` schedules pending history for summarization
without delaying compaction. `SessionStart` reads saved context synchronously with
a three-second timeout; it never calls the model.

This supplements Codex's built-in compaction. It does not replace that algorithm
or rewrite the original conversation. Each successful summary is saved with the
exact history position it covers, so a later timeout can resume from saved
progress. Unchanged history does not trigger another model call.

## Get started

You need **macOS or Linux, Rust 1.88+**, `make`, and an authenticated Codex CLI with compatible
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

The bundled [memory skill](skills/confucius-memory/SKILL.md) helps Codex find
saved discoveries before repeating an investigation. `search` finds matching
notes by words or path/content/tag filters; `read` opens a note, a line range or
a directory, without a model call. The usage guide includes
skill installation instructions.

See the [usage guide](docs/usage.md) for configuration, budgets, manual commands,
troubleshooting, and removal.

## Your data and model usage

Memory is stored locally in `$CODEX_HOME/confucius-memory`, or
`$HOME/.codex/confucius-memory` when `CODEX_HOME` is unset. New directories are
private. Summaries and notes are Markdown; continuation data stays separate.
A project lock and recovery journal keep updates consistent, and existing JSON
stores migrate on their first write while retaining the original file.

**Summarization sends selected conversation history and notes to the model
service used by the isolated Codex CLI worker.** It uses your existing Codex
authentication and normal usage allowance. The worker does not inherit custom
user configuration, including custom provider settings. Model calls can add
background work; this is not an offline summarizer. A newly generated summary may
not be ready at the next startup. Codex cancels unfinished background hooks when
the session ends; retained transcripts allow a later hook to retry. Review changed
hook definitions with `/hooks` before relying on the new configuration.

## Benchmarks

An exploratory run on 2026-10-28 compared isolated `gpt-6-luna` coding sessions
with and without Confucius memory. **It did not demonstrate a quality improvement.**
Three small synthetic tasks covered a repeated importer/lookup investigation,
a related formatter change, and a changed separator policy. Each task was run
three times per condition, with fresh source copies and seeded paired ordering.
Both conditions could solve every task from the same source and logs.

| Automatic restoration: 9 trials per condition | Without saved memory | With saved memory |
| --- | ---: | ---: |
| Passed independent checks | 3/9 | 3/9 |
| Input tokens, including cached tokens | 1,286,100 | 1,027,475 |
| Cached input tokens (subset of input) | 1,208,704 | 966,272 |
| Output tokens | 12,399 | 11,048 |
| Shell command events | 44 | 34 |
| Mean time, including restoration | 20.93 s | 18.49 s |
| Standard API-equivalent trial cost | $0.02603 | $0.02131 |

### Method

1. A separate Luna investigation examined an earlier source snapshot. The actual
   `codex-memory compress` command created one project note and a session summary.
   Held-out task solutions and hidden checks were not supplied to that session.
2. The memory corpus was frozen. Every trial started a fresh CLI process with the
   same model, tools, task source and current requirements. User configuration,
   native memory, plugins and automatic hooks were disabled for isolation.
3. In the automatic-restoration experiment, the runner called the real `restore`
   command and supplied its returned context before the task. The control used an
   empty store; the treatment included the prior summary and note index. This
   emulates continuation context, not the desktop hook scheduler itself.
4. Immutable external Python checks and required file changes determined success.
   Failed solutions stayed in the denominator. Prompts, tool events, usage, code
   snapshots, hashes and check outputs were retained. All 18 saved solutions were
   independently checked again with matching outcomes.
5. A separate, earlier optional-retrieval experiment also ran 18 sessions: the
   control passed 4/9 and treatment 3/9. Agents rarely read the available note.
   Its results are kept separate from automatic restoration.

The automatic-restoration run observed 20% fewer input tokens and 18% lower trial
cost, with equal success and paired outcomes of one memory win, one loss and
seven ties. This is a small descriptive sample, not a statistically established
saving. The stale-policy check also under-specified underscore removal, so its
failures do not establish harm from stale memory. These fixtures are not a
long-horizon benchmark or a comparison against native Codex compaction.

Costs use the published [Luna standard API rates](https://developers.openai.com/api/docs/models/gpt-6-luna):
$0.10 per million uncached input tokens, $0.01 per million cached input tokens and
$0.50 per million output tokens. They are API equivalents, not confirmed Codex
account charges. The measured learning investigation cost $0.00426 and memory
compression $0.00252. Including compression alone reduces the observed nine-trial
saving to 8.4%; charging both learning and compression makes that group 7.9% more
expensive than control. Repeated trials do not establish real-world amortization.
Preliminary setup calls and benchmark-development costs are excluded.

See the [full report](bench/effect/REPORT.md),
[reproduction instructions](bench/effect/README.md), and
[raw automatic-restoration results](bench/effect/results/continuation/full-run.json).
The original [recall-only benchmark](bench/README.md) is retained separately; it
measures access to remembered facts, not coding productivity.

## Status and limits

**Experimental v0.1.** A practical companion for long sessions, with tests for
incremental processing, recovery, isolation, and subprocess failures. Summaries
are lossy and may be incomplete or stale; verify important facts against the
original conversation and code. The Codex rollout format is not a stable API.

There is no established general token saving or long-horizon benchmark improvement
over Codex's native compaction. The aim is to preserve useful context; the actual
benefit depends on the task and the quality of the summaries.

## Credits

Adapted from [Meta's Confucius](https://github.com/facebookresearch/cca-swebench/tree/main/confucius).
See [provenance and changes](THIRD_PARTY_NOTICES.md) and the
[upstream MIT license](LICENSE-META). This is a community project, unaffiliated
with Meta or OpenAI.
