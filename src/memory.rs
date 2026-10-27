// Adapted from Meta's LLMPlannerExtension (MIT). See THIRD_PARTY_NOTICES.md.
use crate::hierarchical_memory::{MemoryPath, create_content_with_frontmatter, edit_content};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

// Rollout records have no stable per-message token counts. This is an estimate,
// not the model tokenizer; leave headroom when selecting thresholds.
pub fn estimated_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "String")]
pub struct Text(String);
impl TryFrom<String> for Text {
    type Error = String;
    fn try_from(value: String) -> Result<Self, String> {
        if value.trim().is_empty() || value.len() > 6000 {
            return Err("memory text must contain 1..6000 UTF-8 bytes".into());
        }
        Ok(Self(value))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    conversation_context: Text,
    technical_decisions: Text,
    implementation_progress: Text,
    technical_details: Text,
    outstanding_items: Text,
    next_step: Text,
}

impl Summary {
    /// Markdown documents carry the existing six fields verbatim. Fences are
    /// longer than every backtick run in a value, so embedded Markdown is data.
    pub fn to_markdown(&self) -> String {
        let fields = [
            ("Conversation context", &self.conversation_context),
            ("Technical decisions", &self.technical_decisions),
            ("Implementation progress", &self.implementation_progress),
            ("Technical details", &self.technical_details),
            ("Outstanding items", &self.outstanding_items),
            ("Next step", &self.next_step),
        ];
        let mut document = String::from("# Summary\n\n");
        for (heading, value) in fields {
            let longest_run = value.0.split(|c| c != '`').map(str::len).max().unwrap_or(0);
            let fence = "`".repeat(3.max(longest_run + 1));
            document.push_str(&format!(
                "## {heading}\n\n{fence}\n{}\n{fence}\n\n",
                value.0
            ));
        }
        document
    }

