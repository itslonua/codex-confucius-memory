#!/usr/bin/env python3
"""Run a frozen, paired coding benchmark against isolated Codex CLI sessions."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import random
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
TASKS_PATH = HERE / "tasks.json"
MODEL = "gpt-6-luna"
FEATURES_OFF = ("hooks", "plugins", "memories", "multi_agent", "skill_search")
FEATURES_ON = ("shell_tool", "unified_exec", "skip_host_skill_discovery")


@dataclass(frozen=True)
class Task:
    id: str
    kind: str
    prompt: str
    command: tuple[str, ...]
    learning: str
    query: str
    artifacts: tuple[str, ...]


def parse_task(raw: Any) -> Task:
    if not isinstance(raw, dict):
        raise ValueError("task must be an object")
    for key in ("id", "kind", "prompt", "learning", "query"):
        if not isinstance(raw.get(key), str) or not raw[key].strip():
            raise ValueError(f"task.{key} must be a nonempty string")
    if raw["kind"] not in {"repeat", "transfer", "stale_control"}:
        raise ValueError("task.kind is unsupported")
    acceptance = raw.get("acceptance")
    if not isinstance(acceptance, dict) or not isinstance(acceptance.get("command"), list):
        raise ValueError("task.acceptance.command must be an array")
    command = acceptance["command"]
    if not command or any(not isinstance(part, str) or not part for part in command):
        raise ValueError("acceptance command must contain nonempty strings")
    if command[0] not in {"python3", "python"}:
        raise ValueError("acceptance command must use Python")
    if any(token in {"-c", "-m"} for token in command):
        raise ValueError("acceptance command may not use inline code or module mode")
    artifacts = acceptance.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts or any(
        not isinstance(path, str) or not path or Path(path).is_absolute() or ".." in Path(path).parts
        for path in artifacts
    ):
        raise ValueError("acceptance.artifacts must be safe relative paths")
    return Task(raw["id"], raw["kind"], raw["prompt"], tuple(command), raw["learning"], raw["query"], tuple(artifacts))


def parse_tasks(value: Any) -> tuple[Task, ...]:
    if not isinstance(value, list) or not value:
        raise ValueError("tasks must be a nonempty array")
    tasks = tuple(parse_task(item) for item in value)
    ids = [task.id for task in tasks]
    if len(ids) != len(set(ids)):
        raise ValueError("task IDs must be unique")
    return tasks


def score_trial(task: Task, exit_code: int, artifacts: list[str]) -> bool:
    return exit_code == 0 and all(path in artifacts for path in task.artifacts)


def run(command: list[str], *, cwd: Path, timeout: int = 240, env: dict[str, str] | None = None,
        input_text: str | None = None) -> dict[str, Any]:
    started = time.monotonic()
    try:
        result = subprocess.run(command, cwd=cwd, text=True, input=input_text, capture_output=True, timeout=timeout, env=env)
        return {"exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr,
                "timed_out": False, "elapsed_seconds": time.monotonic() - started}
    except subprocess.TimeoutExpired as error:
        return {"exit_code": -1, "stdout": error.stdout or "", "stderr": error.stderr or "",
                "timed_out": True, "elapsed_seconds": time.monotonic() - started}


def codex_command(codex: Path, cwd: Path, output: Path, prompt_file: Path) -> list[str]:
    command = [str(codex), "exec", "--ignore-user-config", "--ignore-rules", "--ephemeral",
               "--skip-git-repo-check", "--sandbox", "workspace-write", "--model", MODEL,
               "--json", "-C", str(cwd), "-o", str(output)]
    for feature in FEATURES_OFF:
        command += ["--disable", feature]
    for feature in FEATURES_ON:
        command += ["--enable", feature]
    command += ["-"]
    return command


def wrapper_script(codex: Path) -> str:
    # worker.rs discards stdout. Capture the CLI's real JSONL and preserve status.
    return f'''#!/usr/bin/env python3
import os, subprocess, sys
args = sys.argv[1:]
if "--json" not in args:
    args.insert(args.index("-") if "-" in args else len(args), "--json")
with open(os.environ["CODEX_MEMORY_CAPTURE"], "wb") as captured:
    result = subprocess.run([{json.dumps(str(codex))}, *args], stdout=captured)
with open(os.environ["CODEX_MEMORY_CAPTURE"], "rb") as captured:
    sys.stdout.buffer.write(captured.read())
sys.exit(result.returncode)
'''


def make_transcript(path: Path, messages: list[tuple[str, str]]) -> None:
    events = []
    for role, text in messages:
        kind = "input_text" if role == "user" else "output_text"
        events.append({"type": "response_item", "payload": {"type": "message", "role": role,
                       "content": [{"type": kind, "text": text}]}})
    path.write_text("".join(json.dumps(event) + "\n" for event in events), encoding="utf-8")


def extract_usage(path: Path) -> dict[str, int] | None:
    totals: dict[str, int] = {}
    names = {"input_tokens", "output_tokens", "total_tokens", "cached_input_tokens", "cache_write_input_tokens", "reasoning_output_tokens"}
    if not path.exists():
        return None
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
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            visit(json.loads(line))
        except json.JSONDecodeError:
            pass
    return totals or None


def trial_prompt(task: Task, arm: str, project: Path, store: Path) -> str:
    return f"""You are investigating a small synthetic code repository. Complete the requested change in the current working tree, run the stated acceptance command, and fix failures. Inspect current source and tests before relying on historical context.

