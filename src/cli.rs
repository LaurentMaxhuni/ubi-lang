use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use ubi_lang::{
    BuildResult, CheckResult, Compiler, Diagnostic, Severity, SourceError, MAX_MODULES,
    MAX_PROJECT_BYTES, MAX_SOURCE_BYTES,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Check,
    Build,
    Run,
    Dev,
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
    let working_directory = std::env::current_dir()
        .map_err(|error| format!("cannot read current working directory: {error}"))?;
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
        Some(entry) => (entry.clone(), None),
        None => {
            let selected = arguments
                .target
                .as_deref()
                .or((arguments.command == Command::Dev).then_some("web"));
            let (entry, target) = crate::project::select(&root, selected)?;
            (entry, Some(target))
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

    match arguments.command {
        Command::Check => {
            let check = compiler.check();
            let status = report_check(&compiler, &check, arguments.json, stdout, stderr)?;
            Ok(status)
        }
        Command::Dev => unreachable!(),
        Command::Build | Command::Run => {
            let build = compiler.build();
            if has_errors(&build.diagnostics) {
                return report_build(&compiler, &build, arguments.json, stdout, stderr);
            }
            let out_dir = match arguments.out_dir.as_deref() {
                Some(out_dir) if out_dir.is_absolute() => out_dir.to_path_buf(),
                Some(out_dir) => working_directory.join(out_dir),
                None => match target {
                    Some(target) => root.join(".ubi-build").join(target),
                    None => root.join(".ubi-build"),
                },
            };
            let files = build
                .javascript
                .as_ref()
                .ok_or_else(|| "compiler produced no JavaScript output".to_owned())?;
            write_artifacts(&out_dir, files)?;
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
    let command = match args.next().as_deref() {
        Some("check") => Command::Check,
        Some("build") => Command::Build,
        Some("run") => Command::Run,
        Some("dev") => Command::Dev,
        _ => return Err("expected `check`, `build`, `run`, or `dev`".to_owned()),
    };

    let mut entry = None;
    let mut target = None;
    let mut root = None;
    let mut out_dir = None;
    let mut json = false;
    let mut function = None;
    let mut call_args = None;
    let mut port = None;
    while let Some(argument) = args.next() {
        match argument.as_str() {
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
            "--out-dir" if command == Command::Build => {
                if out_dir.is_some() {
                    return Err("`--out-dir` may be specified only once".to_owned());
                }
                out_dir = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "`--out-dir` needs a directory".to_owned())?,
                ));
            }
            "--out-dir" => return Err("`--out-dir` is valid only for `build`".to_owned()),
            option if option.starts_with('-') => return Err(format!("unknown option: {option}")),
            value if entry.is_none() => entry = Some(value.to_owned()),
            _ => return Err("expected exactly one entry source ID".to_owned()),
        }
    }

    if entry.is_some() && target.is_some() {
        return Err("`--target` requires project config mode; omit the entry source ID".to_owned());
    }
    if matches!(command, Command::Run | Command::Dev) && json {
        return Err("`--json` is supported only by `check` and `build`".to_owned());
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
    const value = module[name](...args);
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
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "refusing to overwrite output symlink: {}",
                    path.display()
                ))
            }
            Ok(metadata) if !metadata.is_file() => {
                return Err(format!("output path is not a file: {}", path.display()))
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
        fs::write(&path, contents)
            .map_err(|error| format!("cannot write output `{}`: {error}", path.display()))?;
    }
    Ok(())
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
