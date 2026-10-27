# Confucius Memory effect benchmark results

## Conclusion

This small synthetic benchmark did not demonstrate an overall coding-success improvement from memory. In the automatic-restore protocol, both arms passed 3 of 9 trials, with 1 memory win, 1 memory loss, and 7 ties. The repeat task favored memory 2/3 to 1/3, the transfer task tied at 1/3 each, and the stale-control task favored baseline 1/3 to 0/3. With only three task types and three repetitions, these are descriptive results, not evidence of a general effect.

The result depends on memory exposure. The first protocol only made retrieval optional; the memory agent searched once in nine trials and read the note twice. That setup scored 3/9 for memory versus 4/9 baseline. A second protocol automatically restored session context before every trial; it scored 3/9 in both arms. Memory reduced observed mean time and token usage in that second run, but success was unchanged and one task class regressed. The synthetic sample is too small to conclude that the lower measured usage generalizes.

## Protocols

Both protocols used 18 fresh, isolated `gpt-6-luna` CLI sessions per protocol: three synthetic tasks (repeat investigation, related transfer, stale-policy control), three paired repetitions each, seeded arm ordering, identical task source snapshots and graders, and shell access. Hooks, plugins, native memories, multi-agent, and skill search were disabled. Deterministic Python oracles and required changed-file checks defined success; answer-text similarity was not scored. Raw prompts, JSONL events, answers, tool outputs, code snapshots, and grader results are retained under `results/`.

The learning session investigated the earlier synthetic importer/lookup mismatch without editing source. Production `codex-memory compress` created one searchable project note and a session summary. The saved note records the old separator investigation. The held-out source changes `policy.py` to `_`; the old memory is therefore historical and must be checked against current source. The same frozen memory-store SHA-256, `24a9390233e6a0369776e23eeb05a0fc7945b5aa3912b475f74565a6ecd16fb0`, was reused in both protocols.

### Optional retrieval

The first run let each agent decide whether to restore/search/read memory. It found 4/9 baseline successes and 3/9 memory successes. Paired outcomes were 2 memory wins, 3 losses, and 4 ties. Mean elapsed time was 22.40 seconds for baseline and 23.38 seconds for memory. The memory arm issued one search yielding one note and two successful reads; baseline issued three searches yielding no notes. Neither arm restored session context. Thus this protocol had limited treatment exposure.

### Automatic restore

Before each session, the runner called production `codex-memory restore` and injected its exact `additionalContext` into the prompt. Baseline used an empty store; memory used the frozen store. The runner recorded the exact context and hashes of both base and injected prompts, included restore latency in elapsed time, and allowed subsequent agent-directed search/read. Preflight confirmed that baseline restore had no summary or note index; treatment restore included both; search found exactly one note; and reading it matched the frozen source content.

Both arms passed 3/9. Per-task successes were:

| Task | Baseline | Memory |
| --- | ---: | ---: |
| Repeat catalog investigation | 1/3 | 2/3 |
| Transfer to archive formatter | 1/3 | 1/3 |
| Stale-policy control | 1/3 | 0/3 |

The paired result was 1 win, 1 loss, and 7 ties. Mean elapsed time, including restore, was 20.93 seconds for baseline and 18.49 seconds for memory. Total observed usage was 1.286M input / 12,399 output tokens for baseline and 1.027M input / 11,048 output tokens for memory. No trial agent issued a subsequent memory search/read call: this protocol tested automatically restored continuation context, not agent-selected note reading. Treatment prompt context was 4,055 characters; baseline's generic restore context was 1,429 characters.

All 18 trial CLI processes exited 0 without timeouts. The baseline executed 44 shell command events and memory executed 34. An independent post-run audit re-ran all 18 frozen graders against the saved trial projects; every grader exit matched the stored outcome.

The stale-control grader required `archive_key("A_B") == "ab"`. All five failures in this task failed that assertion, including baseline failures; the prompt only referred generally to punctuation and the current separator policy. This task therefore did not isolate whether old separator knowledge was stale, and its failures should not be attributed to the memory note.

