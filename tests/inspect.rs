use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};
use ubi_lang::Compiler;

fn compiler(sources: &[(&str, &str)]) -> Compiler {
    let mut compiler = Compiler::new();
    for (id, source) in sources {
        compiler
            .add_source(*id, source.as_bytes().to_vec())
            .unwrap();
    }
    let check = compiler.check();
    assert!(check.diagnostics.is_empty(), "{:?}", check.diagnostics);
    compiler
}

fn inspect(compiler: &Compiler) -> Value {
    serde_json::to_value(compiler.inspect().expect("valid sources inspect")).unwrap()
}

fn export<'a>(summary: &'a Value, module: usize, name: &str) -> &'a Value {
    summary["modules"][module]["exports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == name)
        .unwrap_or_else(|| panic!("missing export {name}: {summary}"))
}

#[test]
fn checked_interfaces_preserve_generic_contracts_and_source_order() {
    let source = r#"fn secret() -> string { "implementation secret" }
/// A public box.
export record Box<T: Equatable> { second: T, first: List<T> }
export enum Chain<T> { End, Next(T, Option<Chain<T>>) }
export fn transform<T>(value: T, apply: fn(T) -> T) -> (T, int) {
    (apply(value), 1)
}
fn inferred() { 42 }
export fn inferredCaller() -> int { inferred() }
export fn singleton(value: (int,)) -> (int,) { value }
"#;
    let checked = compiler(&[("main.ubi", source)]);
    let summary = inspect(&checked);
    assert_eq!(summary["schemaVersion"], 1);
    assert_eq!(summary["offsetUnit"], "utf8-byte");
    assert_eq!(summary["modules"][0]["id"], "main.ubi");
    assert_eq!(summary["modules"][0]["imports"], json!([]));
    let names: Vec<_> = summary["modules"][0]["exports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["Box", "Chain", "inferredCaller", "singleton", "transform"]
    );
    let record = export(&summary, 0, "Box");
    assert_eq!(record["kind"], "record");
    assert_eq!(record["documentation"], "A public box.");
    assert_eq!(
        record["typeParameters"],
        json!([{"name":"T","bound":"Equatable"}])
    );
    assert_eq!(
        record["fields"],
        json!([
            {"name":"second","type":"T"}, {"name":"first","type":"List<T>"}
        ])
    );
    let chain = export(&summary, 0, "Chain");
    assert_eq!(chain["kind"], "enum");
    assert_eq!(chain["typeParameters"], json!([{"name":"T","bound":null}]));
    assert_eq!(
        chain["variants"],
        json!([
            {"name":"End","payloads":[]},
            {"name":"Next","payloads":["T","Option<main.ubi::Chain<T>>"]}
        ])
    );
    let function = export(&summary, 0, "transform");
    assert_eq!(function["kind"], "function");
    assert_eq!(
        function["parameters"],
        json!([
            {"name":"value","type":"T"}, {"name":"apply","type":"fn(T) -> T"}
        ])
    );
    assert_eq!(function["returnType"], "(T, int)");
    assert_eq!(function["async"], false);
    assert_eq!(function["effects"], json!([]));
    assert_eq!(export(&summary, 0, "inferredCaller")["returnType"], "int");
    assert_eq!(export(&summary, 0, "singleton")["returnType"], "(int,)");
    let encoded = serde_json::to_string(&summary).unwrap();
    assert!(!encoded.contains("implementation secret"));
    assert!(!encoded.contains("secret"));
    assert!(!encoded.contains("body"));
}

#[test]
fn imported_nominal_types_keep_their_defining_module_and_import_names() {
    let summary = inspect(&compiler(&[
        ("z/main.ubi", "import { Item, Box } from \"../a/types.ubi\"; export fn wrap(item: Item) -> Box<Item> { Box { value: item } }"),
        ("a/types.ubi", "export record Item { name: string } export record Box<T> { value: T }"),
        ("b/other.ubi", "export record Item { count: int } export fn local(value: Item) -> Item { value }"),
    ]));
    let ids: Vec<_> = summary["modules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|module| module["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["a/types.ubi", "b/other.ubi", "z/main.ubi"]);
    assert_eq!(
        summary["modules"][2]["imports"],
        json!([
            {"names":["Item","Box"],"path":"../a/types.ubi","targetId":"a/types.ubi"}
        ])
    );
    assert_eq!(
        summary["modules"][2]["exports"].as_array().unwrap().len(),
        1
    );
    let wrap = export(&summary, 2, "wrap");
    assert_eq!(
        wrap["parameters"],
        json!([{"name":"item","type":"a/types.ubi::Item"}])
    );
    assert_eq!(wrap["returnType"], "a/types.ubi::Box<a/types.ubi::Item>");
    assert_eq!(
        export(&summary, 1, "local")["returnType"],
        "b/other.ubi::Item"
    );
}

