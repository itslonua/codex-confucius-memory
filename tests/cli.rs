use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};
fn run(args: &[&str], input: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_codex-memory"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn hook_failure_warns_without_blocking_codex() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(&["--store", dir.path().to_str().unwrap(), "hook"], "{}");
    assert!(out.status.success());
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(value["systemMessage"].as_str().unwrap().contains("memory"));
    assert!(value.get("continue").is_none());
}
#[test]
fn generated_hooks_have_correct_events_and_safe_quoting() {
    let out = run(&["--store", "/tmp/memory's store", "hooks"], "");
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["hooks"]["PreCompact"].is_array());
    assert_eq!(
        v["hooks"]["SessionStart"][0]["matcher"],
        "^(startup|resume|compact)$"
    );
    assert!(v["hooks"]["Stop"][0]["hooks"][0].get("async").is_none());
    assert!(
        v["hooks"]["PreCompact"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("'\\''")
    );
}
#[test]
fn fresh_session_does_not_invoke_external_model() {
    let dir = tempfile::tempdir().unwrap();
    let input = json!({"session_id":"fresh","cwd":dir.path(),"hook_event_name":"SessionStart","source":"startup"});
    let out = run(
        &[
            "--store",
            dir.path().join("store").to_str().unwrap(),
            "--codex",
            "/does/not/exist",
            "hook",
        ],
        &input.to_string(),
    );
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], "SessionStart");
    assert!(!dir.path().join("store").exists());
}

fn stored_notes(
    cwd: &std::path::Path,
    store: &std::path::Path,
    notes: Value,
) -> std::path::PathBuf {
    use sha2::{Digest, Sha256};
    let cwd = cwd.canonicalize().unwrap();
    let hash = format!("{:x}", Sha256::digest(cwd.to_str().unwrap().as_bytes()));
    std::fs::create_dir_all(store).unwrap();
    let path = store.join(format!("{hash}.json"));
    std::fs::write(
        &path,
        json!({"version":1,"sessions":{},"notes":notes}).to_string(),
    )
    .unwrap();
    path
}
fn note(id: &str, title: &str, body: &str) -> Value {
    json!({"id":id,"title":title,"body":body,"evidence":["cargo test unicode_boundary passed"]})
}
#[test]
fn search_and_read_are_scoped_read_only_and_do_not_use_model() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let id = "projects/parser/utf8";
    let expected = note(id, "Ошибка UTF-8", "Byte slicing caused the failure");
    let path = stored_notes(
        dir.path(),
        &store,
        json!({id:expected,"other":note("other","Other","unrelated detail")}),
    );
    let before = std::fs::read(&path).unwrap();
    let args = [
        "--store",
        store.to_str().unwrap(),
        "--cwd",
        dir.path().to_str().unwrap(),
        "--codex",
        "/missing/model",
    ];
    let out = run(
        &[
            args.as_slice(),
            &["search", "ОШИБКА slicing unicode_boundary"],
        ]
        .concat(),
        "",
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["matches"], json!([{"id":id,"title":"Ошибка UTF-8"}]));
    assert_eq!(result["total_matches"], 1);
    assert_eq!(result["truncated"], false);
    let out = run(&[args.as_slice(), &["read", id]].concat(), "");
    assert!(out.status.success());
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["note"], expected);
    assert!(
        !String::from_utf8(out.stdout)
            .unwrap()
            .contains("unrelated detail")
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let other = tempfile::tempdir().unwrap();
    let out = run(
        &[
            "--store",
            store.to_str().unwrap(),
            "--cwd",
            other.path().to_str().unwrap(),
            "search",
            "UTF-8",
        ],
        "",
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["total_matches"],
        0
    );
}
#[test]
fn search_is_bounded_and_missing_store_is_not_created() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let args = [
        "--store",
        store.to_str().unwrap(),
        "--cwd",
        dir.path().to_str().unwrap(),
        "search",
        "finding",
    ];
    assert!(run(&args, "").status.success());
    assert!(!store.exists());
    let notes: serde_json::Map<String, Value> = (0..25)
        .map(|i| {
            let id = format!("note-{i:02}");
            (id.clone(), note(&id, "Finding", "details"))
        })
        .collect();
    stored_notes(dir.path(), &store, json!(notes));
    let out = run(&args, "");
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["matches"].as_array().unwrap().len(), 20);
    assert_eq!(result["total_matches"], 25);
    assert_eq!(result["truncated"], true);
    assert_eq!(result["matches"][0]["id"], "note-00");
}
#[test]
fn retrieval_rejects_bad_input_and_missing_notes_without_mutations() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("absent");
    let args = [
        "--store",
        store.to_str().unwrap(),
        "--cwd",
        dir.path().to_str().unwrap(),
    ];
    for (command, value) in [
        ("search", "  "),
        ("read", "../secret"),
        ("read", "a//b"),
        ("read", "missing"),
        ("read", "hook"),
        ("search", "--cwd"),
    ] {
        let out = run(&[args.as_slice(), &[command, value]].concat(), "");
        assert!(!out.status.success(), "{command} {value}");
    }
    assert!(
        !run(
            &[args.as_slice(), &["search", &"x".repeat(513)]].concat(),
            ""
        )
        .status
        .success()
    );
    assert!(
        !run(&[args.as_slice(), &["read"]].concat(), "")
            .status
            .success()
    );
    assert!(!store.exists());
}
#[test]
fn retrieval_rejects_corrupt_identity_in_stored_notes() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let path = stored_notes(
        dir.path(),
        &store,
        json!({"wrong-key":note("actual-id","Finding","details")}),
    );
    let before = std::fs::read(&path).unwrap();
    let out = run(
        &[
            "--store",
            store.to_str().unwrap(),
            "--cwd",
            dir.path().to_str().unwrap(),
            "search",
            "finding",
        ],
        "",
    );
    assert!(!out.status.success());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn restored_read_command_works_from_another_directory_with_quoted_paths() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("project's directory");
    std::fs::create_dir(&cwd).unwrap();
    let store = dir.path().join("memory's store");
    let expected = note("projects/parser/utf8", "UTF-8", "Preserved discovery");
    stored_notes(&cwd, &store, json!({"projects/parser/utf8":expected}));
    let input =
        json!({"session_id":"fresh","cwd":cwd,"hook_event_name":"SessionStart","source":"startup"});
    let out = run(
        &["--store", store.to_str().unwrap(), "hook"],
        &input.to_string(),
    );
    assert!(out.status.success());
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    let context = value["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    let command = context
        .lines()
        .find(|line| line.ends_with(" read 'note-id'"))
        .unwrap()
        .replace("'note-id'", "'projects/parser/utf8'");
    let other = tempfile::tempdir().unwrap();
    let out = Command::new("sh")
        .args(["-c", &command])
        .current_dir(other.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["note"],
        expected
    );
}
