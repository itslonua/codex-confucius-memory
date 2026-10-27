#!/usr/bin/env python3
"""Run paired sessions with automatic SessionStart-style memory restoration."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import random
import re
import shutil
import subprocess
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
TASKS_PATH = HERE / "tasks.json"
MODEL = "gpt-6-luna"
OFF = ("hooks", "plugins", "memories", "multi_agent", "skill_search")
ON = ("shell_tool", "unified_exec", "skip_host_skill_discovery")


@dataclass(frozen=True)
class Task:
    id: str
    kind: str
    prompt: str
    query: str
    oracle: str
    artifacts: tuple[str, ...]


def parse_task(value: Any) -> Task:
    if not isinstance(value, dict):
        raise ValueError("task must be an object")
    for key in ("id", "kind", "prompt", "query"):
        if not isinstance(value.get(key), str) or not value[key].strip():
            raise ValueError(f"task.{key} must be a nonempty string")
    if value["kind"] not in {"repeat", "transfer", "stale_control"}:
        raise ValueError("unsupported task kind")
    acceptance = value.get("acceptance")
    if not isinstance(acceptance, dict):
        raise ValueError("acceptance must be an object")
    oracle = acceptance.get("oracle")
    artifacts = acceptance.get("artifacts")
    if not isinstance(oracle, str) or Path(oracle).name != oracle or not oracle.endswith(".py"):
        raise ValueError("acceptance.oracle must be a Python filename")
    if not isinstance(artifacts, list) or not artifacts or any(
        not isinstance(item, str) or not item or Path(item).is_absolute() or ".." in Path(item).parts
        for item in artifacts
    ):
        raise ValueError("acceptance.artifacts must be safe relative paths")
    return Task(value["id"], value["kind"], value["prompt"], value["query"], oracle, tuple(artifacts))


def parse_tasks(value: Any) -> tuple[Task, ...]:
    if not isinstance(value, list) or not value:
        raise ValueError("tasks must be a nonempty array")
    tasks = tuple(parse_task(item) for item in value)
    ids = [task.id for task in tasks]
    if len(ids) != len(set(ids)):
        raise ValueError("task IDs must be unique")
    return tasks


def score_trial(task: Task, acceptance_exit_code: int, changed_files: list[str]) -> bool:
    return acceptance_exit_code == 0 and all(path in changed_files for path in task.artifacts)


def parse_events(stdout: str) -> list[dict[str, Any]]:
    events = []
    for line in stdout.splitlines():
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            events.append(value)
    return events


def usage(events: list[dict[str, Any]]) -> dict[str, int] | None:
    names = {"input_tokens", "output_tokens", "total_tokens", "cached_input_tokens", "cache_write_input_tokens", "reasoning_output_tokens"}
    totals: dict[str, int] = {}
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
    for event in events:
        visit(event)
    return totals or None


def memory_calls(events: list[dict[str, Any]], executable: Path) -> dict[str, int]:
    counts = {"restore_calls": 0, "restore_successes": 0, "search_calls": 0,
              "search_results": 0, "read_calls": 0, "read_successes": 0}
    for event in events:
        item = event.get("item")
        if event.get("type") != "item.completed" or not isinstance(item, dict):
            continue
        if item.get("type") != "command_execution":
            continue
        command = item.get("command")
        if not isinstance(command, str) or str(executable) not in command:
            continue
        exit_code = item.get("exit_code")
        output = item.get("aggregated_output", "")
        if re.search(r"(?:^|\s)restore(?:\s|$)", command):
            counts["restore_calls"] += 1
            counts["restore_successes"] += exit_code == 0
        elif re.search(r"(?:^|\s)search(?:\s|$)", command):
            counts["search_calls"] += 1
            match = re.search(r"\{[\s\S]*\}", output) if isinstance(output, str) else None
            if match:
                try:
                    result = json.loads(match.group())
                    found = result.get("results", result.get("matches", []))
                    counts["search_results"] += len(found) if isinstance(found, list) else 0
                except json.JSONDecodeError:
                    pass
        elif re.search(r"(?:^|\s)read(?:\s|$)", command):
            counts["read_calls"] += 1
            counts["read_successes"] += exit_code == 0
    return counts


def invoke(command: list[str], cwd: Path, *, input_text: str | None = None,
           env: dict[str, str] | None = None, timeout: int = 240) -> dict[str, Any]:
    start = time.monotonic()
    try:
        result = subprocess.run(command, cwd=cwd, input=input_text, text=True, capture_output=True,
                                timeout=timeout, env=env)
        return {"exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr,
                "timed_out": False, "elapsed_seconds": time.monotonic() - start}
    except subprocess.TimeoutExpired as error:
        def decode(value: str | bytes | None) -> str:
            return value.decode(errors="replace") if isinstance(value, bytes) else value or ""
        return {"exit_code": -1, "stdout": decode(error.stdout), "stderr": decode(error.stderr),
                "timed_out": True, "elapsed_seconds": time.monotonic() - start}


def codex_command(codex: Path, cwd: Path, output: Path) -> list[str]:
    command = [str(codex), "exec", "--ignore-user-config", "--ignore-rules", "--ephemeral",
               "--skip-git-repo-check", "--sandbox", "workspace-write", "--model", MODEL,
               "--json", "-C", str(cwd), "-o", str(output)]
    for flag in OFF:
        command += ["--disable", flag]
    for flag in ON:
        command += ["--enable", flag]
    return command + ["-"]


def transcript(path: Path, prompt: str, events: list[dict[str, Any]], answer: str) -> None:
    """Adapt actual CLI message/tool events to the rollout JSONL read by compress."""
    records: list[dict[str, Any]] = [{"type": "response_item", "payload": {
        "type": "message", "role": "user", "content": [{"type": "input_text", "text": prompt}]}}]
    for event in events:
        item = event.get("item")
        if event.get("type") != "item.completed" or not isinstance(item, dict):
            continue
        item_type = item.get("type")
        if item_type == "agent_message" and isinstance(item.get("text"), str):
            records.append({"type": "response_item", "payload": {"type": "message", "role": "assistant",
                            "content": [{"type": "output_text", "text": item["text"]}]}})
        elif item_type == "command_execution":
            call_id = item.get("id")
            if not isinstance(call_id, str):
                continue
            command = item.get("command", "")
            records.append({"type": "response_item", "payload": {"type": "function_call", "name": "shell",
                            "arguments": {"command": command}}})
            output = item.get("aggregated_output", "")
            exit_code = item.get("exit_code")
            records.append({"type": "response_item", "payload": {"type": "function_call_output",
                            "call_id": call_id, "output": f"{output}\nexit_code={exit_code}"}})
    if not any(record.get("payload", {}).get("role") == "assistant" for record in records):
        records.append({"type": "response_item", "payload": {"type": "message", "role": "assistant",
                        "content": [{"type": "output_text", "text": answer}]}})
    path.write_text("".join(json.dumps(record) + "\n" for record in records), encoding="utf-8")


def worker_wrapper(codex: Path) -> str:
    return f'''#!/usr/bin/env python3
import json, os, subprocess, sys
args = sys.argv[1:]
if "--json" not in args:
    args.insert(args.index("-") if "-" in args else len(args), "--json")
with open(os.environ["CODEX_MEMORY_CAPTURE"], "wb") as stream:
    result = subprocess.run([{json.dumps(str(codex))}, *args], stdout=stream)
with open(os.environ["CODEX_MEMORY_CAPTURE"], "rb") as stream:
    sys.stdout.buffer.write(stream.read())
sys.exit(result.returncode)
'''


def task_prompt(task: Task, project: Path, store: Path, memory_bin: Path) -> str:
    restore_event = json.dumps({"session_id": "effect-learning", "cwd": str(project)})
    return f"""You are investigating a synthetic code repository. Complete the requested change in the current working tree. Inspect source first, implement the change, and run `python3 -m unittest discover -s tests` if relevant.