#[test]
fn async_and_declared_effects_are_reported_without_executing_any_function() {
    let summary = inspect(&compiler(&[(
        "main.ubi",
        r#"
export async fn sync(storage: Storage, http: Http) -> unit effects(storage, http) { () }
export fn neverRun() -> int { 1 / 0 }
"#,
    )]));
    let function = export(&summary, 0, "sync");
    assert_eq!(function["async"], true);
    assert_eq!(function["effects"], json!(["http", "storage"]));
    assert_eq!(
        function["parameters"],
        json!([
            {"name":"storage","type":"Storage"}, {"name":"http","type":"Http"}
        ])
    );
    assert_eq!(function["returnType"], "unit");
}

#[test]
fn builtin_type_constructors_remain_unqualified() {
    let summary = inspect(&compiler(&[(
        "main.ubi",
        r#"
export fn lookup(values: Map<string, int>) -> Option<int> { mapGet(values, "one") }
export fn result() -> Result<int, HostError> { Result.Ok(1) }
"#,
    )]));
    let lookup = export(&summary, 0, "lookup");
    assert_eq!(
        lookup["parameters"],
        json!([{"name":"values","type":"Map<string, int>"}])
    );
    assert_eq!(lookup["returnType"], "Option<int>");
    assert_eq!(
        export(&summary, 0, "result")["returnType"],
        "Result<int, HostError>"
    );
}

#[test]
fn documentation_uses_adjacent_lexer_comments_and_utf8_byte_spans() {
    let source = "/// café\r\n///  second line\r\nexport fn documented() -> int { 1 }\r\n\
/// detached\r\n\r\nexport fn detached() -> int { 2 }\r\n\
fn private() -> string { \"/// pretend\" }\r\nexport fn afterString() -> int { 3 }\r\n\
fn earlier() -> int { 4 } /// trailing\r\nexport fn afterCode() -> int { 5 }\r\n\
fn body() -> int {\r\n/// nested\r\n6\r\n}\r\nexport fn afterBody() -> int { 7 }\r\n\
/* /// block */\r\nexport fn afterBlock() -> int { 8 }\r\n";
    let checked = compiler(&[("main.ubi", source)]);
    let summary = inspect(&checked);
    let function = export(&summary, 0, "documented");
    assert_eq!(function["documentation"], "café\n second line");
    let start = source.find("export fn documented").unwrap();
    let end = start + "export fn documented() -> int { 1 }".len();
    assert_eq!(
        function["span"],
        json!({"sourceId":"main.ubi","start":start,"end":end})
    );
    for name in [
        "detached",
        "afterString",
        "afterCode",
        "afterBody",
        "afterBlock",
    ] {
        assert_eq!(export(&summary, 0, name)["documentation"], "", "{name}");
    }
    assert_eq!(
        summary["modules"][0]["revision"],
        checked.source_revisions()[0].revision
    );
    let mut changed = Compiler::new();
    changed
        .add_source("main.ubi", source.replace("\r\n", "\n").into_bytes())
        .unwrap();
    let changed_summary = inspect(&changed);
    assert_ne!(
        summary["modules"][0]["revision"],
        changed_summary["modules"][0]["revision"]
    );
}

#[test]
fn stable_ordering_does_not_depend_on_source_insertion_and_private_modules_remain() {
    let sources = [
        (
            "z.ubi",
            "export fn z() -> int { 1 } export fn a() -> int { 2 }",
        ),
        ("a.ubi", "fn hidden() -> int { 3 }"),
    ];
    let first = inspect(&compiler(&sources));
    let second = inspect(&compiler(&[sources[1], sources[0]]));
    assert_eq!(first, second);
    assert_eq!(first["modules"][0]["id"], "a.ubi");
    assert_eq!(first["modules"][0]["exports"], json!([]));
}

