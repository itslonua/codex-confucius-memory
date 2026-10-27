# Confucius Memory recall benchmark

This is a small A/B benchmark of memory-assisted recall, not a coding-productivity or long-horizon benchmark. It uses five fixed questions (familiar decisions, a new decision, a stale-memory conflict, and an unknown-answer control), two arms, and three repeats by default. The memory arm gets results from the actual `codex-memory search` and `read` commands over a frozen synthetic v2 Markdown store. The baseline gets the same task and current context with no historical memory. No production memory store is read or changed.

The runner starts independent `codex exec` processes with `gpt-6-luna`, `--ignore-user-config`, `--ephemeral`, a fresh empty temporary cwd, and a read-only sandbox. It disables hooks, plugins, shell and unified execution, multi-agent, and skill search. User authentication is inherited from the current Codex home; no auth data is copied or printed. Historical notes and task inputs are held constant. Arm order is randomized per task/repeat with a fixed seed. The output retains every raw final answer, event stream, stderr, exit code, and elapsed time. Token counts are extracted only when present in Codex JSON events; cost is never inferred.

Run from this directory:

```sh
python3 -m unittest -v test_bench.py
python3 bench.py --repeats 3
```

Scoring checks for every task's predefined required terms in the JSON `answer` field, case-insensitively. This simple rubric does not judge explanation quality. For the stale control, the current task explicitly supplies the updated value to both arms; the memory arm receives the older conflicting note. For the unknown control, both arms must abstain because the note records no palette decision. Baseline results are expected to be low on historical facts because those facts are deliberately absent; this measures availability of recalled information, not general reasoning capability.

The benchmark is exploratory: only five synthetic tasks and three repeated runs, with no statistical significance claim. Results are saved to `results/latest.json`; inspect per-task trials before drawing conclusions.

Retrieval queries and target note paths are supplied by the fixture author; Luna receives the retrieved text and does not choose searches itself. This does not test automatic note creation, SessionStart integration, discovery of useful notes, or native compaction. Reported latency measures the model subprocess and excludes the preceding memory retrieval. A preliminary nine-trial run was interrupted during harness review; smoke and preliminary runs are excluded from the final 30 trials but also consumed model usage.

## Recorded result (2026-10-27)

The completed 30-run result is in `results/latest.json`. Under the frozen literal-term rubric, memory scored 12/15 (80%) and no-memory scored 6/15 (40%). The memory arm took a mean 3.73 seconds per task versus 3.85 seconds for baseline. All trials returned valid structured answers; none failed or timed out. The stale override and unknown-answer controls scored 3/3 in both arms. Memory scored 3/3 on familiar preferences and the fresh decision. Baseline scored 0/3 on those historical facts.

The CLI semantics item is a known rubric false negative: all three treatment responses said “all-word search,” while the frozen expected terms require “all words.” A post-hoc semantic audit therefore rates this item 3/3 correct while leaving the original scores untouched; the audited memory result would be 15/15. Input and output token totals are in the result artifact, taken from Codex JSONL usage events. The benchmark does not convert tokens to cost or claim a statistical effect.
