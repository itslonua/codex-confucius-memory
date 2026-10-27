# Confucius Memory effect benchmark

This directory contains the synthetic coding/investigation benchmark and its raw results. It is separate from the older recall-only benchmark under `tools/codex-memory/bench/`.

## Reproduce the automatic-restore run

The 18-trial continuation protocol reuses the frozen corpus produced by the first run. It requires these artifacts:

- `results/full-run.json`
- `results/artifacts/memory-store/`
- `results/artifacts/memory-search-preflight.json`

With an authenticated Codex CLI and the built `tools/codex-memory/target/release/codex-memory` binary, run from the repository root:

```sh
python3 tools/codex-memory/bench/effect/continuation_bench.py \
  --output tools/codex-memory/bench/effect/results/reproduction/full-run.json
```

Use an unused output path so historical results are not overwritten. This makes 18 fresh `gpt-6-luna` sessions and does not call `compress` again. The tasks, fixtures, graders, prompts, outputs, restored context, source hashes, and per-trial checkpoints are retained under `results/`.

The runner creates a new temporary project cwd. Since Confucius Memory keys project data by canonical cwd, it copies the frozen project directory and lock file under the new cwd-derived key. It verifies that copied store content matches the source-run hash and checks production restore, search, and read before any model call. It does not modify frozen store contents or remove the historical source cwd.

The earlier optional-retrieval 18-trial output is saved at `results/full-run.json`. It used production `codex-memory compress` and agent-directed search/read, but the exact runner source matching its recorded runner hash was not retained; see `REPORT.md`. `effect_bench.py` redirects to the supported continuation runner and cannot regenerate the first protocol.

To run the local deterministic harness checks without model calls:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s tools/codex-memory/bench/effect -p 'test_effect_bench.py' -v
```
