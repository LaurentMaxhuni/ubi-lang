use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use ubi_lang::{
    format_source, BuildResult, CheckResult, Compiler, Diagnostic, Severity, SourceError,
    MAX_MODULES, MAX_PROJECT_BYTES, MAX_SOURCE_BYTES,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Check,
    Inspect,
    Build,
    Run,
    Dev,
    Init,
    Package,
    Format,
    Version,
    Ui,
}

#[derive(Debug)]
struct Arguments {
    command: Command,
    entry: Option<String>,
    target: Option<String>,
    root: Option<PathBuf>,
    out_dir: Option<PathBuf>,
    json: bool,
    function: Option<String>,
    call_args: Option<String>,
    port: u16,
    format_check: bool,
    smoke: bool,
    capture: Option<PathBuf>,
}

pub(crate) fn run(
    arguments: impl IntoIterator<Item = OsString>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    match run_inner(arguments, stdout, stderr) {
        Ok(status) => status,
        Err(message) => {
            let _ = writeln!(stderr, "ubi: {message}");
            2
        }
    }
}

fn run_inner(
    arguments: impl IntoIterator<Item = OsString>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<u8, String> {
    let arguments = parse_arguments(arguments)?;
    if arguments.command == Command::Version {
        writeln!(stdout, "ubi {}", env!("CARGO_PKG_VERSION"))
            .map_err(|error| format!("cannot write version: {error}"))?;
        return Ok(0);
    }
    let working_directory = std::env::current_dir()
        .map_err(|error| format!("cannot read current working directory: {error}"))?;
    if arguments.command == Command::Init {
        return crate::package::init(&working_directory.join(arguments.entry.as_deref().unwrap()))
            .map(|()| 0);
    }
    let root_argument = arguments.root.as_deref().unwrap_or(&working_directory);
    let root = fs::canonicalize(root_argument).map_err(|error| {
        format!(
            "cannot open project root `{}`: {error}",
            root_argument.display()
        )
    })?;
    if !root.is_dir() {
        return Err(format!(
            "project root is not a directory: {}",
            root.display()
        ));
    }
    let (entry, target) = match &arguments.entry {
        Some(entry) => (
            entry.clone(),
            if arguments.command == Command::Package {
                arguments.target.clone()
            } else {
                None
            },
        ),
        None => {
            if arguments.command == Command::Format {
                (crate::project::entry(&root)?, None)
            } else {
                let selected = arguments
                    .target
                    .as_deref()
                    .or((arguments.command == Command::Dev).then_some("web"));
                let (entry, target) = crate::project::select(&root, selected)?;
                (entry, Some(target))
            }
        }
    };
    if !valid_source_id(&entry) {
        return Err(format!(
            "entry must be a canonical root-relative .ubi source ID: {}",
            entry
        ));
    }

    if arguments.command == Command::Dev {
        return crate::dev::serve(&root, &entry, arguments.port, stdout, stderr);
    }
    let compiler = load_compiler(&root, &entry)?;

    if arguments.command == Command::Ui {
        #[cfg(feature = "native-ui")]
        {
            let runtime = compiler.runtime().map_err(|diagnostics| {
                diagnostics
                    .iter()
                    .map(|d| d.message.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
            let app =
                ubi_lang::ui::NativeApplication::new(runtime, &entry).map_err(|e| e.to_string())?;
            if let Some(path) = arguments.capture {
                let adapter = ubi_ui_platform::capture(&app, &path, 1100, 820, false)
                    .map_err(|e| e.to_string())?;
                writeln!(
                    stdout,
                    "Native capture: {} (GPU: {adapter})",
                    path.display()
                )
                .map_err(|e| e.to_string())?;
            } else {
                ubi_ui_platform::run(
                    app,
                    ubi_ui_platform::Options {
                        smoke: arguments.smoke,
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())?;
            }
            return Ok(0);
        }
        #[cfg(not(feature = "native-ui"))]
        {
            let _ = (arguments.smoke, arguments.capture);
            return Err("native UI requires building with `cargo run --features native-ui -- ui <entry.ubi> --root <root>`".into());
        }
    }

    match arguments.command {
        Command::Inspect => match compiler.inspect() {
            Ok(summary) => {
                serde_json::to_writer(&mut *stdout, &summary)
                    .map_err(|error| format!("cannot write summary to stdout: {error}"))?;
                stdout
                    .write_all(b"\n")
                    .map_err(|error| format!("cannot write summary to stdout: {error}"))?;
                Ok(0)
            }
            Err(diagnostics) => report(&compiler, &diagnostics, arguments.json, stdout, stderr),
        },
        Command::Check => {
            let check = compiler.check();
            let status = report_check(&compiler, &check, arguments.json, stdout, stderr)?;
            Ok(status)
        }
        Command::Dev | Command::Ui => unreachable!(),
        Command::Init | Command::Version => unreachable!(),
        Command::Format => format_project(&root, &compiler, arguments.format_check, stdout, stderr),
        Command::Build | Command::Run | Command::Package => {
            let build = compiler.build();
            if has_errors(&build.diagnostics) {
                return report_build(&compiler, &build, arguments.json, stdout, stderr);
            }
            let out_dir = match arguments.out_dir.as_deref() {
                Some(out_dir) if out_dir.is_absolute() => out_dir.to_path_buf(),
                Some(out_dir) => working_directory.join(out_dir),
                None => match &target {
                    Some(target) if arguments.command == Command::Package => {
                        root.join(".ubi-dist").join(target)
                    }
                    Some(target) => root.join(".ubi-build").join(target),
                    None => root.join(".ubi-build"),
                },
            };
            let files = build
                .javascript
                .as_ref()
                .ok_or_else(|| "compiler produced no JavaScript output".to_owned())?;
            if arguments.command == Command::Package {
                crate::package::package(
                    &root,
                    &entry,
                    target.as_deref().unwrap(),
                    &out_dir,
                    files,
                )?;
            } else {
                write_artifacts(&out_dir, files)?;
            }
            if arguments.command == Command::Run {
                report_build(&compiler, &build, false, stdout, stderr)?;
                let module = fs::canonicalize(
                    out_dir.join(entry.strip_suffix(".ubi").unwrap().to_owned() + ".mjs"),
                )
                .map_err(|error| format!("cannot open generated entry: {error}"))?;
                let output = std::process::Command::new("node")
                    .arg("--eval")
                    .arg(NODE_RUNNER)
                    .arg("--")
                    .arg(module)
                    .arg(arguments.function.as_deref().unwrap_or("main"))
                    .arg(arguments.call_args.as_deref().unwrap_or("[]"))
                    .output()
                    .map_err(|error| format!("cannot launch Node.js; install Node.js and ensure `node` is on PATH: {error}"))?;
                stdout
                    .write_all(&output.stdout)
                    .map_err(|error| format!("cannot write run output: {error}"))?;
                stderr
                    .write_all(&output.stderr)
                    .map_err(|error| format!("cannot write run errors: {error}"))?;
                return Ok(match output.status.code() {
                    Some(0) => 0,
                    Some(2) => 2,
                    Some(1) => 1,
                    _ => 2,
                });
            }
            report_build(&compiler, &build, arguments.json, stdout, stderr)
        }
    }
}

fn parse_arguments(arguments: impl IntoIterator<Item = OsString>) -> Result<Arguments, String> {
    let mut args = arguments
        .into_iter()
        .map(|argument| {
            argument
                .into_string()
                .map_err(|_| "command-line arguments must be valid Unicode".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter();
    let command =
        match args.next().as_deref() {
            Some("check") => Command::Check,
            Some("build") => Command::Build,
            Some("run") => Command::Run,
            Some("inspect") => Command::Inspect,
            Some("dev") => Command::Dev,
            Some("init") => Command::Init,
            Some("package") => Command::Package,
            Some("fmt") => Command::Format,
            Some("ui") => Command::Ui,
            Some("--version") => Command::Version,
            _ => return Err(
                "expected `check`, `inspect`, `build`, `run`, `dev`, `init`, `package`, `fmt`, `ui`, or `--version`"
                    .to_owned(),
            ),
        };

    let mut entry = None;
    let mut target = None;
    let mut root = None;
    let mut out_dir = None;
    let mut json = false;
    let mut function = None;
    let mut call_args = None;
    let mut port = None;
    let mut format_check = false;
    let mut smoke = false;
    let mut capture = None;
    while let Some(argument) = args.next() {
        if command == Command::Version {
            return Err("`--version` accepts no additional arguments".to_owned());
        }
        if command == Command::Init && (argument.starts_with('-') || entry.is_some()) {
            return Err("expected `init <directory>`".to_owned());
        }
        match argument.as_str() {
            "--smoke" if command == Command::Ui && !smoke => smoke = true,
            "--capture" if command == Command::Ui && capture.is_none() => {
                capture = Some(PathBuf::from(
                    args.next().ok_or("`--capture` needs a new PNG path")?,
                ));
            }
            "--check" if command == Command::Format && !format_check => format_check = true,
            "--check" if command == Command::Format => {
                return Err("`--check` may be specified only once".to_owned())
            }
            "--port" if command == Command::Dev => {
                if port.is_some() {
                    return Err("`--port` may be specified only once".to_owned());
                }
                port = Some(
                    args.next()
                        .ok_or("`--port` needs a number")?
                        .parse::<u16>()
                        .map_err(|_| "`--port` must be between 0 and 65535")?,
                );
            }
            "--function" | "--args" if command == Command::Run => {
                let slot = if argument == "--function" {
                    &mut function
                } else {
                    &mut call_args
                };
                if slot.is_some() {
                    return Err(format!("`{argument}` may be specified only once"));
                }
                let value = args
                    .next()
                    .ok_or_else(|| format!("`{argument}` needs a value"))?;
                if argument == "--args" {
                    let parsed: serde_json::Value = serde_json::from_str(&value)
                        .map_err(|error| format!("invalid --args JSON array: {error}"))?;
                    if !parsed.is_array() {
                        return Err("`--args` must be a JSON array".to_owned());
                    }
                } else if value.is_empty() {
                    return Err("`--function` needs a nonempty name".to_owned());
                }
                *slot = Some(value);
            }
            "--function" | "--args" => return Err(format!("`{argument}` is valid only for `run`")),
            "--json" if !json => json = true,
            "--json" => return Err("`--json` may be specified only once".to_owned()),
            "--target" => {
                if command == Command::Ui {
                    return Err("`ui` uses the native desktop backend; omit --target".into());
                }
                if command == Command::Format {
                    return Err("`--target` is not supported by `fmt`".to_owned());
                }
                if target.is_some() {
                    return Err("`--target` may be specified only once".to_owned());
                }
                let platform = args
                    .next()
                    .ok_or_else(|| "`--target` needs a target".to_owned())?;
                if !crate::project::valid_target(&platform) {
                    return Err(format!(
                        "unknown target: {platform}; expected web, mobile, desktop, or cli"
                    ));
                }
                target = Some(platform);
            }
            "--root" => {
                if root.is_some() {
                    return Err("`--root` may be specified only once".to_owned());
                }
                root = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "`--root` needs a directory".to_owned())?,
                ));
            }
            "--out-dir" if matches!(command, Command::Build | Command::Package) => {
                if out_dir.is_some() {
                    return Err("`--out-dir` may be specified only once".to_owned());
                }
                out_dir = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "`--out-dir` needs a directory".to_owned())?,
                ));
            }
            "--out-dir" => {
                return Err("`--out-dir` is valid only for `build` and `package`".to_owned())
            }
            option if option.starts_with('-') => return Err(format!("unknown option: {option}")),
            value if entry.is_none() => entry = Some(value.to_owned()),
            _ => return Err("expected exactly one entry source ID".to_owned()),
        }
    }

    if command == Command::Init && entry.is_none() {
        return Err("expected `init <directory>`".to_owned());
    }
    if command == Command::Package && target.is_none() {
        return Err("expected `package [entry] --target <target>`".to_owned());
    }
    if command == Command::Ui && (entry.is_none() || (smoke && capture.is_some())) {
        return Err(
            "expected `ui <entry.ubi> [--root <directory>] [--smoke | --capture <new.png>]`".into(),
        );
    }

    if command != Command::Package && entry.is_some() && target.is_some() {
        return Err("`--target` requires project config mode; omit the entry source ID".to_owned());
    }
    if !matches!(
        command,
        Command::Check | Command::Inspect | Command::Build | Command::Package
    ) && json
    {
        return Err(
            "`--json` is supported only by `check`, `inspect`, `build`, and `package`".to_owned(),
        );
    }
    if command == Command::Dev && target.as_deref().is_some_and(|value| value != "web") {
        return Err("`dev` supports only the web target".to_owned());
    }
    Ok(Arguments {
        command,
        entry,
        target,
        root,
        out_dir,
        json,
        function,
        call_args,
        port: port.unwrap_or(3000),
        format_check,
        smoke,
        capture,
    })
}

pub(crate) fn load_compiler(root: &Path, entry: &str) -> Result<Compiler, String> {
    let mut compiler = Compiler::new();
    let mut budget = SourceBudget::default();
    let bytes = load_source(root, entry, true, &mut budget)?
        .ok_or_else(|| format!("entry source not found within project root: {entry}"))?;
    add_source(&mut compiler, entry, bytes)?;
    load_import_graph(root, &mut compiler, &mut budget)?;
    Ok(compiler)
}

fn format_project(
    root: &Path,
    compiler: &Compiler,
    check_only: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<u8, String> {
    let check = compiler.check();
    let mut diagnostics: Vec<_> = check
        .diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.code.starts_with("UBI000") || diagnostic.code == "UBI0090")
        .collect();
    if has_errors(&diagnostics) {
        return report(compiler, &diagnostics, false, stdout, stderr);
    }
    for import in check.imports {
        if !import
            .target_id
            .as_deref()
            .is_some_and(|id| compiler.contains_source(id))
        {
            return Err(format!(
                "cannot format unresolved import `{}` in `{}`",
                import.path, import.importer_id
            ));
        }
    }

    let mut budget = SourceBudget::default();
    let mut snapshot = Compiler::new();
    let mut changes = BTreeMap::new();
    let mut formatted_bytes = 0;
    for source in compiler.source_revisions() {
        let bytes = load_source(root, &source.id, true, &mut budget)?
            .ok_or_else(|| format!("source disappeared before formatting: {}", source.id))?;
        add_source(&mut snapshot, &source.id, bytes.clone())?;
        let text = std::str::from_utf8(&bytes).map_err(|error| {
            format!(
                "source changed to invalid UTF-8 before formatting: {}: {error}",
                source.id
            )
        })?;
        match format_source(text) {
            Ok(formatted) if formatted.as_bytes() != bytes => {
                formatted_bytes += formatted.len();
                let path = fs::canonicalize(root.join(&source.id))
                    .map_err(|error| format!("cannot resolve source `{}`: {error}", source.id))?;
                if !path.starts_with(root) {
                    return Err(format!(
                        "source escapes canonical project root: {}",
                        source.id
                    ));
                }
                changes.insert(
                    path.clone(),
                    FormatChange {
                        path,
                        original: bytes,
                        formatted,
                        staged: None,
                        backup: None,
                    },
                );
            }
            Ok(formatted) => formatted_bytes += formatted.len(),
            Err(mut diagnostic) => {
                diagnostic.primary.source_id = source.id;
                diagnostics.push(diagnostic);
            }
        }
    }
    if snapshot.source_revisions() != compiler.source_revisions() {
        return Err("source changed while preparing formatting; retry the command".to_owned());
    }
    if has_errors(&diagnostics) {
        return report(compiler, &diagnostics, false, stdout, stderr);
    }
    if formatted_bytes > MAX_PROJECT_BYTES {
        return Err("UBI0090: formatted project exceeds the project byte limit".to_owned());
    }
    if check_only {
        return Ok(u8::from(!changes.is_empty()));
    }
    replace_sources(&mut changes.into_values().collect::<Vec<_>>())?;
    Ok(0)
}

struct FormatChange {
    path: PathBuf,
    original: Vec<u8>,
    formatted: String,
    staged: Option<PathBuf>,
    backup: Option<PathBuf>,
}

fn format_temp(path: &Path, suffix: &str) -> Result<(PathBuf, fs::File), String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let parent = path.parent().unwrap();
    loop {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".ubi-fmt-{}-{serial}.{suffix}", std::process::id()));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "cannot stage formatting for `{}`: {error}",
                    path.display()
                ))
            }
        }
    }
}