    /// Parse only our storage envelope; never guess field boundaries from
    /// arbitrary headings within model-produced text.
    pub fn from_markdown(document: &str) -> Result<Self, String> {
        let mut remaining = document
            .strip_prefix("# Summary\n\n")
            .ok_or("invalid summary Markdown header")?;
        let fields = [
            ("Conversation context", "conversation_context"),
            ("Technical decisions", "technical_decisions"),
            ("Implementation progress", "implementation_progress"),
            ("Technical details", "technical_details"),
            ("Outstanding items", "outstanding_items"),
            ("Next step", "next_step"),
        ];
        let mut values = serde_json::Map::new();
        for (heading, field) in fields {
            remaining = remaining
                .strip_prefix(&format!("## {heading}\n\n"))
                .ok_or_else(|| format!("missing summary section: {heading}"))?;
            let (fence, content) = remaining.split_once('\n').ok_or("missing summary fence")?;
            if fence.len() < 3 || !fence.bytes().all(|byte| byte == b'`') {
                return Err("invalid summary fence".into());
            }
            let (value, rest) = content
                .split_once(&format!("\n{fence}\n\n"))
                .ok_or("unclosed summary fence")?;
            let parsed = Text::try_from(value.to_owned())?;
            values.insert(field.to_owned(), json!(parsed));
            remaining = rest;
        }
        if !remaining.is_empty() {
            return Err("unexpected content after summary".into());
        }
        let summary: Self =
            serde_json::from_value(Value::Object(values)).map_err(|error| error.to_string())?;
        if serde_json::to_vec(&summary)
            .map_err(|error| error.to_string())?
            .len()
            > 12000
        {
            return Err("summary exceeds 12000-byte memory budget".into());
        }
        Ok(summary)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "String")]
pub struct NoteId(String);
impl NoteId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for NoteId {
    type Error = String;
    fn try_from(value: String) -> Result<Self, String> {
        if value.is_empty()
            || value.len() > 120
            || value.split('/').any(|s| {
                s.is_empty()
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            })
        {
            return Err("note id must be a slash-separated slug".into());
        }
        Ok(Self(value))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "RawNote")]
pub struct Note {
    pub id: NoteId,
    pub title: Text,
    body: Text,
    evidence: Vec<Text>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNote {
    id: NoteId,
    title: Text,
    body: Text,
    evidence: Vec<Text>,
}
impl TryFrom<RawNote> for Note {
    type Error = String;
    fn try_from(raw: RawNote) -> Result<Self, String> {
        if raw.evidence.is_empty() || raw.evidence.len() > 8 {
            return Err("notes require 1..8 evidence references".into());
        }
        Ok(Self {
            id: raw.id,
            title: raw.title,
            body: raw.body,
            evidence: raw.evidence,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    summary: Summary,
    operations: Vec<Operation>,
}

#[derive(Deserialize)]
#[serde(try_from = "RawOperation")]
enum Operation {
    Write {
        path: MemoryPath,
        content: String,
        tags: Vec<String>,
    },
    Edit {
        path: MemoryPath,
        old_str: String,
        new_str: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "lowercase", deny_unknown_fields)]
enum RawOperation {
    Write {
        path: String,
        content: String,
        tags: Vec<String>,
    },
    Edit {
        path: String,
        old_str: String,
        new_str: String,
    },
}

impl TryFrom<RawOperation> for Operation {
    type Error = String;
    fn try_from(raw: RawOperation) -> Result<Self, String> {
        match raw {
            RawOperation::Write {
                path,
                content,
                tags,
            } => {
                if content.len() > 6000 {
                    return Err("note content exceeds 6000 UTF-8 bytes".into());
                }
                Ok(Self::Write {
                    path: MemoryPath::try_from(path)?,
                    content,
                    tags,
                })
            }
            RawOperation::Edit {
                path,
                old_str,
                new_str,
            } => {
                if old_str.is_empty() || old_str.len() > 6000 || new_str.len() > 6000 {
                    return Err(
                        "edit requires nonempty old_str and at most 6000 bytes per text".into(),
                    );
                }
                Ok(Self::Edit {
                    path: MemoryPath::try_from(path)?,
                    old_str,
                    new_str,
                })
            }
        }
    }
}

impl Operation {
    fn path(&self) -> &str {
        match self {
            Self::Write { path, .. } | Self::Edit { path, .. } => path.as_str(),
        }
    }
}

pub fn output_schema() -> Value {
    let text = json!({"type":"string"});
    let fields = [
        "conversation_context",
        "technical_decisions",
        "implementation_progress",
        "technical_details",
        "outstanding_items",
        "next_step",
    ];
    let props: serde_json::Map<String, Value> = fields
        .iter()
        .map(|k| (k.to_string(), text.clone()))
        .collect();
    json!({"type":"object","additionalProperties":false,"required":["summary","operations"],"properties":{
        "summary":{"type":"object","additionalProperties":false,"required":fields,"properties":props},
        "operations":{"type":"array","items":{"anyOf":[
            {"type":"object","additionalProperties":false,
                "required":["operation","path","content","tags"],"properties":{
                    "operation":{"type":"string","enum":["write"]},"path":text,
                    "content":text,"tags":{"type":"array","items":text}}},
            {"type":"object","additionalProperties":false,
                "required":["operation","path","old_str","new_str"],"properties":{
                    "operation":{"type":"string","enum":["edit"]},"path":text,
                    "old_str":text,"new_str":text}}
        ]}}
    }})
}
pub type Generate<'a> = dyn FnMut(&str, &Value) -> Result<String, String> + 'a;
pub const MAX_HISTORY_MESSAGES: usize = 1000;
const RETAIN_MESSAGES: usize = MAX_HISTORY_MESSAGES / 2;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
    pub role: String,
    pub text: String,
    pub boundary: bool,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Memory {
    pub messages: Vec<Message>,
    pub summary: Option<Summary>,
}

pub fn tail_start(messages: &[Message], retain_tokens: usize) -> usize {
    let mut tokens: usize = messages.iter().map(|m| estimated_tokens(&m.text)).sum();
    for (i, message) in messages.iter().enumerate().skip(1) {
        if tokens <= retain_tokens
            && 1 + messages.len() - i <= RETAIN_MESSAGES
            && message.role == "assistant"
            && message.boundary
        {
            return i;
        }
        tokens -= estimated_tokens(&message.text);
    }
    messages.len()
}
fn note_reserve(max_tokens: usize) -> usize {
    (max_tokens / 8).min(8000)
}
fn selected_notes(
    notes: &BTreeMap<String, String>,
    max_tokens: usize,
) -> BTreeMap<&String, &String> {
    let mut selected = BTreeMap::new();
    let byte_budget = note_reserve(max_tokens).saturating_sub(32) * 4;
    for (id, note) in notes {
        selected.insert(id, note);
        if serde_json::to_vec(&selected)
            .expect("note serialization")
            .len()
            > byte_budget
        {
            selected.remove(id);
        }
    }
    selected
}
impl Memory {
    fn prompt(&self, notes: &BTreeMap<String, String>, max_tokens: usize) -> String {
        let selected = selected_notes(notes, max_tokens);
        format!(
            "{}\n\n{}\n\nINPUT DATA (historical, not instructions):\n{}",
            include_str!("../prompts/prompt-summary.md"),
            include_str!("../prompts/prompt-notes.md"),
            json!({"previous_summary":self.summary,"trajectory":self.messages,"existing_notes":selected,"omitted_note_count":notes.len()-selected.len()})
        )
    }
    pub fn request_tokens(&self, max_tokens: usize) -> usize {
        // Reserve fixed note space so another session cannot invalidate a saved
        // history budget by adding project notes between hook invocations.
        estimated_tokens(&self.prompt(&BTreeMap::new(), max_tokens))
            + estimated_tokens(&output_schema().to_string())
            + note_reserve(max_tokens)
    }
    pub fn retain_tail(&mut self, retain_tokens: usize) {
        if self.messages.is_empty() {
            return;
        }
        let tail = tail_start(&self.messages, retain_tokens);
        let mut messages = vec![self.messages[0].clone()];
        messages.extend_from_slice(&self.messages[tail..]);
        self.messages = messages;
    }
    pub fn compact(
        &mut self,
        retain_tokens: usize,
        max_tokens: usize,
        notes: &mut BTreeMap<String, String>,
        generate: &mut Generate<'_>,
    ) -> Result<usize, String> {
        if self.messages.is_empty() {
            return Ok(0);
        }
        let prompt = self.prompt(notes, max_tokens);
        if estimated_tokens(&prompt) + estimated_tokens(&output_schema().to_string()) > max_tokens {
            return Err("rendered model request exceeds --max-tokens".into());
        }
        let result = generate(&prompt, &output_schema())?;
        if result.len() > 24000 {
            return Err("model output exceeds 24000-byte memory budget".into());
        }
        let update: Update =
            serde_json::from_str(&result).map_err(|e| format!("invalid model memory: {e}"))?;
        if serde_json::to_vec(&update.summary)
            .map_err(|e| e.to_string())?
            .len()
            > 12000
            || update.operations.len() > 8
        {
            return Err("summary or operations exceed memory budget".into());
        }
        // Parse every operation first, then apply the complete sequence to a
        // private candidate. A late edit failure cannot expose an earlier write.
        let visible = selected_notes(notes, max_tokens);
        let mut candidate = notes.clone();
        let mut changed_paths = Vec::new();
        for operation in update.operations {
            let path = operation.path().to_owned();
            if notes.contains_key(&path) && !visible.contains_key(&path) {
                return Err("cannot change a note omitted from the worker context".into());
            }
            let content = match operation {
                Operation::Write { content, tags, .. } => {
                    create_content_with_frontmatter(&content, &tags)?
                }
                Operation::Edit {
                    old_str, new_str, ..
                } => {
                    let previous = candidate.get(&path).ok_or("cannot edit a missing note")?;
                    edit_content(previous, &old_str, &new_str)?
                }
            };
            candidate.insert(path.clone(), content);
            if !changed_paths.contains(&path) {
                changed_paths.push(path);
            }
        }
        // Parse the final document set with the same domain boundary as lookup.
        // Intermediate operations may repair metadata; the published tree must
        // have valid tags and no file/directory collisions.
        crate::hierarchical_memory::tree_from_documents(
            std::path::Path::new("/memory"),
            &candidate,
        )?;
        // Capacity admission happens after evaluation, so a write followed by an
        // edit still works when a different new document is skipped at capacity.
        let mut updated_notes = notes.clone();
        let mut skipped = 0;
        for path in changed_paths {
            let content = candidate.remove(&path).expect("evaluated operation path");
            let previous = updated_notes.insert(path.clone(), content);
            if updated_notes.len() > 128
                || serde_json::to_vec(&updated_notes)
                    .map_err(|e| e.to_string())?
                    .len()
                    > 32000
            {
                updated_notes.remove(&path);
                if let Some(previous) = previous {
                    updated_notes.insert(path, previous);
                }
                skipped += 1;
            }
        }
        // Saturated project notes must not prevent task-memory progress.
        self.retain_tail(retain_tokens);
        self.summary = Some(update.summary);
        *notes = updated_notes;
        Ok(skipped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn message(role: &str, boundary: bool) -> Message {
        Message {
            role: role.into(),
            text: "x".repeat(40),
            boundary,
        }
    }
    #[test]
    fn markdown_operations_write_then_edit_preserve_source_metadata() {
        let mut memory = Memory {
            messages: vec![message("user", true)],
            summary: None,
        };
        let mut notes = BTreeMap::new();
        let mut update = valid_update();
        let raw = "---\nid: discovery\ntitle: Discovery\ndescription: Costly debugging\nkeywords: [parser]\n---\nOld result\n";
        update["operations"] = json!([
            {"operation":"write", "path":"projects/parser/discovery.md", "content":raw, "tags":["rust"]},
            {"operation":"edit", "path":"projects/parser/discovery.md", "old_str":"Old result", "new_str":"Corrected result"}
        ]);
        assert_eq!(
            memory
                .compact(10, 100000, &mut notes, &mut |_, _| Ok(update.to_string()))
                .unwrap(),
            0
        );
        let document = serde_json::to_value(&notes).unwrap()["projects/parser/discovery.md"]
            .as_str()
            .unwrap()
            .to_owned();
        let (tags, content) = crate::hierarchical_memory::parse_frontmatter(&document).unwrap();
        assert_eq!(tags, vec!["rust"]);
        assert_eq!(content, raw.replace("Old result", "Corrected result"));
    }

    #[test]
    fn invalid_final_tags_or_path_collision_preserve_previous_memory() {
        let mut memory = Memory {
            messages: vec![message("user", true)],
            summary: None,
        };
        let mut notes = BTreeMap::new();
        let before = serde_json::to_value((&memory, &notes)).unwrap();
        for operations in [
            json!([{"operation":"write", "path":"shared/note.md", "content":"---\ntags: [12]\n---\nBody\n", "tags":[]}]),
            json!([
                {"operation":"write", "path":"shared/note.md", "content":"Body\n", "tags":[]},
                {"operation":"write", "path":"shared/note.md/child.md", "content":"Child\n", "tags":[]}
            ]),
        ] {
            let mut update = valid_update();
            update["operations"] = operations;
            assert!(
                memory
                    .compact(10, 100000, &mut notes, &mut |_, _| Ok(update.to_string()))
                    .is_err()
            );
            assert_eq!(serde_json::to_value((&memory, &notes)).unwrap(), before);
        }
    }

    #[test]
    fn final_tree_validation_allows_intermediate_metadata_repair() {
        let mut memory = Memory {
            messages: vec![message("user", true)],
            summary: None,
        };
        let mut notes = BTreeMap::new();
        let mut update = valid_update();
        update["operations"] = json!([
            {"operation":"write", "path":"shared/note.md", "content":"---\ntags: [12]\n---\nBody\n", "tags":[]},
            {"operation":"edit", "path":"shared/note.md", "old_str":"tags: [12]", "new_str":"tags: [rust]"}
        ]);
        memory
            .compact(10, 100000, &mut notes, &mut |_, _| Ok(update.to_string()))
            .unwrap();
        assert_eq!(
            crate::hierarchical_memory::parse_frontmatter(&notes["shared/note.md"])
                .unwrap()
                .0,
            vec!["rust"]
        );
    }

    #[test]
    fn late_edit_failure_rolls_back_prior_write_summary_and_history() {
        let mut memory = Memory {
            messages: vec![message("user", true), message("assistant", true)],
            summary: Some(serde_json::from_value(valid_update()["summary"].clone()).unwrap()),
        };
        let mut notes = BTreeMap::new();
        let before = serde_json::to_value((&memory, &notes)).unwrap();
        let mut update = valid_update();
        update["operations"] = json!([
            {"operation":"write", "path":"shared/new_note.md", "content":"real content\n", "tags":[]},
            {"operation":"edit", "path":"shared/new_note.md", "old_str":"missing text", "new_str":"new"}
        ]);
        assert!(
            memory
                .compact(10, 100000, &mut notes, &mut |_, _| Ok(update.to_string()))
                .is_err()
        );
        assert_eq!(serde_json::to_value((&memory, &notes)).unwrap(), before);
    }

    #[test]
    fn operation_parser_rejects_old_schema_unknown_fields_and_missing_edit_text() {
        let mut old = valid_update();
        old.as_object_mut().unwrap().remove("operations");
        old["notes"] = json!([]);
        assert!(serde_json::from_value::<Update>(old).is_err());
        for operation in [
            json!({"operation":"delete", "path":"shared/note.md"}),
            json!({"operation":"write", "path":"../escape.md", "content":"text", "tags":[]}),
            json!({"operation":"write", "path":"shared/note.md", "content":"text", "tags":[], "evidence":[]}),
            json!({"operation":"edit", "path":"shared/note.md", "old_str":"", "new_str":"text"}),
            json!({"operation":"edit", "path":"shared/note.md", "old_str":"old"}),
            json!({"operation":"write", "path":"shared/note.md", "content":"x".repeat(6001), "tags":[]}),
        ] {
            assert!(serde_json::from_value::<Operation>(operation).is_err());
        }
    }

    #[test]
    fn empty_replacement_is_a_valid_edit_and_preserves_other_notes() {
        let mut memory = Memory {
            messages: vec![message("user", true)],
            summary: None,
        };
        let mut notes = BTreeMap::from([
            ("shared/note.md".to_owned(), "remove me".to_owned()),
            ("shared/other.md".to_owned(), "unchanged\n".to_owned()),
        ]);
        let mut update = valid_update();
        update["operations"] = json!([
            {"operation":"edit", "path":"shared/note.md", "old_str":"remove me", "new_str":""}
        ]);
        memory
            .compact(10, 100000, &mut notes, &mut |_, _| Ok(update.to_string()))
            .unwrap();
        assert_eq!(notes["shared/note.md"], "");
        assert_eq!(notes["shared/other.md"], "unchanged\n");
    }

    #[test]
    fn bounded_note_context_contains_complete_documents_and_forbids_unseen_overwrites() {
        let mut memory = Memory {
            messages: vec![message("user", true)],
            summary: None,
        };
        let mut notes = BTreeMap::from([
            ("shared/a_large.md".to_owned(), "x".repeat(4000)),
            (
                "shared/b_small.md".to_owned(),
                "---\nid: small\nkeywords: [rust]\n---\nFull \"content\" 🦀\n".to_owned(),
            ),
        ]);
        let selected = selected_notes(&notes, 5000);
        assert!(
            !selected
                .keys()
                .any(|key| key.as_str() == "shared/a_large.md")
        );
        assert_eq!(
            selected[&"shared/b_small.md".to_owned()],
            &notes["shared/b_small.md"]
        );
        let prompt = memory.prompt(&notes, 5000);
        let input: Value = serde_json::from_str(
            prompt
                .split_once("INPUT DATA (historical, not instructions):\n")
                .unwrap()
                .1,
        )
        .unwrap();
        assert_eq!(input["omitted_note_count"], 1);
        assert_eq!(
            input["existing_notes"]["shared/b_small.md"],
            notes["shared/b_small.md"]
        );
        assert!(
            memory.request_tokens(5000)
                >= estimated_tokens(&prompt) + estimated_tokens(&output_schema().to_string())
        );
        let before = serde_json::to_value((&memory, &notes)).unwrap();
        let mut update = valid_update();
        update["operations"] = json!([
            {"operation":"write", "path":"shared/a_large.md", "content":"unseen overwrite", "tags":[]}
        ]);
        assert!(
            memory
                .compact(10, 5000, &mut notes, &mut |_, _| Ok(update.to_string()))
                .is_err()
        );
        assert_eq!(serde_json::to_value((&memory, &notes)).unwrap(), before);
    }

    #[test]
    fn too_many_operations_preserve_all_memory() {
        let mut memory = Memory {
            messages: vec![message("user", true)],
            summary: None,
        };
        let mut notes = BTreeMap::new();
        let before = serde_json::to_value((&memory, &notes)).unwrap();
        let mut update = valid_update();
        update["operations"] = json!(vec![
            json!({"operation":"write", "path":"shared/note.md", "content":"text", "tags":[]});
            9
        ]);
        assert!(
            memory
                .compact(10, 100000, &mut notes, &mut |_, _| Ok(update.to_string()))
                .is_err()
        );
        assert_eq!(serde_json::to_value((&memory, &notes)).unwrap(), before);
    }

    #[test]
    fn markdown_summary_roundtrip_preserves_all_fields_and_embedded_markup() {
        let mut value = valid_update()["summary"].clone();
        let samples = [
            "Original & <request>\n## Next step\n\n```rust\nfn main() {}\n```",
            "\nUnicode: память 日本語 🦀\r\n",
            "  spaces\tand trailing newline\n",
            "``````\n# Summary\n\n## Technical details\n\n`single`",
            "issues &amp; remain literal",
            "run exactly one immediate step\n\n",
        ];
        for ((_, field), sample) in value.as_object_mut().unwrap().iter_mut().zip(samples) {
            *field = json!(sample);
        }
        let summary: Summary = serde_json::from_value(value.clone()).unwrap();
        let document = summary.to_markdown();
        assert!(document.starts_with("# Summary\n"));
        assert_eq!(
            serde_json::to_value(Summary::from_markdown(&document).unwrap()).unwrap(),
            value
        );
    }

    #[test]
    fn markdown_summary_rejects_aggregate_over_budget_even_when_each_field_fits() {
        let mut value = valid_update()["summary"].clone();
        for field in value.as_object_mut().unwrap().values_mut() {
            *field = json!("🦀".repeat(700));
        }
        let summary: Summary = serde_json::from_value(value).unwrap();
        assert!(Summary::from_markdown(&summary.to_markdown()).is_err());
    }

    #[test]
    fn markdown_summary_rejects_missing_sections_truncation_and_extra_content() {
        let summary: Summary = serde_json::from_value(valid_update()["summary"].clone()).unwrap();
        let document = summary.to_markdown();
        for broken in [
            document.replace("## Technical details", "## Other"),
            document[..document.len() - 3].to_owned(),
            format!("{document}unexpected"),
            document.replace("```\nprogress", "```text\nprogress"),
        ] {
            assert!(Summary::from_markdown(&broken).is_err());
        }
    }

    #[test]
    fn planner_tail_matches_source_derived_cases() {
        #[derive(Deserialize)]
        struct Case {
            name: String,
            messages: Vec<Message>,
            retain_tokens: usize,
            expected_tail: usize,
        }
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../tests/fixtures/upstream/planner-tail.json"))
                .unwrap();
        for case in cases {
            let initial = case.messages[0].text.clone();
            let expected_tail = case.messages[case.expected_tail..].to_vec();
            assert_eq!(
                tail_start(&case.messages, case.retain_tokens),
                case.expected_tail,
                "{}",
                case.name
            );
            let mut memory = Memory {
                messages: case.messages,
                summary: None,
            };
            memory.retain_tail(case.retain_tokens);
            assert_eq!(memory.messages[0].text, initial, "{}", case.name);
            assert_eq!(
                serde_json::to_value(&memory.messages[1..]).unwrap(),
                serde_json::to_value(expected_tail).unwrap(),
                "{}",
                case.name
            );
        }
    }

    fn valid_update() -> Value {
        json!({"summary":{
            "conversation_context":"Original request & user correction <literal>",
            "technical_decisions":"decisions", "implementation_progress":"progress",
            "technical_details":"details", "outstanding_items":"issues",
            "next_step":"Run cargo test --locked memory"
        }, "operations":[]})
    }

    #[test]
    fn successful_compaction_replaces_summary_and_preserves_first_step_verbatim() {
        let mut memory = Memory {
            messages: vec![message("user", true), message("assistant", true)],
            summary: Some(serde_json::from_value(valid_update()["summary"].clone()).unwrap()),
        };
        let mut update = valid_update();
        update["summary"]["conversation_context"] = json!("Corrected request & <literal>");
        let mut notes = BTreeMap::new();
        memory
            .compact(10, 100000, &mut notes, &mut |prompt, _| {
                assert!(prompt.contains("previous_summary"));
                assert!(prompt.contains("Original request & user correction <literal>"));
                Ok(update.to_string())
            })
            .unwrap();
        assert_eq!(
            serde_json::to_value(&memory.summary).unwrap(),
            update["summary"]
        );
        assert_eq!(memory.messages.len(), 1);
        assert_eq!(memory.messages[0].role, "user");
    }

    #[test]
    fn failed_generation_or_invalid_update_preserves_existing_summary_tail_and_notes() {
        let mut memory = Memory {
            messages: vec![message("user", true), message("assistant", true)],
            summary: Some(serde_json::from_value(valid_update()["summary"].clone()).unwrap()),
        };
        let mut notes = BTreeMap::from([(
            "shared/existing.md".to_owned(),
            "Keep this discovery\n".to_owned(),
        )]);
        let before = serde_json::to_value((&memory, &notes)).unwrap();
        let mut invalid_note = valid_update();
        invalid_note["operations"] = json!([
            {"operation":"write", "path":"shared/valid.md", "content":"new\n", "tags":[]},
            {"operation":"write", "path":"../escape.md", "content":"bad\n", "tags":[]}
        ]);
        let mut missing_step = valid_update();
        missing_step["summary"]
            .as_object_mut()
            .unwrap()
            .remove("next_step");
        let mut oversized_summary = valid_update();
        for field in [
            "conversation_context",
            "technical_decisions",
            "technical_details",
        ] {
            oversized_summary["summary"][field] = json!("x".repeat(5000));
        }
        for response in [
            Err("worker timeout".to_owned()),
            Ok(invalid_note.to_string()),
            Ok(missing_step.to_string()),
            Ok(oversized_summary.to_string()),
            Ok("x".repeat(24001)),
        ] {
            let mut calls = 0;
            let result = memory.compact(10, 100000, &mut notes, &mut |_, _| {
                calls += 1;
                response.clone()
            });
            assert!(result.is_err());
            assert_eq!(calls, 1, "bounded Codex adapter must not repair forever");
            assert_eq!(serde_json::to_value((&memory, &notes)).unwrap(), before);
        }
    }

    #[test]
    fn oversized_request_does_not_invoke_worker_or_mutate_memory() {
        let mut memory = Memory {
            messages: vec![message("user", true)],
            summary: None,
        };
        let before = serde_json::to_value(&memory).unwrap();
        let mut notes = BTreeMap::new();
        assert!(
            memory
                .compact(10, 1, &mut notes, &mut |_, _| panic!("oversized request"))
                .is_err()
        );
        assert_eq!(serde_json::to_value(&memory).unwrap(), before);
    }

    #[test]
    fn retains_initial_request_and_cuts_at_assistant_boundary() {
        let messages = vec![
            message("user", true),
            message("assistant", true),
            message("assistant", false),
            message("tool", false),
            message("assistant", true),
        ];
        assert_eq!(tail_start(&messages, 25), 4);
    }
    #[test]
    fn never_keeps_an_orphan_tool_result_as_tail_start() {
        let messages = vec![
            message("user", true),
            message("assistant", false),
            message("tool", false),
        ];
        assert_eq!(tail_start(&messages, 15), messages.len());
    }
    #[test]
    fn short_history_is_reduced_by_count_at_an_assistant_boundary() {
        let mut memory = Memory {
            messages: vec![message("user", true)],
            summary: None,
        };
        for i in 1..1005 {
            let mut item = message(if i % 3 == 0 { "tool" } else { "assistant" }, i % 3 == 1);
            item.text = i.to_string();
            memory.messages.push(item);
        }
        let initial = memory.messages[0].text.clone();
        memory.retain_tail(50000);
        assert!(memory.messages.len() <= 500);
        assert_eq!(memory.messages[0].text, initial);
        assert_eq!(memory.messages[1].role, "assistant");
        assert!(memory.messages[1].boundary);
        assert_eq!(memory.messages.last().unwrap().text, "1004");
    }
    #[test]
    fn invalid_model_response_preserves_history_and_notes() {
        let mut memory = Memory {
            messages: vec![message("user", true), message("assistant", true)],
            summary: None,
        };
        let before = serde_json::to_string(&memory).unwrap();
        let mut notes = BTreeMap::new();
        assert!(
            memory
                .compact(10, 100000, &mut notes, &mut |_, _| Ok("{}".into()))
                .is_err()
        );
        assert_eq!(serde_json::to_string(&memory).unwrap(), before);
        assert!(notes.is_empty());
    }
    #[test]
    fn saturated_notes_do_not_block_valid_summary() {
        let mut memory = Memory {
            messages: vec![message("user", true)],
            summary: None,
        };
        let mut notes = BTreeMap::new();
        for i in 0..128 {
            notes.insert(format!("shared/note_{i}.md"), "body\n".to_owned());
        }
        let mut update = valid_update();
        update["operations"] = json!([
            {"operation":"write","path":"shared/new_note.md","content":"body\n","tags":[]},
            {"operation":"edit","path":"shared/new_note.md","old_str":"body","new_str":"final body"}
        ]);
        let update = update.to_string();
        let skipped = memory
            .compact(10, 100000, &mut notes, &mut |_, _| Ok(update.clone()))
            .unwrap();
        assert_eq!(skipped, 1);
        assert!(!notes.contains_key("shared/new_note.md"));
        assert!(memory.summary.is_some());
        assert_eq!(notes.len(), 128);
    }
}
