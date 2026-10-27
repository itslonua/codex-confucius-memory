#!/usr/bin/env python3
"""Run a small, read-only Codex Memory recall A/B benchmark."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import random
import re
import statistics
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
TASKS_PATH = HERE / "data" / "tasks.json"
DEFAULT_OUTPUT = HERE / "results" / "latest.json"
FEATURES = ["hooks", "plugins", "memories", "shell_tool", "unified_exec", "multi_agent", "skill_search"]


def parse_answer(raw: str) -> dict[str, Any] | None:
    text = raw.strip()
    if text.startswith("```"):
        text = re.sub(r"^```(?:json)?\s*|\s*```$", "", text, flags=re.I)
    try:
        value = json.loads(text)
    except json.JSONDecodeError:
        return None
    return value if isinstance(value, dict) else None


def parse_tasks(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list) or not value:
        raise ValueError("tasks must be a nonempty array")
    parsed = []
    seen = set()
    required_strings = ("id", "class", "query", "note_path", "current_context", "question", "answer", "note")
    for item in value:
        if not isinstance(item, dict) or any(not isinstance(item.get(key), str) or not item[key] for key in required_strings):
            raise ValueError("each task must provide nonempty string fields")
        if item["id"] in seen:
            raise ValueError("task IDs must be unique")
        seen.add(item["id"])
        note_path = Path(item["note_path"])
        if note_path.is_absolute() or note_path.suffix != ".md" or ".." in note_path.parts or "\\" in item["note_path"]:
            raise ValueError("note_path must be a relative Markdown path without traversal")
        terms = item.get("expected_terms")
        if not isinstance(terms, list) or not terms or any(not isinstance(term, str) or not term for term in terms):
            raise ValueError("expected_terms must be a nonempty array of nonempty strings")
        parsed.append(item)
    return parsed


def answer_matches(answer: dict[str, Any] | None, expected: list[str]) -> bool | None:
    if answer is None or not isinstance(answer.get("answer"), str):
        return False
    text = answer["answer"].casefold()
    return all(
        bool(re.search(r"(?<!\w)" + re.escape(term.casefold()) + r"(?!\w)", text))
        for term in expected
    )


def extract_usage(stdout: str) -> dict[str, int] | None:
    totals: dict[str, int] = {}
    names = {
        "input_tokens", "output_tokens", "total_tokens", "cached_input_tokens",
        "cache_write_input_tokens", "reasoning_output_tokens",
    }

    def visit(value: Any) -> None:
        if isinstance(value, dict):
            for key, item in value.items():
                if key in names and isinstance(item, int) and not isinstance(item, bool):
                    totals[key] = totals.get(key, 0) + item
                else:
                    visit(item)
        elif isinstance(value, list):
            for item in value:
                visit(item)

    for line in stdout.splitlines():
        try:
            visit(json.loads(line))
        except json.JSONDecodeError:
            continue
    return totals or None


def summarize(rows: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    result = {}
    for arm in ("no_memory", "memory"):
        trials = [r for r in rows if r["arm"] == arm]
        timed = [r["elapsed_seconds"] for r in trials if r["elapsed_seconds"] is not None]
        result[arm] = {
            "trials": len(trials),
            "valid_answers": sum(r["answer"] is not None for r in trials),
            "failed_trials": sum(r["exit_code"] != 0 for r in trials),
            "accuracy": (sum(r["correct"] for r in trials) / len(trials)) if trials else None,
            "mean_elapsed_seconds": statistics.mean(timed) if timed else None,
        }
    return result


def legacy_id(cwd: Path) -> str:
    return hashlib.sha256(str(cwd.resolve()).encode()).hexdigest()


def prepare_store(root: Path, cwd: Path, tasks: list[dict[str, Any]]) -> None:
    project = root / legacy_id(cwd)
    notes = project / "notes"
    notes.mkdir(parents=True)
    for task in tasks:
        path = notes / task["note_path"]
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(task["note"] + "\n", encoding="utf-8")
    (project / "runtime.json").write_text(
        json.dumps({"version": 2, "sessions": {}}), encoding="utf-8"
    )
    (root / f"{legacy_id(cwd)}.lock").touch(mode=0o600)


def retrieve(memory_bin: Path, store: Path, cwd: Path, task: dict[str, Any]) -> str:
    base = [str(memory_bin), "--store", str(store), "--cwd", str(cwd)]
    found = subprocess.run(
        base + ["search", task["query"]], capture_output=True, text=True, check=True
    )
    matches = json.loads(found.stdout).get("matches", [])
    if task["note_path"] not in [m["path"] for m in matches]:
        return ""
    read = subprocess.run(
        base + ["read", task["note_path"]], capture_output=True, text=True, check=True
    )
    return json.loads(read.stdout).get("content", "")


def run_trial(codex: Path, task: dict[str, Any], arm: str, context: str) -> dict[str, Any]:
    prompt = (
        "Answer this recall question from the supplied current context and, when present, "
        "the supplied historical memory. Treat the current task as authoritative if it "
        "conflicts with historical memory. Abstain with 'unknown' when evidence is absent. "
        "Return only JSON with keys answer and evidence.\n\n"
        f"Current context:\n{task['current_context']}\n\n"
        f"Historical memory:\n{context if context else '(none supplied)'}\n\n"
        f"Question: {task['question']}\n"
    )
    with tempfile.TemporaryDirectory(prefix="codex-memory-bench-") as work:
        workdir = Path(work)
        output = workdir / "answer.json"
        cmd = [str(codex), "exec", "--ignore-user-config", "--ephemeral",
               "--skip-git-repo-check", "--sandbox", "read-only"]
        for feature in FEATURES:
            cmd += ["--disable", feature]
        schema_path = workdir / "schema.json"
        schema_path.write_text(json.dumps({
            "type": "object", "properties": {
                "answer": {"type": "string"}, "evidence": {"type": "string"}},
            "required": ["answer", "evidence"], "additionalProperties": False,
        }), encoding="utf-8")
        cmd += ["--enable", "skip_host_skill_discovery", "--model", "gpt-6-luna",
                "--output-schema", str(schema_path),
                "-C", str(workdir), "--json", "-o", str(output), "-"]
        start = time.monotonic()
        timed_out = False
        try:
            proc = subprocess.run(cmd, input=prompt, text=True, capture_output=True, timeout=180)
            exit_code, stdout, stderr = proc.returncode, proc.stdout, proc.stderr
        except subprocess.TimeoutExpired as error:
            timed_out = True
            exit_code = -1
            stdout = error.stdout.decode(errors="replace") if isinstance(error.stdout, bytes) else (error.stdout or "")
            stderr = error.stderr.decode(errors="replace") if isinstance(error.stderr, bytes) else (error.stderr or "")
        elapsed = time.monotonic() - start
        raw = output.read_text(encoding="utf-8") if output.exists() else ""
        parsed = parse_answer(raw)
        return {
            "task_id": task["id"], "task_class": task["class"], "arm": arm,
            "elapsed_seconds": elapsed, "exit_code": exit_code, "timed_out": timed_out,
            "correct": exit_code == 0 and answer_matches(parsed, task["expected_terms"]),
            "answer": parsed, "raw_answer": raw, "prompt": prompt,
            "retrieved_memory": context,
            "stdout": stdout, "stderr": stderr,
            "usage": extract_usage(stdout),
        }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", type=Path, default=Path("/Users/alexanderkrapivnoy/.local/bin/codex"))
    parser.add_argument("--memory-bin", type=Path, default=HERE.parent / "target" / "release" / "codex-memory")
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--seed", type=int, default=20261027)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--task-id", help="Run one task for harness debugging")
    args = parser.parse_args()
    if args.repeats < 1 or not args.codex.is_file() or not args.memory_bin.is_file():
        parser.error("repeats must be positive and both executable paths must exist")
    tasks = parse_tasks(json.loads(TASKS_PATH.read_text(encoding="utf-8")))
    if args.task_id:
        tasks = [task for task in tasks if task.get("id") == args.task_id]
        if not tasks:
            parser.error("unknown task id")
    snapshot = hashlib.sha256(TASKS_PATH.read_bytes()).hexdigest()
    rng = random.Random(args.seed)
    rows = []
    with tempfile.TemporaryDirectory(prefix="codex-memory-fixture-") as root:
        root = Path(root)
        cwd = root / "project"
        cwd.mkdir()
        store = root / "store"
        prepare_store(store, cwd, tasks)
        for task in tasks:
            retrieved = retrieve(args.memory_bin, store, cwd, task)
            if not retrieved:
                raise RuntimeError(f"memory retrieval returned no note for task {task['id']}")
            for repeat in range(args.repeats):
                arms = [("no_memory", ""), ("memory", retrieved)]
                rng.shuffle(arms)
                for arm, context in arms:
                    row = run_trial(args.codex, task, arm, context)
                    row["repeat"] = repeat + 1
                    rows.append(row)
                    print(f"{task['id']} repeat {repeat + 1} {arm}: exit={row['exit_code']} correct={row['correct']} {row['elapsed_seconds']:.1f}s", flush=True)
                    partial = {
                        "method": "actual Codex gpt-6-luna subprocess recall QA; synthetic frozen fixture notes; no coding tasks",
                        "model": "gpt-6-luna", "repeats": args.repeats, "seed": args.seed,
                        "tasks_sha256": snapshot, "retrieval": "production codex-memory positional search then read against a temporary v2 Markdown store",
                        "isolation": "codex exec --ignore-user-config --ephemeral, temporary empty cwd, read-only sandbox, hooks/plugins/memories/shell/unified-exec/multi-agent/skill-search disabled; current user's authentication retained",
                        "usage_note": "Token/cost usage is null unless exposed; no cost inferred.",
                        "summary": summarize(rows), "trials": rows,
                    }
                    args.output.parent.mkdir(parents=True, exist_ok=True)
                    args.output.write_text(json.dumps(partial, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    result = {
        "method": "actual Codex gpt-6-luna subprocess recall QA; synthetic frozen fixture notes; no coding tasks",
        "model": "gpt-6-luna", "repeats": args.repeats, "seed": args.seed,
        "tasks_sha256": snapshot,
        "retrieval": "production codex-memory positional search then read against a temporary v2 Markdown store",
        "isolation": "codex exec --ignore-user-config --ephemeral, temporary empty cwd, read-only sandbox, hooks/plugins/memories/shell/unified-exec/multi-agent/skill-search disabled; current user's authentication retained",
        "usage_note": "Token/cost usage is null unless exposed; no cost inferred.",
        "summary": summarize(rows), "trials": rows,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps(result["summary"], indent=2))
    print(f"saved {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