fn replace_sources(changes: &mut [FormatChange]) -> Result<(), String> {
    let operation = (|| {
        for change in changes.iter_mut() {
            let metadata = fs::metadata(&change.path).map_err(|error| {
                format!("cannot inspect source `{}`: {error}", change.path.display())
            })?;
            if metadata.permissions().readonly() {
                return Err(format!("source is read-only: {}", change.path.display()));
            }
            // Opening without truncation catches read-only sources before any replacement.
            fs::OpenOptions::new()
                .write(true)
                .open(&change.path)
                .map_err(|error| {
                    format!("cannot replace source `{}`: {error}", change.path.display())
                })?;
            let (path, mut file) = format_temp(&change.path, "stage")?;
            change.staged = Some(path);
            file.write_all(change.formatted.as_bytes())
                .and_then(|()| file.sync_all())
                .map_err(|error| {
                    format!("cannot stage source `{}`: {error}", change.path.display())
                })?;
            file.set_permissions(metadata.permissions())
                .map_err(|error| {
                    format!(
                        "cannot preserve source permissions `{}`: {error}",
                        change.path.display()
                    )
                })?;
        }
        for change in changes.iter() {
            if !source_unchanged(&change.path, &change.original)? {
                return Err(format!(
                    "source changed before formatting: {}",
                    change.path.display()
                ));
            }
        }
        for change in changes.iter_mut() {
            let (backup, file) = format_temp(&change.path, "backup")?;
            drop(file);
            fs::remove_file(&backup).map_err(|error| {
                format!(
                    "cannot prepare source backup `{}`: {error}",
                    backup.display()
                )
            })?;
            fs::rename(&change.path, &backup).map_err(|error| {
                format!("cannot back up source `{}`: {error}", change.path.display())
            })?;
            change.backup = Some(backup);
        }
        for change in changes.iter() {
            if !source_unchanged(change.backup.as_ref().unwrap(), &change.original)? {
                return Err(format!(
                    "source changed before formatting: {}",
                    change.path.display()
                ));
            }
        }
        for change in changes.iter_mut() {
            fs::rename(change.staged.as_ref().unwrap(), &change.path).map_err(|error| {
                format!("cannot replace source `{}`: {error}", change.path.display())
            })?;
            change.staged = None;
        }
        Ok(())
    })();
    if let Err(mut message) = operation {
        for change in changes.iter_mut().rev() {
            if let Some(backup) = change.backup.take() {
                let restored = if change.staged.is_none() {
                    fs::remove_file(&change.path).and_then(|()| fs::rename(&backup, &change.path))
                } else {
                    fs::rename(&backup, &change.path)
                };
                if let Err(error) = restored {
                    message.push_str(&format!(
                        "; restoration failed: {error}; original preserved at `{}`",
                        backup.display()
                    ));
                }
            }
        }
        for change in changes.iter_mut() {
            if let Some(path) = change.staged.take() {
                let _ = fs::remove_file(path);
            }
        }
        return Err(message);
    }
    for change in changes.iter_mut() {
        if let Some(path) = change.backup.take() {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

fn source_unchanged(path: &Path, original: &[u8]) -> Result<bool, String> {
    let mut current = Vec::with_capacity(original.len());
    fs::File::open(path)
        .and_then(|file| {
            file.take(original.len() as u64 + 1)
                .read_to_end(&mut current)
        })
        .map_err(|error| format!("cannot recheck source `{}`: {error}", path.display()))?;
    Ok(current == original)
}

const NODE_RUNNER: &str = r#"
(async () => {
  const [file, name, text] = process.argv.slice(1);
  try {
    const module = await import(require('node:url').pathToFileURL(file).href);
    if (!Object.prototype.hasOwnProperty.call(module, name) || typeof module[name] !== 'function') {
      console.error(`ubi: exported function ${JSON.stringify(name)} not found; use export fn main() or --function <name>`);
      process.exitCode = 2;
      return;
    }
    const args = JSON.parse(text).map(value => value === null ? undefined : value);
    const value = await module[name](...args);
    if (value !== undefined) {
      console.log(typeof value === 'string' ? value : JSON.stringify(value, (_, item) => {
        if (item === undefined) return null;
        if (typeof item === 'number') {
          if (Number.isNaN(item)) return 'NaN';
          if (item === Infinity) return '+Infinity';
          if (item === -Infinity) return '-Infinity';
          if (Object.is(item, -0)) return '-0';
        }
        return item;
      }));
    }
  } catch (error) {
    const fault = error && typeof error.code === 'string' && error.code.startsWith('UBI-R');
    console.error(`ubi: ${fault ? error.code + ': ' : ''}${error && error.message ? error.message : String(error)}`);
    process.exitCode = fault ? 1 : 2;
  }
})();
"#;

pub(crate) fn valid_source_id(id: &str) -> bool {
    id.ends_with(".ubi")
        && !id.starts_with('/')
        && !id.contains('\\')
        && !id.chars().any(char::is_control)
        && id.split('/').all(|segment| {
            !segment.is_empty() && segment != "." && segment != ".." && !segment.contains(':')
        })
}

fn add_source(compiler: &mut Compiler, id: &str, bytes: Vec<u8>) -> Result<(), String> {
    compiler.add_source(id, bytes).map_err(|error| match error {
        SourceError::InvalidId(id) => format!("invalid source ID: {id}"),
        SourceError::DuplicateId(id) => format!("duplicate source ID: {id}"),
        SourceError::Diagnostic(diagnostic) => format!(
            "source resource limit at {}:{}-{}: {}",
            diagnostic.primary.source_id,
            diagnostic.primary.start,
            diagnostic.primary.end,
            diagnostic.message
        ),
    })
}

#[derive(Default)]
struct SourceBudget {
    bytes: usize,
    modules: usize,
}

fn load_import_graph(
    root: &Path,
    compiler: &mut Compiler,
    budget: &mut SourceBudget,
) -> Result<(), String> {
    let mut attempted = BTreeSet::new();
    loop {
        let imports = compiler.check().imports;
        let mut loaded_any = false;
        for request in imports {
            let Some(target_id) = request.target_id else {
                continue;
            };
            if compiler.contains_source(&target_id) || !attempted.insert(target_id.clone()) {
                continue;
            }
            if let Some(bytes) = load_source(root, &target_id, false, budget)? {
                add_source(compiler, &target_id, bytes)?;
                loaded_any = true;
            }
        }
        if !loaded_any {
            return Ok(());
        }
    }
}

fn load_source(
    root: &Path,
    id: &str,
    required: bool,
    budget: &mut SourceBudget,
) -> Result<Option<Vec<u8>>, String> {
    if !valid_source_id(id) {
        return Ok(None);
    }
    let path = root.join(id);
    let canonical = match fs::canonicalize(&path) {
        Ok(canonical) => canonical,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::InvalidInput
            ) =>
        {
            return Ok(None)
        }
        Err(error) => return Err(format!("cannot resolve source `{id}`: {error}")),
    };
    if !canonical.starts_with(root) {
        return Ok(None);
    }
    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("cannot inspect source `{id}`: {error}"))?;
    if !metadata.is_file() {
        return if required {
            Err(format!("entry source is not a regular file: {id}"))
        } else {
            Ok(None)
        };
    }
    if metadata.len() > MAX_SOURCE_BYTES as u64 {
        return Err(format!(
            "UBI0090: source `{id}` exceeds the {MAX_SOURCE_BYTES}-byte limit"
        ));
    }
    if budget.modules >= MAX_MODULES || budget.bytes + metadata.len() as usize > MAX_PROJECT_BYTES {
        return Err(format!(
            "UBI0090: project source resource limit exceeded at `{id}`"
        ));
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    fs::File::open(canonical)
        .map_err(|error| format!("cannot open source `{id}`: {error}"))?
        .take(MAX_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read source `{id}`: {error}"))?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(format!(
            "UBI0090: source `{id}` exceeds the {MAX_SOURCE_BYTES}-byte limit"
        ));
    }
    if budget.bytes + bytes.len() > MAX_PROJECT_BYTES {
        return Err(format!(
            "UBI0090: project source byte limit exceeded at `{id}`"
        ));
    }
    budget.bytes += bytes.len();
    budget.modules += 1;
    Ok(Some(bytes))
}

fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}

fn report_check(
    compiler: &Compiler,
    check: &CheckResult,
    json: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<u8, String> {
    report(compiler, &check.diagnostics, json, stdout, stderr)
}

fn report_build(
    compiler: &Compiler,
    build: &BuildResult,
    json: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<u8, String> {
    report(compiler, &build.diagnostics, json, stdout, stderr)
}

fn report(
    compiler: &Compiler,
    diagnostics: &[Diagnostic],
    json: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<u8, String> {
    if json {
        let output = diagnostic_json(&compiler.source_revisions(), diagnostics);
        stdout
            .write_all(output.as_bytes())
            .and_then(|()| stdout.write_all(b"\n"))
            .map_err(|error| format!("cannot write diagnostics to stdout: {error}"))?;
    } else {
        for diagnostic in diagnostics {
            writeln!(
                stderr,
                "{}:{}-{}: {}[{}]: {}",
                diagnostic.primary.source_id,
                diagnostic.primary.start,
                diagnostic.primary.end,
                diagnostic.severity.as_str(),
                diagnostic.code,
                diagnostic.message
            )
            .map_err(|error| format!("cannot write diagnostics to stderr: {error}"))?;
        }
    }
    Ok(if has_errors(diagnostics) { 1 } else { 0 })
}

fn diagnostic_json(sources: &[ubi_lang::SourceRevision], diagnostics: &[Diagnostic]) -> String {
    let mut output =
        String::from("{\"schemaVersion\":1,\"offsetUnit\":\"utf8-byte\",\"sources\":[");
    for (index, source) in sources.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"id\":");
        json_string(&mut output, &source.id);
        output.push_str(",\"revision\":");
        json_string(&mut output, &source.revision);
        output.push('}');
    }
    output.push_str("],\"diagnostics\":[");
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"code\":");
        json_string(&mut output, &diagnostic.code);
        output.push_str(",\"severity\":");
        json_string(&mut output, diagnostic.severity.as_str());
        output.push_str(",\"message\":");
        json_string(&mut output, &diagnostic.message);
        output.push_str(",\"primary\":{\"sourceId\":");
        json_string(&mut output, &diagnostic.primary.source_id);
        output.push_str(&format!(
            ",\"start\":{},\"end\":{}}},\"related\":[",
            diagnostic.primary.start, diagnostic.primary.end
        ));
        for (related_index, related) in diagnostic.related.iter().enumerate() {
            if related_index != 0 {
                output.push(',');
            }
            output.push_str("{\"span\":{\"sourceId\":");
            json_string(&mut output, &related.span.source_id);
            output.push_str(&format!(
                ",\"start\":{},\"end\":{}}},\"message\":",
                related.span.start, related.span.end
            ));
            json_string(&mut output, &related.message);
            output.push('}');
        }
        output.push_str("]}");
    }
    output.push_str("]}");
    output
}