#[test]
fn invalid_private_implementations_and_missing_imports_produce_only_diagnostics() {
    for source in [
        "export fn good() -> int { 1 } fn hidden() -> int { unknown }",
        "import { missing } from \"./missing.ubi\"; export fn good() -> int { 1 }",
    ] {
        let mut compiler = Compiler::new();
        compiler
            .add_source("main.ubi", source.as_bytes().to_vec())
            .unwrap();
        let expected = compiler.check().diagnostics;
        assert!(!expected.is_empty());
        let actual = compiler
            .inspect()
            .expect_err("invalid graph must not summarize");
        assert_eq!(actual, expected);
    }
}

#[test]
fn expanded_types_hit_the_summary_budget_without_partial_results() {
    let mut ty = String::from("int");
    for _ in 0..14 {
        ty = format!("({ty},{ty})");
    }
    let source = format!("export record Wide {{ huge: {ty} }}");
    let checked = compiler(&[("main.ubi", &source)]);
    let diagnostics = checked
        .inspect()
        .expect_err("type string exceeds 65536 bytes");
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "UBI0090"));
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ubi-inspect-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, source: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ubi"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn files(root: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            result.extend(files(&path));
        } else {
            result.push(path);
        }
    }
    result.sort();
    result
}

#[test]
fn cli_inspects_only_reachable_modules_and_leaves_every_file_untouched() {
    let project = TempDir::new();
    project.write(
        "app/main.ubi",
        "import { answer } from \"../lib.ubi\"; export fn main() -> int { answer() }",
    );
    project.write("lib.ubi", "export fn answer() -> int { 42 }");
    project.write("unrelated.ubi", "this is deliberately invalid");
    let before: Vec<_> = files(&project.0)
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect();
    let output = project.run(&["inspect", "app/main.ubi", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    let summary: Value = serde_json::from_slice(&output.stdout).unwrap();
    let ids: Vec<_> = summary["modules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|module| module["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["app/main.ubi", "lib.ubi"]);
    let default = project.run(&["inspect", "app/main.ubi"]);
    assert_eq!(default.status.code(), Some(0));
    assert_eq!(default.stdout, output.stdout);
    let after: Vec<_> = files(&project.0)
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect();
    assert_eq!(before, after);
    assert!(!project.0.join(".ubi-build").exists());
    assert!(!project.0.join(".ubi-dist").exists());
}

#[test]
fn cli_config_selection_and_errors_match_existing_check_contract() {
    let project = TempDir::new();
    project.write("ubi.json", r#"{"schemaVersion":1,"name":"inspect-test","entry":"src/main.ubi","targets":["web","cli"]}"#);
    project.write("src/main.ubi", "export fn main() -> int { 1 }");
    let configured = project.run(&["inspect", "--target", "cli", "--json"]);
    assert_eq!(
        configured.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&configured.stderr)
    );
    let summary: Value = serde_json::from_slice(&configured.stdout).unwrap();
    assert_eq!(summary["modules"][0]["id"], "src/main.ubi");
    project.write("src/main.ubi", "export fn main() -> int { missing }");
    let inspect = project.run(&["inspect", "--target", "cli", "--json"]);
    let check = project.run(&["check", "--target", "cli", "--json"]);
    assert_eq!(inspect.status.code(), Some(1));
    assert_eq!(inspect.stdout, check.stdout);
    assert_eq!(inspect.stderr, check.stderr);
    let diagnostic: Value = serde_json::from_slice(&inspect.stdout).unwrap();
    assert!(diagnostic.get("modules").is_none());
    let human = project.run(&["inspect", "--target", "cli"]);
    assert_eq!(human.status.code(), Some(1));
    assert!(human.stdout.is_empty());
    assert!(!human.stderr.is_empty());
    for args in [
        vec!["inspect", "missing.ubi", "--json"],
        vec!["inspect", "../outside.ubi", "--json"],
        vec!["inspect", "src/main.ubi", "--out-dir", "out"],
        vec!["inspect", "src/main.ubi", "--json", "--json"],
        vec!["inspect", "src/main.ubi", "--function", "main"],
    ] {
        let output = project.run(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
    }
}
