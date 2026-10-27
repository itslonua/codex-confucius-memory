//! Markdown persistence and a redo journal at the Codex boundary.
//! Individual upstream file writes are not a transaction with rollout cursors.
use crate::{
    memory::{Memory, Message, Note, Summary},
    transcript::Cursor,
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString},
    fs::File,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    path::{Component, Path},
};

const LIMIT: usize = 32 * 1024 * 1024;

#[derive(Default, Serialize, Deserialize)]
pub struct Session {
    pub cursor: Cursor,
    pub memory: Memory,
    pub revision: u64,
    #[serde(default)]
    pub pending: bool,
}
#[derive(Default)]
pub struct Project {
    pub sessions: BTreeMap<String, Session>,
    pub notes: BTreeMap<String, String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeSession {
    cursor: Cursor,
    messages: Vec<Message>,
    revision: u64,
    pending: bool,
    has_summary: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Runtime {
    version: u32,
    sessions: BTreeMap<String, RuntimeSession>,
}
#[derive(Deserialize)]
struct Legacy {
    version: u32,
    sessions: BTreeMap<String, Session>,
    notes: BTreeMap<String, Note>,
}

fn identifier(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err("invalid stored session identifier".into());
    }
    Ok(())
}
fn document_path(value: &str) -> Result<crate::hierarchical_memory::MemoryPath, String> {
    crate::hierarchical_memory::MemoryPath::file(value)
}
fn cstring(value: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| "NUL in path".into())
}
fn os_error() -> String {
    std::io::Error::last_os_error().to_string()
}
fn bounded(mut file: File) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take((LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > LIMIT {
        return Err("project memory exceeds 32 MiB".into());
    }
    Ok(bytes)
}

/// An opened directory is the authority for every child operation. No child
/// symlink is followed, including during journal replay and tree traversal.
struct Dir(File);
impl Dir {
    fn root(path: &Path, create: bool) -> Result<Option<Self>, String> {
        if !path.is_absolute() {
            return Err("store root must be absolute".into());
        }
        // Resolve aliases only in ancestors (e.g. macOS /var). The selected
        // root's final component is always opened relative to that ancestor
        // with O_NOFOLLOW, never canonicalized after a symlink check.
        if path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err("store root must have normalized components".into());
        }
        let mut suffix = Vec::new();
        let mut ancestor = if let Some(name) = path.file_name() {
            suffix.push(name.to_owned());
            path.parent().ok_or("invalid store root")?.to_path_buf()
        } else {
            path.to_path_buf()
        };
        loop {
            match std::fs::symlink_metadata(&ancestor) {
                Ok(_) => break,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    suffix.push(ancestor.file_name().ok_or("invalid store root")?.to_owned());
                    if !ancestor.pop() {
                        return Err("invalid store root".into());
                    }
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        let mut resolved = ancestor.canonicalize().map_err(|e| e.to_string())?;
        for component in suffix.into_iter().rev() {
            resolved.push(component);
        }
        let mut current = Self(File::open("/").map_err(|e| e.to_string())?);
        for component in resolved.components() {
            match component {
                Component::RootDir => {}
                Component::Normal(name) => {
                    let name = name.to_str().ok_or("store path is not UTF-8")?;
                    let Some(next) = current.child(name, create)? else {
                        return Ok(None);
                    };
                    current = next;
                }
                _ => return Err("store root must have normalized components".into()),
            }
        }
        Ok(Some(current))
    }
    fn child(&self, name: &str, create: bool) -> Result<Option<Self>, String> {
        let name = cstring(name)?;
        if create {
            let rc = unsafe { libc::mkdirat(self.0.as_raw_fd(), name.as_ptr(), 0o700) };
            if rc != 0
                && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
            {
                return Err(os_error());
            }
            if rc == 0 {
                self.sync()?;
            }
        }
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
                return Ok(None);
            }
            return Err(os_error());
        }
        Ok(Some(Self(unsafe { File::from_raw_fd(fd) })))
    }
    fn file(&self, name: &str, create: bool) -> Result<Option<File>, String> {
        let name = cstring(name)?;
        let flags = if create {
            libc::O_RDWR | libc::O_CREAT
        } else {
            libc::O_RDONLY
        };
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                0o600,
            )
        };
        if fd < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
                return Ok(None);
            }
            return Err(os_error());
        }
        let file = unsafe { File::from_raw_fd(fd) };
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("memory entry is not a regular file".into());
        }
        Ok(Some(file))
    }
    fn bytes(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        self.file(name, false)?.map(bounded).transpose()
    }
    fn sync(&self) -> Result<(), String> {
        self.0.sync_all().map_err(|e| e.to_string())
    }
    fn names(&self) -> Result<Vec<String>, String> {
        // openat(".") obtains an independent directory offset (dup does not).
        let dot = cstring(".")?;
        let fd = unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                dot.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(os_error());
        }
        let stream = unsafe { libc::fdopendir(fd) };
        if stream.is_null() {
            unsafe {
                libc::close(fd);
            }
            return Err(os_error());
        }
        let mut result = Vec::new();
        let mut names_budget = LIMIT;
        loop {
            #[cfg(target_os = "macos")]
            unsafe {
                *libc::__error() = 0;
            }
            #[cfg(target_os = "linux")]
            unsafe {
                *libc::__errno_location() = 0;
            }
            let entry = unsafe { libc::readdir(stream) };
            if entry.is_null() {
                let error = std::io::Error::last_os_error();
                unsafe {
                    libc::closedir(stream);
                }
                if error.raw_os_error().unwrap_or(0) != 0 {
                    return Err(error.to_string());
                }
                result.sort();
                return Ok(result);
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            let name = match std::str::from_utf8(name) {
                Ok(name) => name.to_owned(),
                Err(_) => {
                    unsafe {
                        libc::closedir(stream);
                    }
                    return Err("memory path is not UTF-8".into());
                }
            };
            let Some(remaining) =
                names_budget.checked_sub(name.len() + std::mem::size_of::<String>())
            else {
                unsafe {
                    libc::closedir(stream);
                }
                return Err("directory metadata exceeds 32 MiB".into());
            };
            names_budget = remaining;
            result.push(name);
        }
    }
    fn is_directory(&self, name: &str) -> Result<bool, String> {
        let name = cstring(name)?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe {
            libc::fstatat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            return Err(os_error());
        }
        let mode = unsafe { stat.assume_init() }.st_mode & libc::S_IFMT;
        if mode == libc::S_IFLNK {
            return Err("symlink in memory store".into());
        }
        if mode != libc::S_IFREG && mode != libc::S_IFDIR {
            return Err("unsupported memory entry".into());
        }
        Ok(mode == libc::S_IFDIR)
    }
    fn remove(&self, name: &str) -> Result<(), String> {
        let directory = self.is_directory(name)?;
        if directory {
            let child = self.child(name, false)?.ok_or("directory disappeared")?;
            for entry in child.names()? {
                child.remove(&entry)?;
            }
        }
        let name = cstring(name)?;
        if unsafe {
            libc::unlinkat(
                self.0.as_raw_fd(),
                name.as_ptr(),
                if directory { libc::AT_REMOVEDIR } else { 0 },
            )
        } != 0
        {
            return Err(os_error());
        }
        self.sync()
    }
    fn rename(&self, old: &str, new: &str) -> Result<(), String> {
        let old = cstring(old)?;
        let new = cstring(new)?;
        if unsafe {
            libc::renameat(
                self.0.as_raw_fd(),
                old.as_ptr(),
                self.0.as_raw_fd(),
                new.as_ptr(),
            )
        } != 0
        {
            return Err(os_error());
        }
        self.sync()
    }
    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
        if let Some((parent, file)) = path.split_once('/') {
            let child = self
                .child(parent, true)?
                .ok_or("cannot create memory directory")?;
            child.write(file, bytes)?;
            return self.sync();
        }
        // Refuse existing symlinks rather than silently replacing them.
        let _ = self.file(path, false)?;
        static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let (temporary, mut file) = loop {
            let sequence = NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let temporary = format!(".codex-memory-write-{}-{sequence}", std::process::id());
            let name = cstring(&temporary)?;
            let fd = unsafe {
                libc::openat(
                    self.0.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if fd >= 0 {
                break (temporary, unsafe { File::from_raw_fd(fd) });
            }
            if std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists {
                return Err(os_error());
            }
        };
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        self.rename(&temporary, path)
    }

    fn collect(
        &self,
        prefix: &str,
        output: &mut BTreeMap<String, String>,
        budget: &mut usize,
    ) -> Result<(), String> {
        if prefix.len() > 4096 || prefix.split('/').filter(|part| !part.is_empty()).count() >= 64 {
            return Err("memory path exceeds maximum length or depth".into());
        }
        for name in self.names()? {
            if self.is_directory(&name)? {
                self.child(&name, false)?
                    .ok_or("directory disappeared")?
                    .collect(&format!("{prefix}{name}/"), output, budget)?;
            } else if name.ends_with(".md") {
                let value = String::from_utf8(self.bytes(&name)?.ok_or("file disappeared")?)
                    .map_err(|e| e.to_string())?;
                let path = format!("{prefix}{name}");
                document_path(&path)?;
                *budget = budget
                    .checked_sub(path.len() + value.len())
                    .ok_or("project memory exceeds 32 MiB")?;
                output.insert(path, value);
            }
        }
        Ok(())
    }
}

