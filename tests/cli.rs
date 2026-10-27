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
