# Confucius Memory effect benchmark: frozen protocol

## Question
Does agent-directed retrieval from a real Confucius Memory store improve coding/investigation outcomes on later tasks when both conditions receive the same source tree, logs, model, tools, and deterministic acceptance checks?

## Scope and limitations
This is a controlled synthetic coding benchmark, not a benchmark of users' real repositories or native Codex SessionStart injection. Each trial is an isolated `gpt-6-luna` CLI session with shell access. The treatment can restore the session summary and search/read any project note produced by `codex-memory compress` from a separate learning transcript. The control receives an empty store at the same path and identical CLI instructions. The frozen store is never written during trials. All source, tests, logs, and current requirements are available in each condition. Three task classes: repeat investigation, related transfer, and stale/no-relevance control. Three paired repetitions per class (18 trials).

## Frozen task/check design
1. Repeat: fix the observed importer/lookup key mismatch found in a multi-module learning investigation; acceptance checks both catalog/archive normalization and end-to-end record lookup; both formatters are allowed changes.
2. Transfer: apply the related punctuation and separator behavior to the archive key formatter.
3. Stale control: follow the updated policy source after the learning snapshot recorded a previous separator decision.

Every deterministic grader and required changed file is fixed in `tasks.json` before any trial. A trial succeeds only when the grader exits 0 and every required implementation/test artifact differs from the frozen starting state. No answer-text rubric or post-hoc scoring.

## Isolation and run procedure
- Use `codex exec --ignore-user-config --ephemeral --skip-git-repo-check --sandbox workspace-write --json -C <fixture>` with `gpt-6-luna` for every learning/trial session.
- Disable hooks, plugins, native memories, multi-agent, and skill search; enable shell/unified execution and skip host skill discovery. Do not load repository or user configuration. Keep the current authentication context only.
- Learning session is separate from held-out sessions. It traces a reproducible importer/lookup integration failure without editing or applying a fix. Convert the actual CLI message and shell call/output events into the production rollout transcript format, then generate memory with production `codex-memory compress`. Preserve the adapted transcript, original raw JSONL, usage JSONL (via transparent wrapper), and resulting store. If compression creates no notes, retain that result and still run the paired trials using restore and the empty search index.
- Freeze the generated store before trial execution. Baseline points the same production CLI at an empty store. Arm order is seeded and shuffled within each pair. Both arms get identical per-task copies and prompts.
- No real user store is read or written. No secrets or environment files are read.

## Outcomes and reporting
Primary outcome: deterministic pass rate by arm and task class. Secondary outcomes: paired wins/losses/ties, elapsed time including the memory retrieval tool call, tool calls, model usage when present, and memory creation overhead. Preserve exact prompts, answer files, JSONL event streams, acceptance stdout/stderr/exit codes, and execution metadata. Missing CLI usage fields remain null. Translate observed token totals using the published gpt-6-luna prices in `REPORT.md`, but label API-equivalent costs and note Codex billing tier is not observable.

## Analysis lock
No task, prompt, acceptance command, parser/scorer, or rubric changes after first trial. Any harness correction requires discarding all trials and rerunning the full frozen protocol with a new protocol hash. Report all failures and null metrics. This small synthetic run is descriptive; do not claim statistical significance or generalize beyond these fixtures.

## Stale-state control detail
The learning-only snapshot records the earlier archive separator policy (`-`) while the source helpers disagree about keys. Before held-out trials, the same stable project path is reset to the current snapshot where `policy.py` specifies `_`. The stale-control task asks the agent to follow current source without spelling out the output; the hidden grader checks current behavior. Both trial arms receive this identical current snapshot. The learning snapshot, current trial snapshot, grader scripts, and runner are hashed in the result.
