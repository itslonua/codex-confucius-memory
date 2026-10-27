//! Codex rollout JSONL adapter (not a stable interface).
use crate::memory::Message;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};
const MAX_RECORD: usize = 8 * 1024 * 1024;
const MAX_BATCH: usize = 64 * 1024 * 1024;
const MAX_MESSAGES: usize = 4096;
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Cursor {
    path: String,
    offset: usize,
    prefix_hash: String,
    #[serde(default)]
    complete: bool,
}
impl Cursor {
    /// False when a batch limit or an unfinished trailing record stopped reading.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn mark_incomplete(&mut self) {
        self.complete = false;
    }
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn read_new(path: &Path, cursor: &Cursor) -> Result<(Vec<(Message, Cursor)>, Cursor), String> {
    read_batch(path, cursor, MAX_RECORD, MAX_BATCH)
}

fn read_batch(
    path: &Path,
    cursor: &Cursor,
    max_record: usize,
    max_batch: usize,
) -> Result<(Vec<(Message, Cursor)>, Cursor), String> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("transcript path: {e}"))?;
    let name = path
        .to_str()
        .ok_or("transcript path is not UTF-8")?
        .to_owned();
    if !cursor.path.is_empty() && cursor.path != name {
        return Err("transcript changed before saved cursor; refusing to mix history".into());
    }
    let mut reader = BufReader::new(File::open(&path).map_err(|e| e.to_string())?);
    let mut hasher = Sha256::new();
    let mut remaining = cursor.offset;
    let mut buffer = [0u8; 64 * 1024];
    let mut last_byte = None;
    while remaining > 0 {
        let requested = remaining.min(buffer.len());
        let count = reader
            .read(&mut buffer[..requested])
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("transcript changed before saved cursor; refusing to mix history".into());
        }
        hasher.update(&buffer[..count]);
        last_byte = Some(buffer[count - 1]);
        remaining -= count;
    }
    if cursor.offset > 0
        && (format!("{:x}", hasher.clone().finalize()) != cursor.prefix_hash
            || last_byte != Some(b'\n'))
    {
        return Err("transcript changed before saved cursor; refusing to mix history".into());
    }
    let mut end = cursor.offset;
    let mut batch_bytes = 0;
    let mut messages = Vec::new();
    let complete;
    loop {
        // Take bounds allocation even when a corrupted record has no newline.
        let mut line = Vec::new();
        let count = reader
            .by_ref()
            .take(max_record as u64 + 1)
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if count == 0 {
            complete = true;
            break;
        }
        if count > max_record {
            return Err(format!(
                "transcript record exceeds {max_record} bytes; history unchanged"
            ));
        }
        if line.last() != Some(&b'\n') {
            complete = false;
            break;
        }
        if batch_bytes + count > max_batch || messages.len() >= MAX_MESSAGES {
            complete = false;
            break;
        }
        hasher.update(&line);
        end = end.checked_add(count).ok_or("transcript offset overflow")?;
        batch_bytes += count;
        if line == b"\n" {
            continue;
        }
        let event: Value =
            serde_json::from_slice(&line).map_err(|e| format!("invalid transcript JSON: {e}"))?;
        if event.get("type").and_then(Value::as_str) != Some("response_item") {
            continue;
        }
        let payload = event
            .get("payload")
            .ok_or("response_item missing payload")?;
        let kind = payload
            .get("type")
            .and_then(Value::as_str)
            .ok_or("response_item missing type")?;
        let record_cursor = || Cursor {
            path: name.clone(),
            offset: end,
            prefix_hash: format!("{:x}", hasher.clone().finalize()),
            complete: false,
        };
        match kind {
            "message" => {
                let role = payload
                    .get("role")
                    .and_then(Value::as_str)
                    .ok_or("message missing role")?;
                // Never retain policy messages or hidden reasoning in project memory.
                if role != "user" && role != "assistant" {
                    continue;
                }
                let content = payload
                    .get("content")
                    .and_then(Value::as_array)
                    .ok_or("message missing content array")?;
                let mut texts = Vec::new();
                for block in content {
                    match block.get("type").and_then(Value::as_str) {
                        Some("input_text" | "output_text") => texts.push(
                            block
                                .get("text")
                                .and_then(Value::as_str)
                                .ok_or("text block missing text")?
                                .to_owned(),
                        ),
                        Some(_) => texts
                            .push("[non-text content omitted; consult original transcript]".into()),
                        None => return Err("content block missing type".into()),
                    }
                }
                let text = texts.join("\n");
                if !text.is_empty() {
                    messages.push((
                        Message {
                            role: role.into(),
                            text,
                            boundary: true,
                        },
                        record_cursor(),
                    ));
                }
            }
            "function_call" | "custom_tool_call" => {
                let name = payload
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or("tool call missing name")?;
                let args = payload
                    .get("arguments")
                    .or_else(|| payload.get("input"))
                    .ok_or("tool call missing arguments/input")?;
                messages.push((
                    Message {
                        role: "assistant".into(),
                        text: format!("tool {name}: {args}"),
                        boundary: false,
                    },
                    record_cursor(),
                ));
            }
            "function_call_output" | "custom_tool_call_output" => {
                let output = payload.get("output").ok_or("tool result missing output")?;
                let id = payload
                    .get("call_id")
                    .and_then(Value::as_str)
                    .ok_or("tool result missing call_id")?;
                messages.push((
                    Message {
                        role: "tool".into(),
                        text: format!("result {id}: {output}"),
                        boundary: false,
                    },
                    record_cursor(),
                ));
            }
            _ => {}
        }
    }
    Ok((
        messages,
        Cursor {
            path: name,
            offset: end,
            prefix_hash: format!("{:x}", hasher.finalize()),
            complete,
        },
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    #[test]
    fn reads_only_new_complete_records_and_avoids_event_duplicates() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file,"{}",json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"original"}]}})).unwrap();
        writeln!(
            file,
            "{}",
            json!({"type":"event_msg","payload":{"type":"user_message","message":"original"}})
        )
        .unwrap();
        write!(file, "{{\"type\":").unwrap();
        let (messages, cursor) = read_new(file.path(), &Cursor::default()).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].0.text, "original");
        assert!(read_new(file.path(), &cursor).unwrap().0.is_empty());
        writeln!(file,"\"response_item\",\"payload\":{{\"type\":\"function_call_output\",\"output\":\"passed\",\"call_id\":\"c1\"}}}} ").unwrap();
        let (messages, _) = read_new(file.path(), &cursor).unwrap();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].0.text.contains("passed"));
        assert!(!messages[0].0.boundary);
    }

    #[test]
    fn rewritten_transcript_is_not_silently_skipped() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), "{\"type\":\"event_msg\"}\n").unwrap();
        let (_, cursor) = read_new(file.path(), &Cursor::default()).unwrap();
        std::fs::write(file.path(), "{\"type\":\"something\"}\n").unwrap();
        assert!(read_new(file.path(), &cursor).is_err());
    }
    #[test]
    fn bounded_batches_resume_without_skipping_or_duplicating_records() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let record = |text: &str| {
            format!(
                "{}\n",
                json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":text}]}})
            )
        };
        let first = record("first");
        let second = record("second");
        write!(file, "{first}{second}").unwrap();
        let (messages, cursor) =
            read_batch(file.path(), &Cursor::default(), 1024, first.len()).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].0.text, "first");
        assert!(!cursor.is_complete());
        assert_eq!(cursor.offset, first.len());
        assert_eq!(cursor.prefix_hash, digest(first.as_bytes()));
        let (messages, cursor) = read_batch(file.path(), &cursor, 1024, second.len()).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].0.text, "second");
        assert!(cursor.is_complete());
        assert!(read_new(file.path(), &cursor).unwrap().0.is_empty());
    }

    #[test]
    fn partial_tail_preserves_cursor_until_completed() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        write!(file, "{{\"type\":\"event_msg\"}}\n{{\"type\":").unwrap();
        let (_, cursor) = read_new(file.path(), &Cursor::default()).unwrap();
        assert!(!cursor.is_complete());
        let (_, unchanged) = read_new(file.path(), &cursor).unwrap();
        assert_eq!(cursor.offset, unchanged.offset);
        assert_eq!(cursor.prefix_hash, unchanged.prefix_hash);
        writeln!(file, "\"event_msg\"}}").unwrap();
        let (_, completed) = read_new(file.path(), &cursor).unwrap();
        assert!(completed.is_complete());
        assert!(completed.offset > cursor.offset);
    }

    #[test]
    fn oversized_record_is_rejected_without_silent_truncation() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&[b'x'; 65]).unwrap();
        let error = read_batch(file.path(), &Cursor::default(), 64, 1024)
            .err()
            .unwrap();
        assert!(error.contains("record exceeds 64 bytes"));
    }

    #[test]
    fn lifetime_transcript_can_exceed_batch_limit() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let padding = "x".repeat(1024 * 1024);
        let record = format!("{}\n", json!({"type":"event_msg","padding":padding}));
        for _ in 0..65 {
            file.write_all(record.as_bytes()).unwrap();
        }
        let (_, cursor) = read_new(file.path(), &Cursor::default()).unwrap();
        assert!(!cursor.is_complete());
        assert!(cursor.offset <= MAX_BATCH);
        let (_, cursor) = read_new(file.path(), &cursor).unwrap();
        assert!(cursor.is_complete());
        assert_eq!(cursor.offset, record.len() * 65);
        assert!(cursor.offset > MAX_BATCH);
    }
    #[test]
    fn intermediate_cursor_resumes_across_ignored_records_and_partial_tail() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let record = |text: &str| {
            format!(
                "{}\n",
                json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":text}]}})
            )
        };
        let first = record("first");
        let second = record("second");
        let ignored = "{\"type\":\"event_msg\"}\n";
        write!(file, "{first}{ignored}{second}{ignored}{{\"type\":").unwrap();
        let (messages, final_cursor) = read_new(file.path(), &Cursor::default()).unwrap();
        let (_, first_cursor) = &messages[0];
        assert_eq!(first_cursor.offset, first.len());
        assert_eq!(first_cursor.prefix_hash, digest(first.as_bytes()));
        assert!(!first_cursor.is_complete());
        let (resumed, resumed_final) = read_new(file.path(), first_cursor).unwrap();
        assert_eq!(resumed.len(), 1);
        assert_eq!(resumed[0].0.text, "second");
        assert_eq!(
            resumed[0].1.offset,
            first.len() + ignored.len() + second.len()
        );
        assert_eq!(
            resumed[0].1.prefix_hash,
            digest(format!("{first}{ignored}{second}").as_bytes())
        );
        assert!(!resumed[0].1.is_complete());
        assert_eq!(resumed_final.offset, final_cursor.offset);
        assert_eq!(resumed_final.prefix_hash, final_cursor.prefix_hash);
        assert!(!resumed_final.is_complete());
        assert!(read_new(file.path(), &resumed[0].1).unwrap().0.is_empty());
        writeln!(file, "\"event_msg\"}}").unwrap();
        let (_, completed) = read_new(file.path(), &resumed[0].1).unwrap();
        assert!(completed.is_complete());
        assert!(completed.offset > final_cursor.offset);
        // Same-size rewrite in an ignored record must invalidate its successor's cursor.
        let changed = format!(
            "{first}{}{second}",
            ignored.replace("event_msg", "other_msg")
        );
        std::fs::write(file.path(), changed).unwrap();
        assert!(read_new(file.path(), &resumed[0].1).is_err());
    }
}
