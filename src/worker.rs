//! Isolated CLI boundary for generating memory without recursively invoking hooks.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MAX_OUTPUT_BYTES: u64 = 1024 * 1024;
const MAX_DIAGNOSTIC_BYTES: u64 = 8192;

pub fn generate(
    prompt: &str,
    schema: &serde_json::Value,
    executable: &Path,
    model: Option<&str>,
    timeout: Duration,
) -> Result<String, String> {
    if timeout.is_zero() {
        return Err("worker timeout must be positive".into());
    }
    // Resolve before changing directories, including executables passed as ./codex.
    let executable = if executable.is_relative() && executable.components().count() > 1 {
        std::env::current_dir()
            .map_err(|error| format!("cannot resolve worker executable: {error}"))?
            .join(executable)
    } else {
        executable.to_path_buf()
    };
    let directory =
        tempfile::tempdir().map_err(|error| format!("cannot create worker directory: {error}"))?;
    let schema_path = directory.path().join("schema.json");
    let output_path = directory.path().join("output.json");
    let diagnostic_path = directory.path().join("stderr.log");
    let prompt_path = directory.path().join("prompt.txt");
    fs::write(&schema_path, schema.to_string())
        .and_then(|()| fs::write(&prompt_path, prompt))
        .map_err(|error| format!("cannot prepare worker input: {error}"))?;
    let input =
        File::open(&prompt_path).map_err(|error| format!("cannot open worker input: {error}"))?;
    let diagnostic = File::create(&diagnostic_path)
        .map_err(|error| format!("cannot create worker diagnostic: {error}"))?;

    let mut command = Command::new(executable);
    command.current_dir(directory.path()).args([
        "exec",
        "--ignore-user-config",
        "--ephemeral",
        "--skip-git-repo-check",
        "--sandbox",
        "read-only",
    ]);
    for feature in [
        "hooks",
        "plugins",
        "shell_tool",
        "unified_exec",
        "multi_agent",
        "skill_search",
    ] {
        command.args(["--disable", feature]);
    }
    command
        .args(["--enable", "skip_host_skill_discovery"])
        .arg("--output-schema")
        .arg(schema_path)
        .arg("-o")
        .arg(&output_path);
    if let Some(model) = model {
        command.args(["--model", model]);
    }
    command
        .arg("-")
        .stdin(Stdio::from(input))
        .stdout(Stdio::null())
        .stderr(Stdio::from(diagnostic));
    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot start Codex memory worker: {error}"))?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("cannot wait for Codex memory worker: {error}"));
            }
        }
        let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Codex memory worker timed out after {timeout:?}"));
        };
        std::thread::sleep(remaining.min(Duration::from_millis(20)));
    };
    if !status.success() {
        let diagnostic = read_bounded(&diagnostic_path, MAX_DIAGNOSTIC_BYTES)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();
        return Err(format!(
            "Codex memory worker failed ({status}): {diagnostic}"
        ));
    }
    let output = read_bounded(&output_path, MAX_OUTPUT_BYTES + 1)
        .map_err(|error| format!("cannot read Codex memory worker output: {error}"))?;
    if output.len() as u64 > MAX_OUTPUT_BYTES {
        return Err("Codex memory worker output exceeds 1 MiB".into());
    }
    let output = String::from_utf8(output)
        .map_err(|error| format!("Codex memory worker output is not UTF-8: {error}"))?;
    if output.trim().is_empty() {
        return Err("Codex memory worker returned empty output".into());
    }
    Ok(output)
}

fn read_bounded(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(limit).read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    fn fixture(body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("fake codex");
        let mut file = File::create(&executable).unwrap();
        writeln!(file, "#!/bin/sh\nset -eu\n{body}").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        (directory, executable)
    }

    #[test]
    fn passes_input_schema_model_and_recursion_protection_to_isolated_process() {
        let (_directory, executable) = fixture(
            r#"
test "$1" = exec
shift
for expected in --ignore-user-config --ephemeral --skip-git-repo-check --sandbox read-only --disable hooks --disable plugins --disable shell_tool --disable unified_exec --disable multi_agent --disable skill_search --enable skip_host_skill_discovery; do
    test "$1" = "$expected"
    shift
done
test "$1" = --output-schema
test "$(cat "$2")" = '{"type":"object"}'
shift 2
test "$1" = -o
output=$2
shift 2
test "$1" = --model
test "$2" = 'model with spaces'
test "$3" = -
test "$#" = 3
test "$(cat)" = 'prompt with $(literal) and spaces'
test ! -e AGENTS.md
test -f schema.json
printf '%s' '{"summary":"retained"}' > "$output"
"#,
        );
        let result = generate(
            "prompt with $(literal) and spaces",
            &serde_json::json!({"type": "object"}),
            &executable,
            Some("model with spaces"),
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(result, r#"{"summary":"retained"}"#);
    }

    #[test]
    fn reports_failure_with_bounded_stderr() {
        let (_directory, executable) =
            fixture("printf 'diagnostic marker' >&2\nhead -c 20000 /dev/zero >&2\nexit 17");
        let error = generate(
            "",
            &serde_json::json!({}),
            &executable,
            None,
            Duration::from_secs(5),
        )
        .unwrap_err();
        assert!(error.contains("17"));
        assert!(error.contains("diagnostic marker"));
        assert!(error.len() < 8500);
    }

    #[test]
    fn kills_and_reaps_worker_on_timeout() {
        let (_directory, executable) = fixture("exec sleep 10");
        let started = Instant::now();
        let error = generate(
            "",
            &serde_json::json!({}),
            &executable,
            None,
            Duration::from_millis(50),
        )
        .unwrap_err();
        assert!(error.contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn rejects_success_without_output() {
        let (_directory, executable) = fixture("exit 0");
        let error = generate(
            "",
            &serde_json::json!({}),
            &executable,
            None,
            Duration::from_secs(5),
        )
        .unwrap_err();
        assert!(error.contains("cannot read"));
    }
}
