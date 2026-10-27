# Confucius provenance

Rust adaptation based on facebookresearch/cca-swebench commit
`bdeb74b3d828bfbb2c61030c6201fa9da44e69d0` (MIT; see LICENSE-META).

- `confucius/orchestrator/extensions/plan/llm.py`: LLMPlannerExtension trigger, first-message retention, assistant-boundary tail selection, summary replacement, first next step.
- `confucius/orchestrator/extensions/plan/prompts.py`: six-part coding context summary and next-step guidance, adapted in prompts/compress.txt.
- `confucius/analects/note_taker/tasks.py`: selective expensive-to-rediscover insights, existing-note update/deduplication, hierarchical topics, observer boundary.

Differences: Codex rollout adapter and native hooks; UTF-8 bytes/4 estimates rather than upstream token estimator; typed JSON instead of HTML/XML parsing; one structured model call produces summary and selective note updates rather than two independently tooled agents; bounded errors instead of unlimited repair retries; private atomic storage and cursor replay protection. Notes use hierarchical identifiers in a project-scoped JSON map rather than the full Confucius filesystem memory-tool API. No native Codex compaction replacement or hidden reasoning access.