fn json_string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{0008}' => output.push_str("\\b"),
            '\u{000c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{001f}' => {
                output.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

fn write_artifacts(
    out_dir: &Path,
    files: &std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    fs::create_dir_all(out_dir).map_err(|error| {
        format!(
            "cannot create output directory `{}`: {error}",
            out_dir.display()
        )
    })?;
    let output_root = fs::canonicalize(out_dir).map_err(|error| {
        format!(
            "cannot resolve output directory `{}`: {error}",
            out_dir.display()
        )
    })?;

    for (id, contents) in files {
        let relative = Path::new(id);
        if !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err(format!("invalid generated output path: {id}"));
        }
        let file_name = relative
            .file_name()
            .ok_or_else(|| format!("invalid generated output path: {id}"))?;
        let parent = relative.parent().unwrap_or_else(|| Path::new(""));
        let parent = ensure_output_directory(&output_root, parent)?;
        let path = parent.join(file_name);
        replace_artifact(&path, contents.as_bytes())?;
    }
    Ok(())
}

/// Replace the output directory entry without truncating any existing inode.
pub(crate) fn replace_artifact(path: &Path, contents: &[u8]) -> Result<(), String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let inspect = || {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "refusing to overwrite output symlink: {}",
                    path.display()
                ))
            }
            Ok(metadata) if !metadata.is_file() => {
                return Err(format!("output path is not a file: {}", path.display()))
            }
            Ok(metadata) if metadata.permissions().readonly() => {
                return Err(format!("output path is read-only: {}", path.display()))
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "cannot inspect output `{}`: {error}",
                    path.display()
                ))
            }
        }
        Ok(())
    };
    inspect()?;
    let (temporary, mut file) = loop {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let temporary = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!(".ubi-artifact-{}-{serial}.tmp", std::process::id()));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("cannot stage output `{}`: {error}", path.display())),
        }
    };
    let operation = (|| {
        file.write_all(contents)
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("cannot write output `{}`: {error}", path.display()))?;
        drop(file);
        inspect()?;
        fs::rename(&temporary, path)
            .map_err(|error| format!("cannot replace output `{}`: {error}", path.display()))
    })();
    if operation.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    operation
}

fn ensure_output_directory(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(format!(
                "invalid generated output directory: {}",
                relative.display()
            ));
        };
        let path = current.join(segment);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {}
            Ok(metadata) if !metadata.is_dir() => {
                return Err(format!(
                    "output parent is not a directory: {}",
                    path.display()
                ))
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&path).map_err(|error| {
                    format!(
                        "cannot create output directory `{}`: {error}",
                        path.display()
                    )
                })?;
            }
            Err(error) => {
                return Err(format!(
                    "cannot inspect output directory `{}`: {error}",
                    path.display()
                ))
            }
        }
        let canonical = fs::canonicalize(&path).map_err(|error| {
            format!(
                "cannot resolve output directory `{}`: {error}",
                path.display()
            )
        })?;
        if !canonical.starts_with(root) {
            return Err(format!(
                "output directory escapes canonical output root: {}",
                path.display()
            ));
        }
        if !canonical.is_dir() {
            return Err(format!(
                "output parent is not a directory: {}",
                path.display()
            ));
        }
        current = canonical;
    }
    Ok(current)
}
