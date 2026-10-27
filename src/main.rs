mod app;
pub mod hierarchical_memory;
mod memory;
pub mod store;
mod transcript;
mod worker;

use serde_json::{Value, json};
use std::{
    env,
    io::{self, Read},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const HELP: &str = "codex-memory [OPTIONS] COMMAND\n\nCommands:\n  hook       Read Codex hook JSON on stdin (errors warn; never block)\n  compress   Force summarization; stdin: session_id, cwd, transcript_path\n  restore    Return saved context; stdin: session_id, cwd\n  hooks      Print native Codex hooks.json configuration\n  schema     Print the model output JSON schema\n  search QUERY  Find project notes matching all query words\n  read PATH     Read a Markdown note or directory (legacy IDs accepted)\n\nHierarchical search: search --path-pattern GLOB --content-pattern REGEX --tag TAG --max-results N\nRead range: read PATH --start-line N --end-line N (-1 means EOF)\n\nOptions:\n  --store PATH          Absolute memory directory (default: $CODEX_HOME/confucius-memory)\n  --cwd PATH            Project directory for search/read (default: current directory)\n  --codex PATH          Codex CLI executable (default: codex)\n  --model NAME          Summarizer model (default: hook model, then CLI default)\n  --max-tokens N        Estimated history threshold (default: 100000)\n  --retain-tokens N     Estimated history target (default: 50000)\n  --timeout-seconds N   Total model time per hook, 1..600 (default: 120)\n\nToken estimates use UTF-8 bytes/4. No native Codex transcript is rewritten.\n";
enum Command {
    Hook,
    Compress,
    Restore,
    Hooks,
    Schema,
    Lookup(app::Lookup),
}
struct Options {
    store: PathBuf,
    codex: PathBuf,
    model: Option<String>,
    max: usize,
    retain: usize,
    timeout: Duration,
    command: Command,
    cwd: Option<PathBuf>,
}
impl Options {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let home = env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".codex")))
            .ok_or("set CODEX_HOME or HOME")?;
        let mut options = Self {
            store: home.join("confucius-memory"),
            codex: "codex".into(),
            model: None,
            max: 100000,
            retain: 50000,
            timeout: Duration::from_secs(120),
            command: Command::Hook,
            cwd: None,
        };
        let mut command = None;
        let mut args = args.into_iter().peekable();
        let mut path_pattern = None;
        let mut content_pattern = None;
        let mut tags = Vec::new();
        let mut max_results = 20;
        let mut search_filters = false;
        let mut start_line = None;
        let mut end_line = None;
        while let Some(arg) = args.next() {
            if matches!(
                arg.as_str(),
                "hook" | "compress" | "restore" | "hooks" | "schema" | "search" | "read"
            ) {
                if command.is_some() {
                    return Err("only one command is allowed".into());
                }
                command = Some(match arg.as_str() {
                    "hook" => Command::Hook,
                    "compress" => Command::Compress,
                    "restore" => Command::Restore,
                    "hooks" => Command::Hooks,
                    "schema" => Command::Schema,
                    "search" => {
                        if args.peek().is_some_and(|value| !value.starts_with("--")) {
                            Command::Lookup(app::Lookup::Search(args.next().unwrap().try_into()?))
                        } else {
                            Command::Lookup(app::Lookup::SearchMemory(
                                hierarchical_memory::SearchInput::new(None, None, Vec::new(), 20)?,
                            ))
                        }
                    }
                    "read" => Command::Lookup(app::Lookup::Read {
                        path: args.next().ok_or("missing argument for read")?,
                        start: None,
                        end: None,
                    }),
                    _ => unreachable!(),
                });
                continue;
            }
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {arg}"))?;
            match arg.as_str() {
                "--path-pattern" => {
                    path_pattern = Some(value);
                    search_filters = true;
                }
                "--content-pattern" => {
                    content_pattern = Some(value);
                    search_filters = true;
                }
                "--tag" => {
                    tags.push(value);
                    search_filters = true;
                }
                "--max-results" => {
                    let count: i64 = value.parse().map_err(|_| "invalid max-results")?;
                    max_results = count.max(0) as usize;
                    search_filters = true;
                }
                "--start-line" => {
                    start_line = Some(value.parse().map_err(|_| "invalid start-line")?)
                }
                "--end-line" => end_line = Some(value.parse().map_err(|_| "invalid end-line")?),
                "--store" => options.store = value.into(),
                "--cwd" => options.cwd = Some(value.into()),
                "--codex" => options.codex = value.into(),
                "--model" if !value.trim().is_empty() => options.model = Some(value),
                "--max-tokens" => options.max = value.parse().map_err(|_| "invalid max-tokens")?,
                "--retain-tokens" => {
                    options.retain = value.parse().map_err(|_| "invalid retain-tokens")?
                }
                "--timeout-seconds" => {
                    options.timeout =
                        Duration::from_secs(value.parse().map_err(|_| "invalid timeout")?)
                }
                _ => return Err(format!("unknown option: {arg}")),
            }
        }
        options.command = command.ok_or("a command is required; see --help")?;
        match &mut options.command {
            Command::Lookup(app::Lookup::SearchMemory(input)) => {
                *input = hierarchical_memory::SearchInput::new(
                    path_pattern,
                    content_pattern,
                    tags,
                    max_results,
                )?;
            }
            _ if search_filters => {
                return Err("search filters require search without a positional query".into());
            }
            _ => {}
        }
        match &mut options.command {
            Command::Lookup(app::Lookup::Read { start, end, .. }) => {
                *start = start_line;
                *end = end_line;
            }
            _ if start_line.is_some() || end_line.is_some() => {
                return Err("line ranges require read".into());
            }
            _ => {}
        }
        if options.cwd.is_some() && !matches!(options.command, Command::Lookup(_)) {
            return Err("--cwd is only supported for search and read; hooks use event cwd".into());
        }
        if !options.store.is_absolute() {
            return Err("store path must be absolute".into());
        }
        if options.retain < 256
            || options.max <= options.retain.saturating_add(3000)
            || options.max > 160000
        {
            return Err("require retain >=256, max > retain+3000, max <=160000".into());
        }
        if options.timeout.is_zero() || options.timeout > Duration::from_secs(600) {
            return Err("timeout must be 1..600 seconds".into());
        }
        Ok(options)
    }
    fn hooks(&self) -> Result<Value, String> {
        let executable = env::current_exe().map_err(|e| e.to_string())?;
        let mut command = format!(
            "{} --store {} --codex {} --max-tokens {} --retain-tokens {} --timeout-seconds {}",
            quote(&executable)?,
            quote(&self.store)?,
            quote(&self.codex)?,
            self.max,
            self.retain,
            self.timeout.as_secs()
        );
        if let Some(model) = &self.model {
            command.push_str(&format!(" --model {}", quote(Path::new(model))?));
        }
        command.push_str(" hook");
        let handler = json!({"type":"command","command":command,"timeout":self.timeout.as_secs()+10,"async":true,"statusMessage":"Updating Confucius memory"});
        let mut restore = handler.clone();
        // Restoration is local read-only I/O. Keep its startup wait bounded;
        // generation runs in native background hooks and retains its own budget.
        restore["async"] = json!(false);
        restore["timeout"] = json!(3);
        restore["additionalContextLimit"] = json!(5000);
        restore["statusMessage"] = json!("Restoring Confucius memory");
        Ok(
            json!({"description":"Confucius memory adaptation for Codex; automatic LLM summaries and selective notes", "hooks":{
                "PreCompact":[{"matcher":"^(auto|manual)$","hooks":[handler]}],
                "Stop":[{"hooks":[handler]}],
                "SessionStart":[{"matcher":"^(startup|resume|compact)$","hooks":[restore]}]
            }}),
        )
    }
}
fn quote(path: &Path) -> Result<String, String> {
    Ok(format!(
        "'{}'",
        path.to_str()
            .ok_or("path is not UTF-8")?
            .replace('\'', "'\\''")
    ))
}
fn run(args: Vec<String>) -> Result<Value, String> {
    let options = Options::parse(args)?;
    match &options.command {
        Command::Hooks => return options.hooks(),
        Command::Schema => return Ok(memory::output_schema()),
        Command::Lookup(request) => {
            let cwd = options
                .cwd
                .clone()
                .map(Ok)
                .unwrap_or_else(env::current_dir)
                .map_err(|e| e.to_string())?;
            return app::lookup(&options.store, &cwd, request);
        }
        _ => {}
    }
    let mut input = String::new();
    io::stdin()
        .take(1024 * 1024 + 1)
        .read_to_string(&mut input)
        .map_err(|e| e.to_string())?;
    if input.len() > 1024 * 1024 {
        return Err("hook input exceeds 1 MiB".into());
    }
    let mut event: Value =
        serde_json::from_str(&input).map_err(|e| format!("invalid hook input: {e}"))?;
    if matches!(options.command, Command::Compress | Command::Restore) {
        let object = event.as_object_mut().ok_or("input must be an object")?;
        object.insert(
            "hook_event_name".into(),
            json!(if matches!(options.command, Command::Compress) {
                "PreCompact"
            } else {
                "SessionStart"
            }),
        );
        object.insert("source".into(), json!("resume"));
        input = event.to_string();
    }
    let model = options
        .model
        .as_deref()
        .or_else(|| event.get("model").and_then(Value::as_str));
    let started = Instant::now();
    app::process(
        &options.store,
        &input,
        options.max,
        options.retain,
        &mut |prompt, schema| {
            let remaining = options
                .timeout
                .checked_sub(started.elapsed())
                .ok_or("memory generation deadline exceeded")?;
            worker::generate(prompt, schema, &options.codex, model, remaining)
        },
    )
}
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h") {
        print!("{HELP}");
        return;
    }
    // Option values and lookup arguments may themselves be named "hook".
    let mut scan = args.iter();
    let mut hook = false;
    while let Some(arg) = scan.next() {
        if arg.starts_with("--") {
            scan.next();
        } else {
            hook = arg == "hook";
            break;
        }
    }
    match run(args) {
        Ok(output) => println!("{output}"),
        Err(error) if hook => println!(
            "{}",
            json!({"systemMessage":format!("Confucius memory: {error}")})
        ),
        Err(error) => {
            eprintln!("codex-memory: {error}");
            std::process::exit(1);
        }
    }
}