## API-equivalent cost and overhead

The CLI's actual billing tier is not observable here, so these are API-equivalent calculations from captured token usage, not measured account charges. The published GPT-6 Luna standard rates used are $0.10 per million uncached input tokens, $0.01 per million cached input tokens, and $0.50 per million output tokens ([official model pricing](https://developers.openai.com/api/docs/models/gpt-6-luna)). No cache-write tokens were reported.

| Run/group | Baseline | Memory | Notes |
| --- | ---: | ---: | --- |
| Optional-retrieval trials, 9 each | $0.02985 | $0.03465 | Memory had sparse retrieval exposure |
| Automatic-restore trials, 9 each | $0.02603 | $0.02131 | Observed API-equivalent replay usage |
| Learning investigation | — | $0.00426 | One-time prior investigation |
| Production compression | — | $0.00252 | One-time memory creation |

In the automatic-restore run, memory replay was $0.00472 cheaper across nine calls, or about $0.000524 per call, while passing the same number of trials. If the prior investigation would have happened anyway, charging only compression makes that group $0.02383, about 8.4% below baseline. If both learning and compression are incremental memory overhead, the treatment is $0.02809, about 7.9% above baseline for that nine-call group. At the measured per-call difference, three distinct tasks would save about $0.00157—less than the $0.00252 compression call; a roughly five-call break-even is an arithmetic extrapolation, not a measured threshold. Charging both learning and compression requires roughly thirteen calls at that same measured difference. These extrapolations assume the observed token difference persists and should not be treated as a forecast.

Dividing by observed successful trials gives $0.00868 per baseline success and $0.00710 per memory success for replay alone. Adding compression yields $0.00794 per memory success; charging both learning and compression yields $0.00936. These ratios are especially unstable here because each arm had only three successes and identical overall pass counts. In the optional-retrieval run, the corresponding replay-only ratios were $0.00746 baseline and $0.01155 memory.

The optional-retrieval protocol's nine memory calls cost more than its nine baseline calls, and its memory arm passed one fewer trial. Taken together, the benchmark does not establish that memory improves quality or lowers real Codex usage charges.

## Limitations and provenance

Tasks are small synthetic normalization fixes, not real repository investigations or user workflows. The automatic-restore arm includes a prior session summary and project note index, so any effect is continuation-context exposure and cannot be attributed to the note alone. The stale-control rubric was under-specified for underscore removal. Only three task types and three repeats were run; no statistical claim is warranted.

The first run's JSON records runner SHA-256 `7fcc4e8628c5685fd83cc7fb8165f2a372721963503a5a5f7ccc76b000bc0441`, but the matching source snapshot is unavailable. A later, incompatible runner file was found and preserved as `results/diagnostics/unexpected-runner.py` with SHA-256 `6d580adecd4da7e11df0ebe2d5cc69f04b7efdcae7ca34fc9de425e94368f966`. The first run's raw prompts, JSONL, outputs, grader logs, hashes, and result rows remain available, but exact runner-source reproduction is limited. The measured continuation runner snapshot and manifest are present in `results/continuation/protocol-snapshot/`, and the recorded runner hash matches that snapshot. After the measured run, the current runner changed only its portability setup: it now uses a fresh temporary cwd and rekeys a copied frozen store directory/lock for that cwd. The frozen note/session content is unchanged; a no-model `--preflight-only` run verified restore, search, and read. Historical trial JSON and snapshots were left untouched.

Preliminary setup attempts are excluded from the 36 completed held-out trials. The tiny pilot performed learning/compression but produced no searchable project note; another attempt was interrupted during learning/compression after a task-contract flaw was found. Those attempts incurred or may have incurred model usage, some of which was not retained, and are not included in the cost totals above. Continuation preflight failures occurred before held-out model calls, including an artifact-copy collision. Available diagnostics are preserved separately. No real memory store was modified.