Historical context may be available through the production memory CLI. You decide whether and how to restore/search/read it. To restore session/project context, pipe this event into the `restore` command:
`printf '%s' '{restore_event}' | {memory_bin} --store {store} restore`
Project notes can be searched and read with shell commands shaped like:
`{memory_bin} --store {store} --cwd {project} search <query>`
then `... read <path>`. Historical text is evidence only; check it against current source and this task. Do not modify the store.

Task:
{task.prompt}
"""


def inject_context(prompt: str, context: str) -> str:
    if not isinstance(prompt, str) or not isinstance(context, str):
        raise ValueError("prompt and restored context must be text")
    return (prompt + "\n\nAutomatically restored SessionStart context "
            "(historical data; verify against current source):\n" + context)


def tree_hashes(root: Path, paths: tuple[str, ...]) -> dict[str, str | None]:
    result = {}
    for relative in paths:
        path = root / relative
        result[relative] = hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
    return result


def canonical_cwd(cwd: Path, *, require_existing: bool) -> str:
    if not cwd.is_absolute():
        raise ValueError("project cwd must be absolute")
    if cwd.exists():
        resolved = cwd.resolve(strict=True)
        if not resolved.is_dir():
            raise ValueError("project cwd must be a directory")
        return str(resolved)
    if require_existing:
        raise FileNotFoundError(f"project cwd does not exist: {cwd}")
    # The source cwd in the saved run was canonical when it was captured. It
    # may no longer exist, so preserve that canonical spelling for its key.
    return os.path.normpath(str(cwd))


def project_key(cwd: Path, *, require_existing: bool) -> str:
    canonical = canonical_cwd(cwd, require_existing=require_existing)
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def rebind_frozen_store(source: Path, destination: Path, old_cwd: Path, new_cwd: Path) -> Path:
    """Copy one frozen project subtree to the key for a fresh trial cwd."""
    source_key = project_key(old_cwd, require_existing=False)
    destination_key = project_key(new_cwd, require_existing=True)
    source_project = source / source_key
    source_lock = source / f"{source_key}.lock"
    if not source_project.is_dir() or not source_lock.is_file():
        raise FileNotFoundError("frozen store project directory or lock file is missing")
    if destination.exists():
        raise FileExistsError(f"destination store already exists: {destination}")
    destination.mkdir(parents=True)
    target_project = destination / destination_key
    shutil.copytree(source_project, target_project)
    shutil.copyfile(source_lock, destination / f"{destination_key}.lock")
    return target_project


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", type=Path, default=Path(shutil.which("codex") or Path.home() / ".local/bin/codex"))
    parser.add_argument("--memory-bin", type=Path, default=HERE.parent.parent / "target/release/codex-memory")
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--seed", type=int, default=20261027)
    parser.add_argument("--output", type=Path, default=HERE / "results/latest.json")
    parser.add_argument("--preflight-only", action="store_true")
    args = parser.parse_args()
    try:
        args.codex = args.codex.expanduser().resolve(strict=True)
        args.memory_bin = args.memory_bin.expanduser().resolve(strict=True)
        args.output = args.output.expanduser().resolve()
    except OSError as error:
        parser.error(str(error))
    if args.repeats != 3 or not args.codex.is_file() or not args.memory_bin.is_file():
        parser.error("frozen protocol requires three repeats and existing binaries")
    tasks = parse_tasks(json.loads(TASKS_PATH.read_text(encoding="utf-8")))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    artifacts = args.output.parent / "artifacts"
    artifacts.mkdir(parents=True, exist_ok=True)
    def digest_tree(path: Path) -> str:
        digest = hashlib.sha256()
        for entry in sorted(path.rglob("*")):
            if entry.is_file():
                digest.update(entry.relative_to(path).as_posix().encode())
                digest.update(entry.read_bytes())
        return digest.hexdigest()
    fixture_digest = digest_tree(HERE / "fixtures/repo")
    learning_fixture_digest = digest_tree(HERE / "fixtures/learning_repo")
    oracle_digest = digest_tree(HERE / "oracle")
    task_digest = hashlib.sha256(TASKS_PATH.read_bytes()).hexdigest()
    runner_digest = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    plan_digest = hashlib.sha256((HERE / "PLAN_CONTINUATION.md").read_bytes()).hexdigest()
    protocol_hash = hashlib.sha256(TASKS_PATH.read_bytes() + (HERE / "PLAN.md").read_bytes()
                                   + (HERE / "PLAN_CONTINUATION.md").read_bytes()
                                   + fixture_digest.encode() + learning_fixture_digest.encode() + oracle_digest.encode()
                                   + runner_digest.encode()).hexdigest()
    rng = random.Random(args.seed)
    rows: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory(prefix="effect-bench-") as temp:
        root = Path(temp)
        project = root / "project"
        shutil.copytree(HERE / "fixtures/repo", project)
        store, active_store = root / "memory-store", root / "active-store"
        active_store.mkdir()
        prior_preflight = json.loads((HERE / "results/artifacts/memory-search-preflight.json").read_text(encoding="utf-8"))
        historical_cwd = Path(prior_preflight["search"]["cwd"])
        prior_result_path = HERE / "results/full-run.json"
        source_store = HERE / "results/artifacts/memory-store"
        prior_result = json.loads(prior_result_path.read_text(encoding="utf-8"))
        rebound_project = rebind_frozen_store(source_store, store, historical_cwd, project)
        memory_digest = hashlib.sha256(b"".join(path.read_bytes() for path in sorted(store.rglob("*")) if path.is_file())).hexdigest()
        if memory_digest != prior_result["frozen_memory_sha256"]:
            raise RuntimeError("reused frozen store does not match the completed optional-retrieval run")
        preflight = prior_result["memory_preflight"]
        learning_usage = prior_result.get("learning_usage")
        memory_creation_usage = prior_result.get("memory_creation_usage")
        learning_elapsed = prior_result.get("learning_elapsed_seconds")
        memory_creation_elapsed = prior_result.get("memory_creation_elapsed_seconds")
        shutil.copyfile(prior_result_path, artifacts / "source-optional-run.json")
        shutil.copytree(store, artifacts / "frozen-memory-store", dirs_exist_ok=True)
        (artifacts / "run-status.json").write_text(
            json.dumps({"state": "ready_for_trials", "completed_trials": 0,
                        "memory_preflight": preflight,
                        "reused_from": str(prior_result_path),
                        "source_project_cwd": str(historical_cwd),
                        "trial_project_cwd": str(project),
                        "rebound_project_key": rebound_project.name,
                        "frozen_store_sha256": memory_digest}, indent=2), encoding="utf-8")
        event_text = json.dumps({"session_id": "effect-learning", "cwd": str(project)})
        empty_restore = invoke([str(args.memory_bin), "--store", str(active_store), "restore"],
                               project, input_text=event_text)
        treatment_restore = invoke([str(args.memory_bin), "--store", str(store), "restore"],
                                   project, input_text=event_text)
        if empty_restore["exit_code"] or treatment_restore["exit_code"]:
            raise RuntimeError("automatic restore preflight failed")
        empty_context = json.loads(empty_restore["stdout"]).get("hookSpecificOutput", {}).get("additionalContext", "")
        treatment_context = json.loads(treatment_restore["stdout"]).get("hookSpecificOutput", {}).get("additionalContext", "")
        if "Task summary" in empty_context or "key_mismatch.md" in empty_context:
            raise RuntimeError("baseline restore unexpectedly has summary or project note index")
        if "Task summary" not in treatment_context or "key_mismatch.md" not in treatment_context:
            raise RuntimeError("treatment restore did not expose the saved summary and note index")
        baseline_search = invoke([str(args.memory_bin), "--store", str(active_store), "--cwd", str(project),
                                  "search", "--content-pattern", "."], project)
        treatment_search = invoke([str(args.memory_bin), "--store", str(store), "--cwd", str(project),
                                   "search", "--content-pattern", "."], project)
        if baseline_search["exit_code"] or treatment_search["exit_code"]:
            raise RuntimeError("production note search preflight failed")
        baseline_matches = json.loads(baseline_search["stdout"]).get("results", [])
        treatment_matches = json.loads(treatment_search["stdout"]).get("results", [])
        if baseline_matches or len(treatment_matches) != 1:
            raise RuntimeError("empty/frozen store note-search preflight was unexpected")
        frozen_note = prior_preflight["reads"][0]["content"]
        treatment_note_path = treatment_matches[0]["path"]
        treatment_read = invoke([str(args.memory_bin), "--store", str(store), "--cwd", str(project),
                                 "read", treatment_note_path], project)
        if treatment_read["exit_code"]:
            raise RuntimeError("frozen note read preflight failed")
        treatment_note = json.loads(treatment_read["stdout"]).get("content")
        if treatment_note != frozen_note:
            raise RuntimeError("frozen note content differs from the source run")
        automatic_preflight = {"empty_restore": empty_restore["stdout"],
                               "treatment_restore": treatment_restore["stdout"],
                               "baseline_search": baseline_search["stdout"],
                               "treatment_search": treatment_search["stdout"],
                               "treatment_read": treatment_read["stdout"],
                               "note_sha256": hashlib.sha256(treatment_note.encode()).hexdigest(),
                               "note_path": treatment_note_path, "all_assertions_passed": True}
        (artifacts / "automatic-restore-preflight.json").write_text(
            json.dumps(automatic_preflight, indent=2, ensure_ascii=False), encoding="utf-8")
        if args.preflight_only:
            (artifacts / "run-status.json").write_text(
                json.dumps({"state": "preflight_complete_no_trials", "completed_trials": 0,
                            "automatic_restore_preflight": automatic_preflight,
                            "frozen_memory_sha256": memory_digest}, indent=2), encoding="utf-8")
            print(json.dumps({"preflight": automatic_preflight, "trials_started": 0}, indent=2))
            return 0
        # The held-out snapshot supersedes the policy in the learning snapshot.
        shutil.rmtree(project)
        shutil.copytree(HERE / "fixtures/repo", project)
        for task in tasks:
            paths = tuple(dict.fromkeys(task.artifacts))
            base_hashes = tree_hashes(HERE / "fixtures/repo", paths)
            for repeat in range(1, args.repeats + 1):
                arms = ["baseline", "memory"]
                rng.shuffle(arms)
                for arm in arms:
                    shutil.rmtree(project)
                    shutil.copytree(HERE / "fixtures/repo", project)
                    shutil.rmtree(active_store)
                    active_store.mkdir()
                    if arm == "memory":
                        shutil.copytree(store, active_store, dirs_exist_ok=True)
                    base_prompt = task_prompt(task, project, active_store, args.memory_bin.resolve())
                    restore_event = json.dumps({"session_id": "effect-learning", "cwd": str(project)})
                    restore_start = time.monotonic()
                    restore_result = invoke([str(args.memory_bin), "--store", str(active_store), "restore"],
                                            project, input_text=restore_event)
                    restore_elapsed = time.monotonic() - restore_start
                    if restore_result["exit_code"] != 0:
                        raise RuntimeError(f"automatic memory restore failed: {restore_result['stderr']}")
                    restore_json = json.loads(restore_result["stdout"])
                    injected_context = restore_json.get("hookSpecificOutput", {}).get("additionalContext", "")
                    if not isinstance(injected_context, str):
                        raise ValueError("restore additionalContext must be text")
                    prompt = inject_context(base_prompt, injected_context)
                    output = artifacts / f"{task.id}-r{repeat}-{arm}-answer.md"
                    jsonl_path = artifacts / f"{task.id}-r{repeat}-{arm}.jsonl"
                    prompt_path = artifacts / f"{task.id}-r{repeat}-{arm}-prompt.md"
                    prompt_path.write_text(prompt, encoding="utf-8")
                    trial_env = os.environ.copy()
                    trial_env["PYTHONDONTWRITEBYTECODE"] = "1"
                    result = invoke(codex_command(args.codex, project, output), project,
                                    input_text=prompt, env=trial_env)
                    events = parse_events(result["stdout"])
                    jsonl_path.write_text(result["stdout"], encoding="utf-8")
                    changed = tree_hashes(project, paths)
                    changed_paths = [path for path in paths if changed[path] != base_hashes[path]]
                    baseline_files = {path.relative_to(HERE / "fixtures/repo").as_posix()
                                      for path in (HERE / "fixtures/repo").rglob("*") if path.is_file()}
                    trial_files = {path.relative_to(project).as_posix() for path in project.rglob("*") if path.is_file()}
                    unrequested_changes = []
                    for path in sorted(baseline_files | trial_files):
                        if path not in paths and tree_hashes(HERE / "fixtures/repo", (path,))[path] != tree_hashes(project, (path,))[path]:
                            unrequested_changes.append(path)
                    check_env = os.environ.copy()
                    check_env["PYTHONPATH"] = str(project)
                    check_env["PYTHONDONTWRITEBYTECODE"] = "1"
                    check = invoke(["python3", str(HERE / "oracle" / task.oracle), str(project)], root, env=check_env)
                    passed = (result["exit_code"] == 0 and not unrequested_changes
                              and score_trial(task, check["exit_code"], changed_paths))
                    row = {"task_id": task.id, "kind": task.kind, "repeat": repeat, "arm": arm,
                           "prompt": prompt, "answer": output.read_text(encoding="utf-8") if output.exists() else "",
                           "jsonl": result["stdout"], "agent_stderr": result["stderr"],
                           "agent_exit_code": result["exit_code"], "agent_timed_out": result["timed_out"],
                           "elapsed_seconds_including_retrieval": restore_elapsed + result["elapsed_seconds"],
                           "automatic_restore_stdout": restore_result["stdout"],
                           "automatic_restore_stderr": restore_result["stderr"],
                           "automatic_restore_elapsed_seconds": restore_elapsed,
                           "automatic_context": injected_context,
                           "base_prompt_sha256": hashlib.sha256(base_prompt.encode()).hexdigest(),
                           "injected_prompt_sha256": hashlib.sha256(prompt.encode()).hexdigest(),
                           "usage": usage(events), "memory_retrieval": memory_calls(events, args.memory_bin.resolve()),
                           "tool_call_events": sum(
                               1 for event in events
                               if event.get("type") == "item.completed"
                               and isinstance(event.get("item"), dict)
                               and event["item"].get("type") == "command_execution"),
                           "changed_files": changed_paths, "unrequested_changes": unrequested_changes,
                           "acceptance_exit_code": check["exit_code"],
                           "acceptance_stdout": check["stdout"], "acceptance_stderr": check["stderr"], "success": passed}
                    rows.append(row)
                    shutil.copytree(project, artifacts / f"{task.id}-r{repeat}-{arm}-project")
                    (artifacts / f"{task.id}-r{repeat}-{arm}-acceptance.json").write_text(json.dumps({key: row[key] for key in ("changed_files", "acceptance_exit_code", "acceptance_stdout", "acceptance_stderr", "success")}, indent=2), encoding="utf-8")
                    result_file = {"method": "paired synthetic code investigations; production compress followed by automatic SessionStart restore into each trial prompt", "model": MODEL,
                                   "codex_command": "codex exec --ignore-user-config --ignore-rules --ephemeral --sandbox workspace-write --json; hooks/plugins/memories/multi_agent/skill_search disabled; shell/unified_exec enabled",
                                   "reasoning_effort": "Codex CLI default, same for all sessions", "protocol_sha256": protocol_hash,
                                   "task_sha256": task_digest, "fixture_sha256": fixture_digest,
                                   "learning_fixture_sha256": learning_fixture_digest,
                                   "oracle_sha256": oracle_digest, "runner_sha256": runner_digest,
                                   "continuation_plan_sha256": plan_digest,
                                   "seed": args.seed, "repeats_per_task": args.repeats, "frozen_memory_sha256": memory_digest,
                                   "memory_preflight": preflight,
                                   "automatic_restore_preflight": automatic_preflight,
                                   "learning_usage": learning_usage, "memory_creation_usage": memory_creation_usage,
                                   "learning_elapsed_seconds": learning_elapsed, "memory_creation_elapsed_seconds": memory_creation_elapsed,
                                   "learning_and_creation_reused_from": str(HERE / "results/full-run.json"),
                                   "summary": summarize(rows, tasks), "trials": rows}
                    args.output.parent.mkdir(parents=True, exist_ok=True)
                    args.output.write_text(json.dumps(result_file, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
                    (artifacts / "run-status.json").write_text(json.dumps({"state": "trials_in_progress", "completed_trials": len(rows), "memory_preflight": preflight}, indent=2), encoding="utf-8")
                    print(f"{task.id} pair={repeat} {arm}: passed={passed} elapsed={row['elapsed_seconds_including_retrieval']:.1f}s restored_chars={len(injected_context)}", flush=True)
        final_memory_digest = hashlib.sha256(b"".join(path.read_bytes() for path in sorted(store.rglob("*")) if path.is_file())).hexdigest()
        if final_memory_digest != memory_digest:
            raise RuntimeError("frozen treatment store changed during held-out trials")
        final_runner_digest = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
        final_plan_digest = hashlib.sha256((HERE / "PLAN_CONTINUATION.md").read_bytes()).hexdigest()
        final_fixture_digest = digest_tree(HERE / "fixtures/repo")
        final_learning_digest = digest_tree(HERE / "fixtures/learning_repo")
        final_oracle_digest = digest_tree(HERE / "oracle")
        final_task_digest = hashlib.sha256(TASKS_PATH.read_bytes()).hexdigest()
        if (final_runner_digest != runner_digest or final_plan_digest != plan_digest
                or final_fixture_digest != fixture_digest or final_learning_digest != learning_fixture_digest
                or final_oracle_digest != oracle_digest
                or final_task_digest != task_digest):
            raise RuntimeError("continuation protocol source changed during held-out trials")
        (artifacts / "run-status.json").write_text(json.dumps({"state": "complete", "completed_trials": len(rows), "memory_preflight": preflight}, indent=2), encoding="utf-8")
        print(json.dumps(summarize(rows, tasks), indent=2))
    return 0


def summarize(rows: list[dict[str, Any]], tasks: tuple[Task, ...]) -> dict[str, Any]:
    output = {}
    for arm in ("baseline", "memory"):
        trials = [row for row in rows if row["arm"] == arm]
        retrieval = {key: sum(row["memory_retrieval"][key] for row in trials)
                     for key in ("restore_calls", "restore_successes", "search_calls", "search_results", "read_calls", "read_successes")}
        output[arm] = {"trials": len(trials), "successes": sum(row["success"] for row in trials),
                       "pass_rate": sum(row["success"] for row in trials) / len(trials) if trials else None,
                       "mean_elapsed_seconds": sum(row["elapsed_seconds_including_retrieval"] for row in trials) / len(trials) if trials else None,
                       "usage": sum_usage([row["usage"] for row in trials]), "memory_retrieval": retrieval}
    output["by_task"] = {}
    output["paired_outcomes"] = {}
    for task in tasks:
        for arm in ("baseline", "memory"):
            output["by_task"].setdefault(task.id, {})[arm] = sum(row["success"] for row in rows if row["task_id"] == task.id and row["arm"] == arm)
        outcomes = []
        for repeat in range(1, 4):
            pair = {row["arm"]: row["success"] for row in rows if row["task_id"] == task.id and row["repeat"] == repeat}
            if len(pair) == 2:
                outcomes.append("win" if pair["memory"] and not pair["baseline"] else "loss" if pair["baseline"] and not pair["memory"] else "tie")
        output["paired_outcomes"][task.id] = {name: outcomes.count(name) for name in ("win", "loss", "tie")}
    return output


def sum_usage(rows: list[dict[str, int] | None]) -> dict[str, int] | None:
    present = [row for row in rows if row is not None]
    if not present:
        return None
    total: dict[str, int] = {}
    for row in present:
        for key, value in row.items():
            total[key] = total.get(key, 0) + value
    return total


if __name__ == "__main__":
    raise SystemExit(main())