fn location(path: &Path) -> Result<(&Path, String), String> {
    let parent = path.parent().ok_or("memory path has no parent")?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("invalid memory filename")?;
    if stem.len() != 64 || !stem.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("memory filename must be a project hash".into());
    }
    Ok((parent, stem.to_owned()))
}
fn legacy(root: &Dir, stem: &str) -> Result<Project, String> {
    let Some(bytes) = root.bytes(&format!("{stem}.json"))? else {
        return Ok(Project::default());
    };
    let old: Legacy =
        serde_json::from_slice(&bytes).map_err(|e| format!("corrupt legacy memory: {e}"))?;
    if old.version != 1 {
        return Err("unsupported memory version".into());
    }
    let mut notes = BTreeMap::new();
    for (id, note) in old.notes {
        if id != note.id.as_str() {
            return Err("corrupt memory: note key does not match its id".into());
        }
        // Preserve all legacy fields without inventing audit or tags metadata.
        let raw = serde_json::to_value(note).map_err(|e| e.to_string())?;
        let title = raw["title"].as_str().ok_or("invalid legacy title")?;
        let body = raw["body"].as_str().ok_or("invalid legacy body")?;
        let evidence = raw["evidence"]
            .as_array()
            .ok_or("invalid legacy evidence")?;
        let metadata = serde_yaml_ng::to_string(&serde_json::json!({"id":id,"title":title}))
            .map_err(|e| e.to_string())?;
        let evidence = evidence
            .iter()
            .map(|v| v.as_str().ok_or("invalid legacy evidence"))
            .collect::<Result<Vec<_>, _>>()?;
        let mut text = format!("---\n{metadata}---\n\n{body}\n\n## Legacy evidence\n\n");
        for entry in evidence {
            let run = entry.split(|c| c != '`').map(str::len).max().unwrap_or(0);
            let fence = "`".repeat(3.max(run + 1));
            text.push_str(&format!("{fence}\n{entry}\n{fence}\n\n"));
        }
        notes.insert(format!("{id}.md"), text);
    }
    for id in old.sessions.keys() {
        identifier(id)?;
    }
    Ok(Project {
        sessions: old.sessions,
        notes,
    })
}
fn load_project(root: &Dir, stem: &str) -> Result<Project, String> {
    let Some(dir) = root.child(stem, false)? else {
        return legacy(root, stem);
    };
    if dir.child(".pending", false)?.is_some() {
        return Err("memory recovery needed; retry after a writer recovers the journal".into());
    }
    let Some(bytes) = dir.bytes("runtime.json")? else {
        // A crash while staging the first migration has no published v2 state.
        if dir.names()?.iter().all(|name| name == ".journal-tmp") {
            return legacy(root, stem);
        }
        return Err("incomplete v2 memory: missing runtime.json".into());
    };
    let mut total_bytes = bytes.len();
    let runtime: Runtime =
        serde_json::from_slice(&bytes).map_err(|e| format!("corrupt runtime: {e}"))?;
    if runtime.version != 2 {
        return Err("unsupported memory version".into());
    }
    let mut sessions = BTreeMap::new();
    for (id, value) in runtime.sessions {
        identifier(&id)?;
        let summary = if value.has_summary {
            let sessions_dir = dir
                .child("sessions", false)?
                .ok_or("missing sessions directory")?;
            let session_dir = sessions_dir
                .child(&id, false)?
                .ok_or("missing session directory")?;
            let text = String::from_utf8(
                session_dir
                    .bytes("summary.md")?
                    .ok_or("missing summary document")?,
            )
            .map_err(|e| e.to_string())?;
            total_bytes = total_bytes
                .checked_add(text.len())
                .ok_or("memory size overflow")?;
            if total_bytes > LIMIT {
                return Err("project memory exceeds 32 MiB".into());
            }
            Some(Summary::from_markdown(&text)?)
        } else {
            None
        };
        sessions.insert(
            id,
            Session {
                cursor: value.cursor,
                memory: Memory {
                    messages: value.messages,
                    summary,
                },
                revision: value.revision,
                pending: value.pending,
            },
        );
    }
    let mut notes = BTreeMap::new();
    if let Some(notes_dir) = dir.child("notes", false)? {
        notes_dir.collect("", &mut notes, &mut (LIMIT - total_bytes))?;
    }
    crate::hierarchical_memory::tree_from_documents(Path::new(""), &notes)?;
    Ok(Project { sessions, notes })
}

