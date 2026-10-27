// Adapted from Meta's hierarchical memory (MIT). See LICENSE-META.
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MemoryNode {
    pub path: PathBuf,
    pub name: String,
    pub content: String,
    pub tags: Vec<String>,
    pub children: Vec<MemoryNode>,
}

/// Parse the upstream tags envelope, then enforce MemoryNode's list[str] type.
/// Malformed/non-tags YAML remains untouched, as in Confucius parse_frontmatter.
pub fn parse_frontmatter(content: &str) -> Result<(Vec<String>, String), String> {
    let unchanged = || Ok((Vec::new(), content.to_owned()));
    if !content.starts_with("---\n") {
        return unchanged();
    }
    let mut parts = content.splitn(3, "---\n");
    parts.next();
    let Some(frontmatter) = parts.next() else {
        return unchanged();
    };
    let Some(body) = parts.next() else {
        return unchanged();
    };
    let Ok(mut value) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(frontmatter) else {
        return unchanged();
    };
    if value.apply_merge().is_err() {
        return unchanged();
    }
    let Some(mapping) = value.as_mapping() else {
        return unchanged();
    };
    let Some(tags) = mapping.get(serde_yaml_ng::Value::String("tags".into())) else {
        return unchanged();
    };
    let Some(tags) = tags.as_sequence() else {
        return unchanged();
    };
    let tags = tags
        .iter()
        .map(|tag| match tag {
            serde_yaml_ng::Value::String(tag) => Ok(tag.clone()),
            _ => Err("memory tags must be strings".to_owned()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((tags, body.to_owned()))
}

/// Preserve caller content, including any existing note-taker metadata.
/// YAML emitter quoting may differ from PyYAML; the tags/content data is unchanged.
pub fn create_content_with_frontmatter(content: &str, tags: &[String]) -> Result<String, String> {
    if tags.is_empty() {
        return Ok(content.to_owned());
    }
    // A JSON flow sequence is valid YAML. Always quote string tags so YAML 1.1
    // consumers do not resolve yes/on/date-looking strings as bools or dates.
    // Escape YAML line separators which JSON may otherwise emit literally.
    let tags = serde_json::to_string(tags)
        .map_err(|error| error.to_string())?
        .replace('\u{85}', "\\u0085")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    Ok(format!("---\ntags: {tags}\n---\n{content}"))
}

/// A parsed relative Markdown file path; usable before any storage side effect.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct MemoryPath(String);

impl MemoryPath {
    pub fn file(path: &str) -> Result<Self, String> {
        if path.is_empty()
            || path.len() > 4096
            || path.split('/').count() > 64
            || !path.ends_with(".md")
            || path
                .chars()
                .any(|ch| ch.is_control() || ch == '\\' || ch == ':')
            || path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err("memory file must be a relative .md path without traversal".into());
        }
        Ok(Self(path.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for MemoryPath {
    type Error = String;
    fn try_from(path: String) -> Result<Self, String> {
        Self::file(&path)
    }
}

/// Build the upstream display/search tree from documents loaded by the secure
/// store boundary. This layer never reopens filesystem paths.
pub fn tree_from_documents(
    base: &Path,
    documents: &std::collections::BTreeMap<String, String>,
) -> Result<Vec<MemoryNode>, String> {
    if documents.len() > 4096
        || documents.values().map(String::len).sum::<usize>() > 32 * 1024 * 1024
    {
        return Err("memory tree exceeds limits".into());
    }
    let mut nodes = Vec::new();
    for (path, raw) in documents {
        let path = MemoryPath::file(path)?;
        if raw.len() > 1024 * 1024 || path.as_str().split('/').count() > 64 {
            return Err("memory document exceeds limits".into());
        }
        let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
        let (tags, content) = parse_frontmatter(&normalized)?;
        insert_node(
            &mut nodes,
            base,
            &path.as_str().split('/').collect::<Vec<_>>(),
            &content,
            &tags,
        )?;
    }
    fn collapse(nodes: Vec<MemoryNode>) -> Vec<MemoryNode> {
        nodes
            .into_iter()
            .map(|mut node| {
                node.children = collapse(node.children);
                merge_single_child_memory_dirs(node)
            })
            .collect()
    }
    Ok(collapse(nodes))
}

fn insert_node(
    nodes: &mut Vec<MemoryNode>,
    parent: &Path,
    parts: &[&str],
    content: &str,
    tags: &[String],
) -> Result<(), String> {
    let name = parts[0];
    let index = match nodes.binary_search_by(|node| node.name.as_str().cmp(name)) {
        Ok(index) => {
            if parts.len() == 1 || nodes[index].children.is_empty() {
                return Err("memory path is both a file and a directory".into());
            }
            index
        }
        Err(index) => {
            nodes.insert(
                index,
                MemoryNode {
                    path: parent.join(name),
                    name: name.into(),
                    content: String::new(),
                    tags: Vec::new(),
                    children: Vec::new(),
                },
            );
            index
        }
    };
    if parts.len() == 1 {
        nodes[index].content = content.to_owned();
        nodes[index].tags = tags.to_vec();
    } else {
        let path = nodes[index].path.clone();
        insert_node(
            &mut nodes[index].children,
            &path,
            &parts[1..],
            content,
            tags,
        )?;
    }
    Ok(())
}

#[derive(Debug)]
enum ContentPattern {
    Regex(fancy_regex::Regex),
    Literal(String),
}
impl ContentPattern {
    fn parse(pattern: &str) -> Result<Self, String> {
        use fancy_regex::{CompileError, Error, ParseError};
        // Python re rejects unknown ASCII-letter escapes. Some are extensions
        // accepted by the backend (e.g. \p{L}); preserve Python's literal fallback.
        let mut chars = pattern.chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                if let Some(escaped) = chars.next() {
                    if escaped.is_ascii_alphabetic() && !"aAbBdDsSwWZfnrtvuxUN".contains(escaped) {
                        return Ok(Self::Literal(pattern.to_lowercase()));
                    }
                }
            }
        }
        match fancy_regex::RegexBuilder::new(pattern)
            .case_insensitive(true)
            .backtrack_limit(100_000)
            .build()
        {
            Ok(regex) => Ok(Self::Regex(regex)),
            Err(Error::ParseError(
                _,
                ParseError::UnclosedOpenParen
                | ParseError::TrailingBackslash
                | ParseError::InvalidClass
                | ParseError::InvalidRepeat
                | ParseError::InvalidHex
                | ParseError::InvalidCodepointValue,
            ))
            | Err(Error::CompileError(
                CompileError::LookBehindNotConst | CompileError::InvalidBackref(_),
            )) => Ok(Self::Literal(pattern.to_lowercase())),
            Err(error) => Err(format!(
                "regex is unsupported or ambiguous under Python compatibility: {error}"
            )),
        }
    }
    fn matches(&self, content: &str) -> Result<bool, String> {
        match self {
            Self::Regex(regex) => regex.is_match(content).map_err(|error| error.to_string()),
            Self::Literal(pattern) => Ok(content.to_lowercase().contains(pattern)),
        }
    }
}

#[derive(Debug)]
pub struct SearchInput {
    path_pattern: Option<String>,
    content_pattern: Option<ContentPattern>,
    tags: Vec<String>,
    max_results: usize,
}
impl SearchInput {
    pub fn new(
        path_pattern: Option<String>,
        content_pattern: Option<String>,
        tags: Vec<String>,
        max_results: usize,
    ) -> Result<Self, String> {
        if path_pattern
            .iter()
            .chain(content_pattern.iter())
            .any(|p| p.len() > 512)
            || tags.iter().any(|t| t.len() > 512)
            || tags.len() > 128
        {
            return Err("search input exceeds limits".into());
        }
        Ok(Self {
            path_pattern,
            content_pattern: content_pattern
                .as_deref()
                .map(ContentPattern::parse)
                .transpose()?,
            tags,
            max_results,
        })
    }
}
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SearchResult {
    pub path: String,
    pub tags: Vec<String>,
    pub content_preview: String,
}
pub fn search_memory(
    base: &Path,
    documents: &std::collections::BTreeMap<String, String>,
    input: &SearchInput,
) -> Result<Vec<SearchResult>, String> {
    let tree = tree_from_documents(base, documents)?;
    let mut found = Vec::new();
    fn visit(
        nodes: &[MemoryNode],
        base: &Path,
        input: &SearchInput,
        found: &mut Vec<SearchResult>,
    ) -> Result<(), String> {
        for node in nodes {
            if found.len() >= input.max_results {
                break;
            }
            let path = node
                .path
                .strip_prefix(base)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("non-UTF8 path")?
                .to_owned();
            let matching_path = path.strip_suffix(".md").unwrap_or(&path);
            if input
                .path_pattern
                .as_deref()
                .is_none_or(|p| glob_matches(p, matching_path))
                && input.tags.iter().all(|t| node.tags.contains(t))
                && match &input.content_pattern {
                    None => true,
                    Some(pattern) => pattern.matches(&node.content)?,
                }
            {
                let mut preview: String = node.content.chars().take(100).collect();
                if node.content.chars().count() > 100 {
                    preview.push_str("...");
                }
                found.push(SearchResult {
                    path,
                    tags: node.tags.clone(),
                    content_preview: preview,
                });
            }
            visit(&node.children, base, input, found)?;
        }
        Ok(())
    }
    visit(&tree, base, input, &mut found)?;
    Ok(found)
}

// Python fnmatch treats slash as an ordinary character, unlike path globs.
// Dynamic programming avoids exponential wildcard backtracking.
fn glob_matches(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let mut row = vec![false; text.len() + 1];
    row[0] = true;
    let mut i = 0;
    while i < pattern.len() {
        let token = pattern[i];
        i += 1;
        if token == '*' {
            for j in 1..row.len() {
                row[j] |= row[j - 1];
            }
            continue;
        }
        let mut class = None;
        if token == '[' {
            let mut end = i;
            if pattern.get(end) == Some(&'!') {
                end += 1;
            }
            if pattern.get(end) == Some(&']') {
                end += 1;
            }
            while end < pattern.len() && pattern[end] != ']' {
                end += 1;
            }
            if end < pattern.len() {
                class = Some(&pattern[i..end]);
                i = end + 1;
            }
        }
        for j in (1..row.len()).rev() {
            let matched = if let Some(class) = class {
                let negative = class.first() == Some(&'!');
                let class = if negative { &class[1..] } else { class };
                let mut k = 0;
                let mut found = false;
                while k < class.len() {
                    if k + 2 < class.len() && class[k + 1] == '-' {
                        found |= class[k] <= text[j - 1] && text[j - 1] <= class[k + 2];
                        k += 3;
                    } else {
                        found |= class[k] == text[j - 1];
                        k += 1;
                    }
                }
                found != negative
            } else {
                token == '?' || token == text[j - 1]
            };
            row[j] = row[j - 1] && matched;
        }
        row[0] = false;
    }
    row[text.len()]
}

pub fn read_memory(
    base: &Path,
    documents: &std::collections::BTreeMap<String, String>,
    path: &str,
    start: Option<i64>,
    end: Option<i64>,
) -> Result<String, String> {
    if let Some(raw) = documents.get(path) {
        MemoryPath::file(path)?;
        return view_content(&raw.replace("\r\n", "\n").replace('\r', "\n"), start, end);
    }
    let path = if matches!(path, "" | "/" | ".") {
        ""
    } else {
        path.trim_end_matches('/')
    };
    // Parse directory paths with the same component rules as file paths.
    if !path.is_empty() {
        MemoryPath::file(&format!("{path}/__directory.md"))?;
    }
    let prefix = if path.is_empty() {
        String::new()
    } else {
        format!("{path}/")
    };
    let mut children = std::collections::BTreeSet::new();
    for name in documents.keys() {
        MemoryPath::file(name)?;
        if let Some(relative) = name.strip_prefix(&prefix) {
            let (first, directory) = relative
                .split_once('/')
                .map_or((relative, false), |(first, _)| (first, true));
            if first.starts_with('.') {
                continue;
            }
            let child = base.join(path).join(first);
            children.insert(format!(
                "{}{}",
                child.display(),
                if directory { "/" } else { "" }
            ));
        }
    }
    // An existing hidden-only directory still produces an empty listing.
    if !path.is_empty() && !documents.keys().any(|key| key.starts_with(&prefix)) {
        return Err(format!("Memory node '{path}' not found"));
    }
    Ok(children.into_iter().collect::<Vec<_>>().join("\n"))
}
fn view_content(content: &str, start: Option<i64>, end: Option<i64>) -> Result<String, String> {
    let lines: Vec<&str> = content.split('\n').collect();
    let start = start.unwrap_or(1);
    let end = match end {
        None | Some(-1) => lines.len() as i64,
        Some(end) => end,
    };
    if start < 1 || end < 1 {
        return Err("Line numbers must be positive".into());
    }
    if start > end {
        return Err("Start line must be less than or equal to end line".into());
    }
    let width = end.to_string().len();
    let mut result = Vec::new();
    if start > 1 {
        result.push(format!("({} line(s) above)", start - 1));
    }
    for (index, line) in lines.iter().enumerate() {
        let number = index as i64 + 1;
        if number >= start && number <= end {
            result.push(format!("{number:width$}|{line}"));
        }
    }
    if end < (lines.len() as i64) {
        result.push(format!("({} line(s) below)", lines.len() as i64 - end));
    }
    // Upstream textwrap.dedent removes common indentation from numbered lines.
    let margin = result
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches(' ').len())
        .min()
        .unwrap_or(0);
    Ok(result
        .iter()
        .map(|line| &line[margin..])
        .collect::<Vec<_>>()
        .join("\n"))
}

pub fn edit_content(content: &str, old_str: &str, new_str: &str) -> Result<String, String> {
    // Path.read_text() in the source translates universal newlines before edit.
    let normalized_content = content.replace("\r\n", "\n").replace('\r', "\n");
    let content = normalized_content.as_str();
    fn normalize(text: &str) -> String {
        text.split('\n')
            .map(|line| {
                if line.chars().all(char::is_whitespace) {
                    ""
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    let source = normalize(content);
    let find = normalize(old_str);
    if find.is_empty() {
        return Err("Empty find string provided.".into());
    }
    let matches: Vec<_> = source
        .char_indices()
        .filter_map(|(i, _)| source[i..].starts_with(&find).then_some(i))
        .collect();
    if matches.len() != 1 {
        return Err(format!(
            "Expected one exact occurrence, found {}",
            matches.len()
        ));
    }
    let index = matches[0];
    let end = index + find.len();
    let full_line = (index == 0 || source.as_bytes()[index - 1] == b'\n')
        && (end == source.len() || source.as_bytes()[end] == b'\n');
    if !full_line {
        return Ok(source.replace(&find, new_str));
    }
    let before = source[..index].bytes().filter(|b| *b == b'\n').count();
    let count = find.bytes().filter(|b| *b == b'\n').count() + 1;
    let original: Vec<_> = content.split('\n').collect();
    let mut result = original[..before].to_vec();
    result.extend(new_str.split('\n'));
    result.extend_from_slice(&original[before + count..]);
    Ok(result.join("\n"))
}

fn merge_single_child_memory_dirs(mut node: MemoryNode) -> MemoryNode {
    // Children have already been merged by recursive scanning.
    if node.children.len() == 1
        && !node.children[0].children.is_empty()
        && node.content.is_empty()
        && node.tags.is_empty()
    {
        let mut child = node.children.remove(0);
        child.name = format!("{}/{}", node.name, child.name);
        return child;
    }
    node
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn fixtures() -> Vec<Value> {
        serde_json::from_str::<Value>(include_str!("../tests/fixtures/upstream/hierarchical.json"))
            .unwrap()["cases"]
            .as_array()
            .unwrap()
            .clone()
    }

    #[test]
    fn hierarchical_frontmatter_matches_upstream_fixtures() {
        for case in fixtures()
            .into_iter()
            .filter(|case| case["function"] == "parse_frontmatter")
        {
            let content = case["input"]["content"].as_str().unwrap();
            if case["name"] == "nonstring_tag" {
                // The helper returns untyped tags; original MemoryNode rejects them.
                assert!(parse_frontmatter(content).is_err());
                continue;
            }
            let (tags, body) = parse_frontmatter(content).unwrap();
            assert_eq!(
                serde_json::json!([tags, body]),
                case["expected"]["value"],
                "{}",
                case["name"]
            );
        }
    }

    #[test]
    fn hierarchical_render_preserves_metadata_and_tags() {
        for case in fixtures()
            .into_iter()
            .filter(|case| case["function"] == "create_content_with_frontmatter")
        {
            let input = &case["input"];
            let tags: Vec<String> = serde_json::from_value(input["tags"].clone()).unwrap();
            let content = input["content"].as_str().unwrap();
            let rendered = create_content_with_frontmatter(content, &tags).unwrap();
            // YAML emitter quote/Unicode styles may differ; semantic data must not.
            assert_eq!(
                parse_frontmatter(&rendered).unwrap(),
                (tags, content.to_owned())
            );
        }
        let metadata = "---\nid: topic\ntitle: Topic\nkeywords: [rust]\n---\nBody\n";
        assert_eq!(
            create_content_with_frontmatter(metadata, &[]).unwrap(),
            metadata
        );
    }

    #[test]
    fn hierarchical_render_quotes_yaml11_ambiguous_strings() {
        let tags = vec![
            "yes".into(),
            "on".into(),
            "2026-10-27".into(),
            "line\u{85}next".into(),
        ];
        let rendered = create_content_with_frontmatter("Body", &tags).unwrap();
        assert_eq!(
            rendered,
            "---\ntags: [\"yes\",\"on\",\"2026-10-27\",\"line\\u0085next\"]\n---\nBody"
        );
    }

    #[test]
    fn hierarchical_yaml_resolver_differences_are_explicit() {
        let fixture: Value =
            serde_json::from_str(include_str!("../tests/fixtures/upstream/hierarchical.json"))
                .unwrap();
        for case in fixture["compatibility_cases"].as_array().unwrap() {
            let input = case["input"].as_str().unwrap();
            let actual = parse_frontmatter(input).unwrap();
            match case["name"].as_str().unwrap() {
                "unquoted_yes" | "unquoted_on" | "unquoted_date" => {
                    assert_eq!(case["expected"]["error_type"], "ValidationError");
                    assert_eq!(actual.0.len(), 1); // documented YAML 1.2 resolver difference
                    assert_eq!(actual.1, "Body");
                }
                "duplicate_tags" => {
                    assert_eq!(case["expected"]["value"][0], serde_json::json!(["second"]));
                    assert_eq!(actual, (Vec::new(), input.to_owned()));
                }
                _ => assert_eq!(serde_json::json!(actual), case["expected"]["value"]),
            }
        }
    }

    #[test]
    fn hierarchical_tree_matches_upstream_fixture() {
        let case = fixtures()
            .into_iter()
            .find(|case| case["name"] == "filesystem_tree")
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let documents = case["input"]["files"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(name, _)| name.ends_with(".md"))
            .map(|(name, content)| (name.clone(), content.as_str().unwrap().to_owned()))
            .collect();
        let mut nodes = tree_from_documents(directory.path(), &documents).unwrap();
        fn relative(nodes: &mut [MemoryNode], base: &Path) {
            for node in nodes {
                node.path = node.path.strip_prefix(base).unwrap().to_owned();
                relative(&mut node.children, base);
            }
        }
        relative(&mut nodes, directory.path());
        assert_eq!(
            serde_json::to_value(nodes).unwrap(),
            case["expected"]["value"]
        );
    }

    #[test]
    fn hierarchical_regex_does_not_expose_backend_only_syntax() {
        let pattern = ContentPattern::parse(r"\p{L}").unwrap();
        assert!(!pattern.matches("letters").unwrap());
        assert!(pattern.matches(r"literal \p{L}").unwrap());
        assert!(ContentPattern::parse(r"\N{LATIN CAPITAL LETTER A}").is_err());
    }

    #[test]
    fn hierarchical_globs_match_python_fixtures() {
        for case in fixtures()
            .into_iter()
            .filter(|c| c["function"] == "fnmatch")
        {
            assert_eq!(
                glob_matches(
                    case["input"]["pattern"].as_str().unwrap(),
                    case["input"]["text"].as_str().unwrap()
                ),
                case["expected"]["value"].as_bool().unwrap(),
                "{}",
                case["name"]
            );
        }
    }

    #[test]
    fn hierarchical_search_matches_upstream_fixtures() {
        let cases = fixtures();
        let tree = cases
            .iter()
            .find(|c| c["name"] == "filesystem_tree")
            .unwrap();
        let docs = tree["input"]["files"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(p, _)| p.ends_with(".md"))
            .map(|(p, c)| (p.clone(), c.as_str().unwrap().to_owned()))
            .collect();
        for case in cases
            .iter()
            .filter(|c| c["function"] == "collect_matching_nodes")
        {
            let input = &case["input"];
            let search = SearchInput::new(
                input["path_pattern"].as_str().map(str::to_owned),
                input["content_pattern"].as_str().map(str::to_owned),
                serde_json::from_value(input["tags"].clone()).unwrap_or_default(),
                input["max_results"].as_u64().unwrap() as usize,
            )
            .unwrap();
            let found = search_memory(Path::new("/memory"), &docs, &search).unwrap();
            assert_eq!(
                serde_json::to_value(found).unwrap(),
                case["expected"]["value"],
                "{}",
                case["name"]
            );
        }
    }
    #[test]
    fn hierarchical_reads_match_upstream_ranges() {
        for case in fixtures()
            .into_iter()
            .filter(|c| c["function"] == "view_file_content")
        {
            let input = &case["input"];
            let docs = std::collections::BTreeMap::from([(
                "note.md".to_owned(),
                input["content"].as_str().unwrap().to_owned(),
            )]);
            let result = read_memory(
                Path::new("/memory"),
                &docs,
                "note.md",
                input["start_line"].as_i64(),
                input["end_line"].as_i64(),
            );
            if case["expected"].get("error_type").is_some() {
                assert!(result.is_err());
            } else {
                assert_eq!(
                    result.unwrap(),
                    case["expected"]["value"].as_str().unwrap(),
                    "{}",
                    case["name"]
                );
            }
        }
    }
    #[test]
    fn hierarchical_edit_uses_upstream_universal_newlines() {
        assert_eq!(edit_content("a\r\nb\r\n", "b", "x").unwrap(), "a\nx\n");
    }

    #[test]
    fn hierarchical_edit_matches_upstream() {
        let cases = fixtures();
        let edits: Vec<_> = cases
            .iter()
            .filter(|c| c["function"] == "replace_in_file")
            .collect();
        assert!(!edits.is_empty());
        for case in edits {
            let input = &case["input"];
            let result = edit_content(
                input["content"].as_str().unwrap(),
                input["old_str"].as_str().unwrap(),
                input["new_str"].as_str().unwrap(),
            );
            if case["expected"].get("error_type").is_some() {
                assert!(result.is_err(), "{}", case["name"]);
            } else {
                assert_eq!(
                    result.unwrap(),
                    case["expected"]["value"].as_str().unwrap(),
                    "{}",
                    case["name"]
                );
            }
        }
    }

    #[test]
    fn hierarchical_paths_parse_before_tree_creation() {
        for bad in [
            "../x.md",
            "/x.md",
            "x/../a.md",
            "./x.md",
            "a//b.md",
            "a\\b.md",
            "C:x.md",
            "x.txt",
        ] {
            assert!(MemoryPath::file(bad).is_err(), "{bad}");
        }
        let too_deep = format!("{}note.md", "nested/".repeat(64));
        assert!(MemoryPath::file(&too_deep).is_err());
        assert_eq!(
            MemoryPath::file("projects/日本語/.note.md")
                .unwrap()
                .as_str(),
            "projects/日本語/.note.md"
        );
        let docs = std::collections::BTreeMap::from([("../x.md".to_owned(), "body".to_owned())]);
        assert!(tree_from_documents(Path::new("/memory"), &docs).is_err());
    }

    #[test]
    fn hierarchical_documents_reject_conflicting_paths_and_oversize() {
        let docs = std::collections::BTreeMap::from([
            ("a.md".to_owned(), "body".to_owned()),
            ("a.md/b.md".to_owned(), "body".to_owned()),
        ]);
        assert!(tree_from_documents(Path::new("/memory"), &docs).is_err());
        let docs = std::collections::BTreeMap::from([("a.md".to_owned(), "a".repeat(1_048_577))]);
        assert!(tree_from_documents(Path::new("/memory"), &docs).is_err());
    }
}