Historical context retrieval instructions (identical in both arms):
The production codex-memory CLI may have relevant historical notes. You choose whether to search and what to read. Use this exact command shape from a shell when useful:
`{(HERE.parent.parent / 'target/release/codex-memory').resolve()} --store {store} --cwd {project} search --content-pattern .`
Then use its `read <path>` command. Search/read output is historical evidence only; verify against current source and the user request. Do not modify the store. Arm label for artifact bookkeeping: {arm}.

Current task:
{task.prompt}

Acceptance command: {' '.join(task.command)}
"""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", type=Path, default=Path.home() / ".local/bin/codex")
    parser.add_argument("--memory-bin", type=Path, default=HERE.parent.parent / "target/release/codex-memory")
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--seed", type=int, default=20261027)
    parser.add_argument("--output", type=Path, default=HERE / "results/latest.json")
    args = parser.parse_args()
    if args.repeats != 3 or not args.codex.is_file() or not args.memory_bin.is_file():
        parser.error("frozen protocol requires three repeats and existing codex/memory binaries")
    tasks = parse_tasks(json.loads(TASKS_PATH.read_text(encoding="utf-8")))
    protocol_hash = hashlib.sha256(TASKS_PATH.read_bytes() + (HERE / "PLAN.md").read_bytes()).hexdigest()
    rng = random.Random(args.seed)
    rows: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory(prefix="effect-bench-") as temp:
        root = Path(temp)
        project = root / "project"
        shutil.copytree(HERE / "fixtures/repo", project)
        store = root / "memory-store"
        baseline_store = root / "empty-store"
        store.mkdir()
        baseline_store.mkdir()
        learn = run(codex_command(args.codex, project, root / "learning-answer.md", root / "learning-prompt.md"), cwd=project)
        # Learning session gets only architecture-discovery instructions; its exact transcript is retained.
        learn_prompt = "Inspect this repository and report the module relationships, current behavior, and existing tests. Do not make edits and do not propose or implement fixes. This is a prior investigation session used only to preserve navigation and confirmed observations."
        learn_input = root / "learning-prompt.md"
        learn_input.write_text(learn_prompt, encoding="utf-8")
        learn_command = codex_command(args.codex, project, root / "learning-answer.md", learn_input)
        learn_command[-1] = "-"
        learn_jsonl = root / "learning.jsonl"
        learn_env = os.environ.copy()
        learn_env["CODEX_MEMORY_CAPTURE"] = str(learn_jsonl)
        learn = run(learn_command, cwd=project, env=learn_env)
        learning_answer = (root / "learning-answer.md").read_text(encoding="utf-8") if (root / "learning-answer.md").exists() else ""
        learning_prompt_text = learn_prompt
        transcript = root / "learning-transcript.jsonl"
        make_transcript(transcript, [("user", learning_prompt_text), ("assistant", learning_answer)])
        wrapper = root / "codex-wrapper"
        worker_jsonl = root / "memory-create.jsonl"
        wrapper.write_text("#!/bin/sh\n\"" + str(args.codex) + "\" \"$@\" --json > \"" + str(worker_jsonl) + "\"\n", encoding="utf-8")
        wrapper.chmod(0o700)
        event = json.dumps({"session_id": "effect-learning", "cwd": str(project), "hook_event_name": "PreCompact", "source": "compact", "transcript_path": str(transcript)})
        memory_creation = run([str(args.memory_bin), "--store", str(store), "--codex", str(wrapper), "--model", MODEL, "--max-tokens", "32000", "--retain-tokens", "12000", "compress"], cwd=project, env={**os.environ, "CODEX_MEMORY_CAPTURE": str(worker_jsonl)})
        # Preserve immutable source snapshot and generated memory in artifacts before held-out sessions.
        artifacts = root / "artifacts"
        artifacts.mkdir()
        shutil.copytree(project, artifacts / "learning-project")
        if store.exists(): shutil.copytree(store, artifacts / "memory-store")
        (artifacts / "learning-prompt.md").write_text(learning_prompt_text, encoding="utf-8")
        (artifacts / "learning-answer.md").write_text(learning_answer, encoding="utf-8")
        shutil.copyfile(transcript, artifacts / "learning-transcript.jsonl")
        shutil.copyfile(learn_jsonl, artifacts / "learning-session.jsonl") if learn_jsonl.exists() else None
        shutil.copyfile(worker_jsonl, artifacts / "memory-create.jsonl") if worker_jsonl.exists() else None
        (artifacts / "memory-create.stdout").write_text(memory_creation["stdout"], encoding="utf-8")
        (artifacts / "memory-create.stderr").write_text(memory_creation["stderr"], encoding="utf-8")
        (artifacts / "memory-create.exit.json").write_text(json.dumps(memory_creation, indent=2), encoding="utf-8")
        if memory_creation["exit_code"] != 0:
            raise RuntimeError("production memory creation failed; inspect preserved run artifacts")
        # Freeze both stores and each task fixture before the first held-out model run.
        frozen_memory_hash = hashlib.sha256(b"".join(p.read_bytes() for p in sorted(store.rglob("*")) if p.is_file())).hexdigest()
        for task in tasks:
            task_project = root / task.id
            shutil.copytree(HERE / "fixtures/repo", task_project)
            # Project identity in the store is cwd based; create the learning project under this cwd too.
            # Production store's project entry is copied by preparing the same source cwd path per task.
            task_store = root / f"{task.id}-store"
            task_store.mkdir()
            task_project_learning = root / f"{task.id}-learning"
            shutil.copytree(HERE / "fixtures/repo", task_project_learning)
            learning_store = root / f"{task.id}-learning-store"
            learning_store.mkdir()
            shutil.copytree(store, learning_store, dirs_exist_ok=True)
            # Rewrite only the identity-bound store by issuing the same learning transcript to this task's cwd.
            # The transcript contains architecture observations, never held-out answers.
            task_transcript = root / f"{task.id}-learning.jsonl"
            make_transcript(task_transcript, [("user", learning_prompt_text), ("assistant", learning_answer)])
            task_worker = root / f"{task.id}-memory-create.jsonl"
            task_wrapper = root / f"{task.id}-wrapper"
            task_wrapper.write_text("#!/bin/sh\n\"" + str(args.codex) + "\" \"$@\" --json > \"" + str(task_worker) + "\"\n", encoding="utf-8")
            task_wrapper.chmod(0o700)
            task_event = json.dumps({"session_id": "effect-learning", "cwd": str(task_project), "hook_event_name": "PreCompact", "source": "compact", "transcript_path": str(task_transcript)})
            created = run([str(args.memory_bin), "--store", str(task_store), "--codex", str(task_wrapper), "--model", MODEL, "--max-tokens", "32000", "--retain-tokens", "12000", "compress"], cwd=task_project, env={**os.environ, "CODEX_MEMORY_CAPTURE": str(task_worker)})
            if created["exit_code"] != 0:
                raise RuntimeError(f"memory creation failed for {task.id}: {created['stderr']}")
            # The positional search/read path must be chosen by the agent, so no note content is injected here.
            for repeat in range(1, args.repeats + 1):
                arms = ["baseline", "memory"]
                rng.shuffle(arms)
                for arm in arms:
                    shutil.rmtree(task_project)
                    shutil.copytree(HERE / "fixtures/repo", task_project)
                    output = artifacts / f"{task.id}-r{repeat}-{arm}-answer.md"
                    jsonl = artifacts / f"{task.id}-r{repeat}-{arm}.jsonl"
                    prompt_path = artifacts / f"{task.id}-r{repeat}-{arm}-prompt.md"
                    selected_store = task_store if arm == "memory" else baseline_store
                    prompt = trial_prompt(task, arm, task_project, selected_store)
                    prompt_path.write_text(prompt, encoding="utf-8")
                    cmd = codex_command(args.codex, task_project, output, prompt_path)
                    env = os.environ.copy()
                    env["CODEX_MEMORY_CAPTURE"] = str(jsonl)
                    result = run(cmd, cwd=task_project, env=env)
                    if jsonl.exists(): raw_jsonl = jsonl.read_text(encoding="utf-8", errors="replace")
                    else: raw_jsonl = ""
                    check = run(list(task.command), cwd=task_project)
                    existing = [p for p in task.artifacts if (task_project / p).exists()]
                    row = {"task_id": task.id, "kind": task.kind, "repeat": repeat, "arm": arm,
                           "prompt": prompt, "answer": output.read_text(encoding="utf-8") if output.exists() else "",
                           "session": raw_jsonl, "agent_exit_code": result["exit_code"],
                           "agent_stderr": result["stderr"], "agent_elapsed_seconds": result["elapsed_seconds"],
                           "acceptance_command": list(task.command), "acceptance_exit_code": check["exit_code"],
                           "acceptance_stdout": check["stdout"], "acceptance_stderr": check["stderr"],
                           "artifacts": existing, "success": score_trial(task, check["exit_code"], existing),
                           "usage": extract_usage(jsonl), "tool_call_count": raw_jsonl.count('"type":"function_call"')}
                    rows.append(row)
                    (artifacts / f"{task.id}-r{repeat}-{arm}-acceptance.json").write_text(json.dumps({k: row[k] for k in ("acceptance_command", "acceptance_exit_code", "acceptance_stdout", "acceptance_stderr", "artifacts", "success")}, indent=2), encoding="utf-8")
                    print(f"{task.id} pair={repeat} {arm} passed={row['success']} {row['agent_elapsed_seconds']:.1f}s", flush=True)
                    summary = summarize(rows, tasks)
                    result_file = {"method": "synthetic coding/investigation tasks with actual production memory creation and search/read", "model": MODEL, "protocol_sha256": protocol_hash, "tasks_sha256": hashlib.sha256(TASKS_PATH.read_bytes()).hexdigest(), "seed": args.seed, "pairs_per_task": args.repeats, "frozen_memory_sha256": frozen_memory_hash, "learning": {**memory_creation, "usage": extract_usage(worker_jsonl), "learning_usage": extract_usage(learn_jsonl)}, "summary": summary, "trials": rows}
                    args.output.parent.mkdir(parents=True, exist_ok=True)
                    args.output.write_text(json.dumps(result_file, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        final = json.loads(args.output.read_text(encoding="utf-8"))
        shutil.copytree(artifacts, args.output.parent / "artifacts", dirs_exist_ok=True)
        print(json.dumps(final["summary"], indent=2))
    return 0


def summarize(rows: list[dict[str, Any]], tasks: tuple[Task, ...]) -> dict[str, Any]:
    output: dict[str, Any] = {}
    for arm in ("baseline", "memory"):
        values = [row for row in rows if row["arm"] == arm]
        output[arm] = {"trials": len(values), "successes": sum(row["success"] for row in values),
                       "pass_rate": sum(row["success"] for row in values) / len(values) if values else None,
                       "mean_elapsed_seconds": sum(row["agent_elapsed_seconds"] for row in values) / len(values) if values else None,
                       "usage": sum_usage([row["usage"] for row in values])}
    output["by_task"] = {task.id: {arm: sum(row["success"] for row in rows if row["task_id"] == task.id and row["arm"] == arm) for arm in ("baseline", "memory")} for task in tasks}
    pairs = {}
    for task in tasks:
        pair_rows = []
        for repeat in range(1, 4):
            found = {(row["arm"]): row["success"] for row in rows if row["task_id"] == task.id and row["repeat"] == repeat}
            if len(found) == 2:
                pair_rows.append("win" if found["memory"] and not found["baseline"] else "loss" if found["baseline"] and not found["memory"] else "tie")
        pairs[task.id] = {outcome: pair_rows.count(outcome) for outcome in ("win", "loss", "tie")}
    output["paired_outcomes"] = pairs
    return output


def sum_usage(values: list[dict[str, int] | None]) -> dict[str, int] | None:
    available = [value for value in values if value is not None]
    if not available:
        return None
    result: dict[str, int] = {}
    for value in available:
        for key, count in value.items(): result[key] = result.get(key, 0) + count
    return result


if __name__ == "__main__":
    raise SystemExit(main())