pub fn read(path: &Path) -> Result<Project, String> {
    read_with_legacy_boundary(path, || Ok(()))
}

fn read_with_legacy_boundary(
    path: &Path,
    before_legacy: impl FnOnce() -> Result<(), String>,
) -> Result<Project, String> {
    let (parent, stem) = location(path)?;
    let Some(root) = Dir::root(parent, false)? else {
        return Ok(Project::default());
    };
    let lock = root.file(&format!("{stem}.lock"), false)?;
    if let Some(lock) = &lock {
        FileExt::try_lock_shared(lock).map_err(|_| "memory is busy; retry reading".to_string())?;
    } else {
        if root.child(&stem, false)?.is_some() {
            return Err("v2 memory lock is missing".into());
        }
        // A writer can publish v2 after these absence checks. Without a shared
        // lock, only the immutable legacy snapshot is safe to inspect.
        before_legacy()?;
        return legacy(&root, &stem);
    }
    load_project(&root, &stem)
}

pub struct Writer {
    root: Dir,
    stem: String,
    _lock: File,
}
impl Writer {
    pub fn open(path: &Path) -> Result<Self, String> {
        let (parent, stem) = location(path)?;
        let root = Dir::root(parent, true)?.ok_or("cannot create store")?;
        let lock = root
            .file(&format!("{stem}.lock"), true)?
            .ok_or("cannot create lock")?;
        lock.try_lock_exclusive()
            .map_err(|_| "memory is busy; history will be retried on the next hook".to_string())?;
        let value = Self {
            root,
            stem,
            _lock: lock,
        };
        value.recover(&mut || Ok(()))?;
        Ok(value)
    }
    pub fn load(&self) -> Result<Project, String> {
        load_project(&self.root, &self.stem)
    }
    pub fn save(&self, project: &Project) -> Result<(), String> {
        self.save_checked(project, &mut || Ok(()))
    }
    fn save_checked(
        &self,
        project: &Project,
        checkpoint: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<(), String> {
        let mut documents = BTreeMap::new();
        let mut sessions = BTreeMap::new();
        for (id, session) in &project.sessions {
            identifier(id)?;
            if let Some(summary) = &session.memory.summary {
                let document = summary.to_markdown();
                // Legacy JSON accepted each field separately. Parse the full
                // persistence value before publishing any recovery journal.
                Summary::from_markdown(&document)?;
                documents.insert(format!("sessions/{id}/summary.md"), document);
            }
            sessions.insert(
                id.clone(),
                RuntimeSession {
                    cursor: session.cursor.clone(),
                    messages: session.memory.messages.clone(),
                    revision: session.revision,
                    pending: session.pending,
                    has_summary: session.memory.summary.is_some(),
                },
            );
        }
        for (path, content) in &project.notes {
            document_path(path)?;
            documents.insert(format!("notes/{path}"), content.clone());
        }
        crate::hierarchical_memory::tree_from_documents(Path::new(""), &project.notes)?;
        let runtime = serde_json::to_string(&Runtime {
            version: 2,
            sessions,
        })
        .map_err(|e| e.to_string())?;
        let manifest = serde_json::to_string(&documents.keys().collect::<Vec<_>>())
            .map_err(|e| e.to_string())?;
        if runtime.len() + manifest.len() + documents.values().map(String::len).sum::<usize>()
            > LIMIT
        {
            return Err("project memory exceeds 32 MiB".into());
        }
        let previous = self.load()?;
        if previous
            .notes
            .keys()
            .any(|path| !project.notes.contains_key(path))
            || previous
                .sessions
                .keys()
                .any(|id| !project.sessions.contains_key(id))
            || previous.sessions.iter().any(|(id, old)| {
                old.memory.summary.is_some() && project.sessions[id].memory.summary.is_none()
            })
        {
            return Err("removing stored notes, sessions or summaries is not supported".into());
        }
        for path in documents.keys() {
            for (position, _) in path.match_indices('/') {
                if documents.contains_key(&path[..position]) {
                    return Err("memory file conflicts with a directory".into());
                }
            }
        }
        let dir = self
            .root
            .child(&self.stem, true)?
            .ok_or("cannot create project directory")?;
        if dir.child(".pending", false)?.is_some() {
            return Err("recover pending memory before another save".into());
        }
        if dir.child(".journal-tmp", false)?.is_some() {
            dir.remove(".journal-tmp")?;
        }
        let stage = dir
            .child(".journal-tmp", true)?
            .ok_or("cannot create journal")?;
        for (path, content) in &documents {
            stage.write(path, content.as_bytes())?;
        }
        stage.write("runtime.json", runtime.as_bytes())?;
        stage.write("manifest.json", manifest.as_bytes())?;
        stage.sync()?;
        dir.rename(".journal-tmp", ".pending")?;
        self.root.sync()?;
        checkpoint()?;
        self.recover(checkpoint)
    }
    fn recover(&self, checkpoint: &mut dyn FnMut() -> Result<(), String>) -> Result<(), String> {
        let Some(dir) = self.root.child(&self.stem, false)? else {
            return Ok(());
        };
        let Some(stage) = dir.child(".pending", false)? else {
            return Ok(());
        };
        let manifest = stage
            .bytes("manifest.json")?
            .ok_or("missing journal manifest")?;
        let mut budget = LIMIT
            .checked_sub(manifest.len())
            .ok_or("journal exceeds 32 MiB")?;
        let list: Vec<String> =
            serde_json::from_slice(&manifest).map_err(|e| format!("invalid journal: {e}"))?;
        let mut contents = BTreeMap::new();
        for path in list {
            let valid = if let Some(note) = path.strip_prefix("notes/") {
                document_path(note).is_ok()
            } else if let Some(session) = path
                .strip_prefix("sessions/")
                .and_then(|p| p.strip_suffix("/summary.md"))
            {
                identifier(session).is_ok()
            } else {
                false
            };
            if !valid || contents.contains_key(&path) {
                return Err("invalid journal document path".into());
            }
            let mut part = stage.child(".", false)?.ok_or("journal disappeared")?;
            let mut pieces = path.split('/').peekable();
            let mut bytes = None;
            while let Some(piece) = pieces.next() {
                if pieces.peek().is_some() {
                    part = part
                        .child(piece, false)?
                        .ok_or("missing journal directory")?;
                } else {
                    bytes = part.bytes(piece)?;
                }
            }
            let bytes = bytes.ok_or("missing journal document")?;
            budget = budget
                .checked_sub(bytes.len())
                .ok_or("journal exceeds 32 MiB")?;
            contents.insert(path, bytes);
        }
        let runtime = stage
            .bytes("runtime.json")?
            .ok_or("missing journal runtime")?;
        if runtime.len() > budget {
            return Err("journal exceeds 32 MiB".into());
        }
        let parsed: Runtime = serde_json::from_slice(&runtime)
            .map_err(|e| format!("invalid journal runtime: {e}"))?;
        if parsed.version != 2 {
            return Err("unsupported journal runtime".into());
        }
        for (id, session) in &parsed.sessions {
            identifier(id)?;
            let summary_path = format!("sessions/{id}/summary.md");
            if session.has_summary {
                let bytes = contents
                    .get(&summary_path)
                    .ok_or("journal missing referenced summary")?;
                Summary::from_markdown(std::str::from_utf8(bytes).map_err(|e| e.to_string())?)?;
            } else if contents.contains_key(&summary_path) {
                return Err("journal has an unreferenced summary".into());
            }
        }
        for (path, bytes) in &contents {
            std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
            if let Some(id) = path
                .strip_prefix("sessions/")
                .and_then(|p| p.strip_suffix("/summary.md"))
            {
                if !parsed.sessions.contains_key(id) {
                    return Err("journal has an unknown session".into());
                }
            }
        }
        if runtime.len() + contents.values().map(Vec::len).sum::<usize>() > LIMIT {
            return Err("journal exceeds 32 MiB".into());
        }
        let notes = contents
            .iter()
            .filter_map(|(path, bytes)| {
                path.strip_prefix("notes/")
                    .map(|path| (path.to_owned(), bytes))
            })
            .map(|(path, bytes)| {
                Ok((
                    path,
                    String::from_utf8(bytes.clone()).map_err(|e| e.to_string())?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        crate::hierarchical_memory::tree_from_documents(Path::new(""), &notes)?;
        for (path, bytes) in contents {
            dir.write(&path, &bytes)?;
            checkpoint()?;
        }
        dir.write("runtime.json", &runtime)?;
        checkpoint()?;
        if dir.child(".committed", false)?.is_some() {
            dir.remove(".committed")?;
        }
        dir.rename(".pending", ".committed")?;
        dir.remove(".committed")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn path(dir: &tempfile::TempDir) -> std::path::PathBuf {
        dir.path().join(format!("{}.json", "a".repeat(64)))
    }
    fn summary() -> Summary {
        serde_json::from_value(json!({"conversation_context":"request", "technical_decisions":"decision", "implementation_progress":"progress", "technical_details":"details", "outstanding_items":"remaining", "next_step":"next"})).unwrap()
    }
    fn project() -> Project {
        Project {
            sessions: BTreeMap::from([(
                "session-one".into(),
                Session {
                    cursor: Cursor::default(),
                    memory: Memory {
                        messages: vec![Message {
                            role: "user".into(),
                            text: "pending requirement".into(),
                            boundary: true,
                        }],
                        summary: Some(summary()),
                    },
                    revision: 1,
                    pending: true,
                },
            )]),
            notes: BTreeMap::from([
                ("shared/a.md".into(), "---\ntags: [rust]\n---\nA\n".into()),
                ("project/b.md".into(), "B\n".into()),
            ]),
        }
    }
    #[test]
    fn roundtrip_markdown_with_separate_runtime_and_no_legacy_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = path(&dir);
        let before = project();
        {
            let writer = Writer::open(&path).unwrap();
            writer.save(&before).unwrap();
        }
        let after = read(&path).unwrap();
        assert_eq!(after.notes, before.notes);
        assert_eq!(
            serde_json::to_value(&after.sessions).unwrap(),
            serde_json::to_value(&before.sessions).unwrap()
        );
        assert!(!path.exists());
        let root = path.with_extension("");
        let runtime = std::fs::read_to_string(root.join("runtime.json")).unwrap();
        assert!(!runtime.contains("technical_decisions"));
        assert!(!runtime.contains("tags"));
        assert!(root.join("sessions/session-one/summary.md").exists());
    }
    #[test]
    fn recovery_after_every_commit_point_is_idempotent_and_blocks_readers() {
        // Marker publication, three document writes, service state write.
        for fail_after in 1..=5 {
            let dir = tempfile::tempdir().unwrap();
            let path = path(&dir);
            let mut new = project();
            {
                let writer = Writer::open(&path).unwrap();
                writer.save(&new).unwrap();
            }
            new.notes.insert(
                "shared/a.md".into(),
                "Changed exact-match edit result\n".into(),
            );
            new.sessions.get_mut("session-one").unwrap().revision = 2;
            {
                let writer = Writer::open(&path).unwrap();
                let mut point = 0;
                assert!(
                    writer
                        .save_checked(&new, &mut || {
                            point += 1;
                            if point == fail_after {
                                Err("injected crash".into())
                            } else {
                                Ok(())
                            }
                        })
                        .is_err()
                );
                assert!(read(&path).is_err());
            }
            assert!(read(&path).err().unwrap().contains("recovery needed"));
            {
                let writer = Writer::open(&path).unwrap();
                let saved = writer.load().unwrap();
                assert_eq!(saved.notes, new.notes);
                assert_eq!(saved.sessions["session-one"].revision, 2);
            }
            assert_eq!(read(&path).unwrap().notes, new.notes);
            {
                let writer = Writer::open(&path).unwrap();
                assert_eq!(writer.load().unwrap().notes, new.notes);
            }
        }
    }
    #[test]
    fn migration_retains_unsummarized_messages_cursor_and_original_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = path(&dir);
        let legacy = json!({"version":1,"sessions":{"pending-session":{"cursor":{"path":"/source/rollout","offset":42,"prefix_hash":"hash","complete":false},"memory":{"messages":[{"role":"user","text":"never lose this pending correction","boundary":true}],"summary":null},"revision":7,"pending":true}},"notes":{"shared/note":{"id":"shared/note","title":"Title: café","body":"Body\n## Embedded\n","evidence":["command --flag\nresult","path:42"]}}});
        let bytes = serde_json::to_vec(&legacy).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        assert!(!path.with_extension("").exists());
        let before = read(&path).unwrap();
        assert!(!path.with_extension("").exists());
        for _ in 0..2 {
            let writer = Writer::open(&path).unwrap();
            let data = writer.load().unwrap();
            writer.save(&data).unwrap();
        }
        let after = read(&path).unwrap();
        assert_eq!(
            serde_json::to_value(&after.sessions).unwrap(),
            legacy["sessions"]
        );
        assert_eq!(after.notes, before.notes);
        let note = &after.notes["shared/note.md"];
        for text in [
            "Title: café",
            "Body\n## Embedded\n",
            "command --flag\nresult",
            "path:42",
        ] {
            assert!(note.contains(text));
        }
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    #[test]
    fn corrupt_v2_does_not_fallback_and_readonly_missing_store_creates_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir
            .path()
            .join("missing")
            .join(format!("{}.json", "a".repeat(64)));
        assert!(read(&missing).unwrap().sessions.is_empty());
        assert!(!missing.parent().unwrap().exists());
        let path = path(&dir);
        {
            let writer = Writer::open(&path).unwrap();
            writer.save(&project()).unwrap();
        }
        std::fs::write(path.with_extension("").join("runtime.json"), b"{}").unwrap();
        std::fs::write(&path, b"{\"version\":1,\"sessions\":{},\"notes\":{}}").unwrap();
        assert!(read(&path).is_err());
    }
    #[test]
    fn invalid_paths_parse_before_any_project_documents() {
        let dir = tempfile::tempdir().unwrap();
        let path = path(&dir);
        let writer = Writer::open(&path).unwrap();
        let mut bad = project();
        bad.notes.insert("../escape.md".into(), "bad".into());
        assert!(writer.save(&bad).is_err());
        assert!(!path.with_extension("").exists());
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed_on_reads_writes_or_recovery() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let path = path(&dir);
        {
            let writer = Writer::open(&path).unwrap();
            writer.save(&project()).unwrap();
        }
        let file = path.with_extension("").join("notes/shared/a.md");
        std::fs::remove_file(&file).unwrap();
        let external = outside.path().join("protected.md");
        std::fs::write(&external, "untouched").unwrap();
        symlink(&external, &file).unwrap();
        assert!(read(&path).is_err());
        {
            let writer = Writer::open(&path).unwrap();
            assert!(writer.save(&project()).is_err());
        }
        assert_eq!(std::fs::read_to_string(&external).unwrap(), "untouched");
        // No journal was published because the existing tree failed parsing.
        assert!(Writer::open(&path).unwrap().load().is_err());
    }
    #[cfg(unix)]
    #[test]
    fn directory_swap_after_prepare_cannot_redirect_recovery() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let path = path(&dir);
        {
            let writer = Writer::open(&path).unwrap();
            writer.save(&project()).unwrap();
        }
        let project_dir = path.with_extension("");
        let writer = Writer::open(&path).unwrap();
        let mut first = true;
        let result = writer.save_checked(&project(), &mut || {
            if first {
                first = false;
                std::fs::rename(
                    project_dir.join("notes/shared"),
                    project_dir.join("notes/saved-shared"),
                )
                .unwrap();
                symlink(external.path(), project_dir.join("notes/shared")).unwrap();
                return Err("simulated swap and crash".into());
            }
            Ok(())
        });
        assert!(result.is_err());
        drop(writer);
        assert!(Writer::open(&path).is_err());
        assert_eq!(std::fs::read_dir(external.path()).unwrap().count(), 0);
    }
    #[cfg(unix)]
    #[test]
    fn selected_store_root_symlink_is_rejected() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), dir.path().join("alias")).unwrap();
        let path = dir
            .path()
            .join("alias")
            .join(format!("{}.json", "a".repeat(64)));
        assert!(read(&path).is_err());
        assert!(Writer::open(&path).is_err());
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    }
    #[test]
    fn corrupt_journal_is_rejected_before_any_apply() {
        let dir = tempfile::tempdir().unwrap();
        let path = path(&dir);
        let mut data = project();
        {
            let writer = Writer::open(&path).unwrap();
            writer.save(&data).unwrap();
        }
        data.notes
            .insert("shared/a.md".into(), "replacement".into());
        {
            let writer = Writer::open(&path).unwrap();
            assert!(
                writer
                    .save_checked(&data, &mut || Err("crash after prepare".into()))
                    .is_err()
            );
        }
        let root = path.with_extension("");
        std::fs::write(
            root.join(".pending/sessions/session-one/summary.md"),
            "truncated invalid summary",
        )
        .unwrap();
        assert!(Writer::open(&path).is_err());
        assert!(
            std::fs::read_to_string(root.join("notes/shared/a.md"))
                .unwrap()
                .contains("tags: [rust]")
        );
    }
    #[test]
    fn oversized_legacy_summary_fails_before_publish_and_retry_can_succeed() {
        let dir = tempfile::tempdir().unwrap();
        let path = path(&dir);
        let mut oversized = serde_json::to_value(summary()).unwrap();
        for value in oversized.as_object_mut().unwrap().values_mut() {
            *value = json!("x".repeat(3000));
        }
        let legacy = json!({"version":1,"sessions":{"one":{"cursor":{"path":"", "offset":0,"prefix_hash":"", "complete":false},"memory":{"messages":[],"summary":oversized},"revision":1,"pending":false}},"notes":{}});
        let bytes = serde_json::to_vec(&legacy).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        {
            let writer = Writer::open(&path).unwrap();
            let mut data = writer.load().unwrap();
            assert!(writer.save(&data).unwrap_err().contains("12000"));
            assert!(!path.with_extension("").exists());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            data.sessions.get_mut("one").unwrap().memory.summary = Some(summary());
            writer.save(&data).unwrap();
        }
        assert!(
            read(&path).unwrap().sessions["one"]
                .memory
                .summary
                .is_some()
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    #[test]
    fn note_directory_named_like_internal_temp_does_not_block_later_siblings() {
        let dir = tempfile::tempdir().unwrap();
        let path = path(&dir);
        let mut data = project();
        data.notes.insert(
            "shared/.codex-memory-write/note.md".into(),
            "real note".into(),
        );
        {
            let writer = Writer::open(&path).unwrap();
            writer.save(&data).unwrap();
        }
        data.notes
            .insert("shared/sibling.md".into(), "later sibling".into());
        {
            let writer = Writer::open(&path).unwrap();
            writer.save(&data).unwrap();
        }
        assert_eq!(read(&path).unwrap().notes, data.notes);
    }
    #[test]
    fn manually_created_overdeep_notes_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = path(&dir);
        {
            let writer = Writer::open(&path).unwrap();
            writer.save(&project()).unwrap();
        }
        let mut deep = path.with_extension("").join("notes");
        for _ in 0..64 {
            deep.push("d");
        }
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("note.md"), "too deep").unwrap();
        assert!(read(&path).err().unwrap().contains("depth"));
    }
    #[test]
    fn malformed_tag_metadata_is_rejected_before_publish_and_on_disk_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = path(&dir);
        let mut data = project();
        data.notes
            .insert("bad.md".into(), "---\ntags: [12]\n---\nBody".into());
        {
            let writer = Writer::open(&path).unwrap();
            assert!(writer.save(&data).is_err());
        }
        assert!(!path.with_extension("").exists());
        {
            let writer = Writer::open(&path).unwrap();
            writer.save(&project()).unwrap();
        }
        std::fs::write(
            path.with_extension("").join("notes/bad.md"),
            "---\ntags: [12]\n---\nBody",
        )
        .unwrap();
        assert!(read(&path).is_err());
    }
    #[test]
    fn reader_without_lock_never_enters_v2_published_after_absence_checks() {
        let dir = tempfile::tempdir().unwrap();
        let path = path(&dir);
        let legacy_bytes = br#"{"version":1,"sessions":{},"notes":{}}"#;
        std::fs::write(&path, legacy_bytes).unwrap();
        let mut active_writer = None;
        let snapshot = read_with_legacy_boundary(&path, || {
            let writer = Writer::open(&path)?;
            writer.save(&project())?;
            active_writer = Some(writer);
            Ok(())
        })
        .unwrap();
        // The newly published v2 remains exclusively locked. Returning it here
        // would bypass reader synchronization, even if its first save finished.
        assert!(snapshot.sessions.is_empty());
        assert!(snapshot.notes.is_empty());
        assert!(read(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), legacy_bytes);
        drop(active_writer);
        assert_eq!(read(&path).unwrap().notes, project().notes);
    }
}
