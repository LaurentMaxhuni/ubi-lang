use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_file;
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_file;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ubi-cli-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn write_bytes(&self, path: &str, contents: &[u8]) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn ubi(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ubi"))
        .args(args)
        .output()
        .unwrap()
}

fn ubi_in(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ubi"))
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap()
}

fn check(root: &Path, entry: &str) -> Output {
    ubi(&["check", entry, "--root", root.to_str().unwrap(), "--json"])
}

fn assert_run_success(output: Output, expected: &str) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

#[test]
fn run_selects_exported_functions_and_accepts_primitive_and_unit_arguments() {
    let project = TempDir::new();
    project.write(
        "main.ubi",
        r#"
export fn main() -> int { 42 }
export fn default(x: int, flag: bool, text: string, real: float) -> string {
  if (flag && x == 7 && real == 1.5) { text } else { "bad arguments" }
}
export fn isUnit(value: unit) -> bool { value == () }
export fn nothing() -> unit { () }
"#,
    );
    assert_run_success(
        ubi(&[
            "run",
            "main.ubi",
            "--root",
            project.path().to_str().unwrap(),
        ]),
        "42\n",
    );
    assert_run_success(
        ubi_in(
            project.path(),
            &[
                "run",
                "main.ubi",
                "--function",
                "default",
                "--args",
                "[7,true,\"雪😀\",1.5]",
            ],
        ),
        "雪😀\n",
    );
    assert_run_success(
        ubi_in(
            project.path(),
            &[
                "run",
                "--args",
                "[null]",
                "--function",
                "isUnit",
                "main.ubi",
            ],
        ),
        "true\n",
    );
    assert_run_success(
        ubi_in(
            project.path(),
            &["run", "main.ubi", "--function", "nothing"],
        ),
        "",
    );
    assert!(project.path().join(".ubi-build/main.mjs").is_file());
}

#[test]
fn run_prints_json_records_and_special_floats_with_unit_null() {
    let project = TempDir::new();
    project.write("main.ubi", r#"
export record Input { value: int, text: string, done: unit }
export record Output { value: int, text: string, done: unit, nan: float, positive: float, negative: float, zero: float }
export fn main(input: Input) -> Output {
  Output { value: input.value + 1, text: input.text, done: input.done, nan: 0.0 / 0.0, positive: 1.0 / 0.0, negative: -1.0 / 0.0, zero: -0.0 }
}
export fn nan() -> float { 0.0 / 0.0 }
export fn positive() -> float { 1.0 / 0.0 }
export fn negative() -> float { -1.0 / 0.0 }
export fn negativeZero() -> float { -0.0 }
export fn positiveZero() -> float { 0.0 }
export fn finite() -> float { 1.5 }
"#);
    let arguments = serde_json::json!([r#"{"value":4,"text":"😀","done":null}"#]).to_string();
    let output = ubi_in(project.path(), &["run", "main.ubi", "--args", &arguments]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        serde_json::json!({"value":5,"text":"😀","done":null,"nan":"NaN","positive":"+Infinity","negative":"-Infinity","zero":"-0"})
    );
    for (function, expected) in [
        ("nan", "\"NaN\"\n"),
        ("positive", "\"+Infinity\"\n"),
        ("negative", "\"-Infinity\"\n"),
        ("negativeZero", "\"-0\"\n"),
        ("positiveZero", "0\n"),
        ("finite", "1.5\n"),
    ] {
        assert_run_success(
            ubi_in(project.path(), &["run", "main.ubi", "--function", function]),
            expected,
        );
    }
}

#[test]
fn run_uses_reachable_imports_and_config_target_selection() {
    let project = TempDir::new();
    project.write(
        "app/main.ubi",
        "import { base } from \"../lib/math.ubi\"; export fn main() -> int { base() + 1 }",
    );
    project.write("lib/math.ubi", "export fn base() -> int { 41 }");
    project.write("unrelated.ubi", "invalid source must not be loaded");
    project.write("ubi.json", &project_config("app/main.ubi", "[\"cli\"]"));
    assert_run_success(ubi_in(project.path(), &["run"]), "42\n");
    assert!(project.path().join(".ubi-build/cli/app/main.mjs").is_file());
    assert!(project.path().join(".ubi-build/cli/lib/math.mjs").is_file());
    assert!(!project.path().join(".ubi-build/cli/unrelated.mjs").exists());
    project.write(
        "ubi.json",
        &project_config("app/main.ubi", "[\"web\",\"cli\"]"),
    );
    assert_run_success(
        ubi(&[
            "run",
            "--target",
            "web",
            "--root",
            project.path().to_str().unwrap(),
        ]),
        "42\n",
    );
    assert!(project.path().join(".ubi-build/web/app/main.mjs").is_file());
    for args in [
        vec!["run"],
        vec!["run", "--target", "desktop"],
        vec!["run", "app/main.ubi", "--target", "web"],
    ] {
        let output = ubi_in(project.path(), &args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    project.write("ubi.json", "invalid config");
    assert_run_success(ubi_in(project.path(), &["run", "app/main.ubi"]), "42\n");
}

#[test]
fn run_compile_errors_and_runtime_faults_exit_one_without_normal_output() {
    let project = TempDir::new();
    project.write("main.ubi", "export fn main() -> int { missing }");
    let output = ubi_in(project.path(), &["run", "main.ubi"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(
        diagnostic.contains("main.ubi:") && diagnostic.contains("UBI0010"),
        "{diagnostic}"
    );
    assert!(!project.path().join(".ubi-build").exists());
    project.write(
        "main.ubi",
        "export fn main() -> int { 1 / 0 } export fn overflow() -> int { 2147483647 + 1 }",
    );
    for (function, code) in [("main", "UBI-R0002"), ("overflow", "UBI-R0001")] {
        let output = ubi_in(project.path(), &["run", "main.ubi", "--function", function]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains(code));
    }
}

#[test]
fn run_rejects_invalid_options_arguments_and_entry_exports_as_operational_errors() {
    let project = TempDir::new();
    project.write("main.ubi", "export record Data { value: int } fn private() -> int { 7 } export fn main(x: int) -> int { 1 / 0 } export fn recordArg(x: Data) -> int { 1 / 0 }");
    let bad_record = serde_json::json!([r#"{"value":true}"#]).to_string();
    for options in [
        vec![],
        vec!["--args", "[true]"],
        vec!["--args", "[2147483648]"],
        vec!["--args", "[1,2]"],
        vec!["--args", "{"],
        vec!["--args", "{}"],
        vec!["--args", "null"],
        vec!["--args", "[NaN]"],
        vec!["--function", "missing"],
        vec!["--function", "private"],
        vec!["--function", "Data"],
        vec!["--function", "recordArg", "--args", "[{\"value\":1}]"],
        vec!["--function", "recordArg", "--args", &bad_record],
        vec!["--json"],
        vec!["--out-dir", "out"],
        vec!["--function"],
        vec!["--args"],
        vec!["--unknown"],
    ] {
        let mut args = vec!["run", "main.ubi"];
        args.extend(options);
        let output = ubi_in(project.path(), &args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("UBI-R0002"),
            "invalid input executed body: {args:?}"
        );
    }
    project.write("no-main.ubi", "export fn other() -> int { 1 }");
    assert_eq!(
        ubi_in(project.path(), &["run", "no-main.ubi"])
            .status
            .code(),
        Some(2)
    );
    for command in ["check", "build"] {
        for option in [["--function", "main"], ["--args", "[1]"]] {
            let output = ubi_in(project.path(), &[command, "main.ubi", option[0], option[1]]);
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert!(!output.stderr.is_empty());
        }
    }
    let missing_node = Command::new(env!("CARGO_BIN_EXE_ubi"))
        .current_dir(project.path())
        .env("PATH", "")
        .args(["run", "main.ubi", "--args", "[1]"])
        .output()
        .unwrap();
    assert_eq!(missing_node.status.code(), Some(2));
    assert!(missing_node.stdout.is_empty());
    assert!(!missing_node.stderr.is_empty());
}

#[test]
fn run_treats_unusual_paths_function_names_and_argument_text_as_data() {
    let project = TempDir::new();
    project.write(
        "space # & ; λ/entry file.ubi",
        "export fn default(text: string) -> string { text }",
    );
    let text = "quote: \"; $(echo SHELL_INJECTION) & echo SHELL_INJECTION; 雪😀\nsecond line";
    let arguments = serde_json::json!([text]).to_string();
    assert_run_success(
        ubi_in(
            project.path(),
            &[
                "run",
                "space # & ; λ/entry file.ubi",
                "--function",
                "default",
                "--args",
                &arguments,
            ],
        ),
        &format!("{text}\n"),
    );
    for function in [
        "default; echo SHELL_INJECTION",
        "default & echo SHELL_INJECTION",
        "$(echo SHELL_INJECTION)",
    ] {
        let output = ubi_in(
            project.path(),
            &[
                "run",
                "space # & ; λ/entry file.ubi",
                "--function",
                function,
                "--args",
                &arguments,
            ],
        );
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn check_loads_only_reachable_imports_and_emits_a_json_envelope() {
    let project = TempDir::new();
    project.write(
        "app/main.ubi",
        "import { base } from \"../lib/math.ubi\"; export fn answer() -> int { base() + 1 }",
    );
    project.write("lib/math.ubi", "export fn base() -> int { 41 }");
    project.write("unrelated.ubi", "this is intentionally invalid");

    let output = check(project.path(), "app/main.ubi");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(json.starts_with("{\"schemaVersion\":1,\"offsetUnit\":\"utf8-byte\""));
    assert!(json.contains("\"id\":\"app/main.ubi\""));
    assert!(json.contains("\"id\":\"lib/math.ubi\""));
    assert!(!json.contains("unrelated.ubi"));
    assert!(json.ends_with("\"diagnostics\":[]}\n"));
}

#[test]
fn check_defaults_project_root_to_the_working_directory() {
    let project = TempDir::new();
    project.write("main.ubi", "export fn answer() -> int { 42 }");

    let output = ubi_in(project.path(), &["check", "main.ubi", "--json"]);

    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("\"id\":\"main.ubi\""));
}

#[test]
fn missing_import_is_a_compiler_diagnostic_and_json_is_valid() {
    let project = TempDir::new();
    project.write(
        "main.ubi",
        "import { value } from \"./missing.ubi\"; export fn answer() -> int { value() }",
    );

    let output = check(project.path(), "main.ubi");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(json.contains("\"code\":\"UBI0012\""));
    assert!(json.contains("\"sourceId\":\"main.ubi\""));
    assert_json_parses(&json);
}

#[test]
fn directory_at_an_import_path_is_reported_as_a_missing_module() {
    let project = TempDir::new();
    project.write(
        "main.ubi",
        "import { value } from \"./library.ubi\"; export fn answer() -> int { value() }",
    );
    fs::create_dir(project.path().join("library.ubi")).unwrap();

    let output = check(project.path(), "main.ubi");

    assert_eq!(output.status.code(), Some(1));
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(json.contains("\"code\":\"UBI0012\""));
    assert_json_parses(&json);
}

#[test]
fn compiler_errors_fail_with_source_accurate_json_spans() {
    for (source, target, code) in [
        (
            "export fn answer() -> int { missing }",
            "missing",
            "UBI0010",
        ),
        ("export fn answer() -> bool { 1 }", "1", "UBI0020"),
    ] {
        let project = TempDir::new();
        project.write("main.ubi", source);

        let output = check(project.path(), "main.ubi");

        assert_eq!(output.status.code(), Some(1));
        let json = String::from_utf8(output.stdout).unwrap();
        let start = source.find(target).unwrap();
        let end = start + target.len();
        assert!(json.contains(&format!("\"code\":\"{code}\"")), "{json}");
        assert!(
            json.contains(&format!(
                "\"primary\":{{\"sourceId\":\"main.ubi\",\"start\":{start},\"end\":{end}}}"
            )),
            "{json}"
        );
        assert_json_parses(&json);
    }
}

#[test]
fn json_diagnostics_escape_control_characters_from_import_paths() {
    let project = TempDir::new();
    project.write(
        "main.ubi",
        "import { value } from \"./bad\\n.ubi\"; export fn answer() -> int { value() }",
    );

    let output = check(project.path(), "main.ubi");

    assert_eq!(output.status.code(), Some(1));
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(
        json.contains("Invalid relative module path: ./bad\\n.ubi"),
        "{json:?}"
    );
    assert_json_parses(&json);
}

#[test]
fn import_cycle_json_includes_related_spans() {
    let project = TempDir::new();
    project.write(
        "a.ubi",
        "import { b } from \"./b.ubi\"; export fn a() -> int { b() }",
    );
    project.write(
        "b.ubi",
        "import { a } from \"./a.ubi\"; export fn b() -> int { a() }",
    );

    let output = check(project.path(), "a.ubi");

    assert_eq!(output.status.code(), Some(1));
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(json.contains("\"related\":[{\"span\":{"));
    assert_json_parses(&json);
}

#[test]
fn build_writes_mjs_without_clobbering_other_output_files() {
    let project = TempDir::new();
    project.write("main.ubi", "export fn answer() -> int { 42 }");
    project.write(".ubi-build/keep.txt", "keep me");
    project.write(".ubi-build/package.json", "user metadata");

    let output = ubi(&[
        "build",
        "main.ubi",
        "--root",
        project.path().to_str().unwrap(),
        "--json",
    ]);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(project.path().join(".ubi-build/main.mjs").is_file());
    assert_eq!(
        fs::read_to_string(project.path().join(".ubi-build/keep.txt")).unwrap(),
        "keep me"
    );
    assert_eq!(
        fs::read_to_string(project.path().join(".ubi-build/package.json")).unwrap(),
        "user metadata"
    );
    assert_json_parses(std::str::from_utf8(&output.stdout).unwrap());
}

#[test]
fn failed_build_does_not_create_generated_artifacts() {
    let project = TempDir::new();
    project.write("main.ubi", "export fn answer() -> int { missing }");
    let output_dir = project.path().join("custom-out");

    let output = ubi(&[
        "build",
        "main.ubi",
        "--root",
        project.path().to_str().unwrap(),
        "--out-dir",
        output_dir.to_str().unwrap(),
        "--json",
    ]);

    assert_eq!(output.status.code(), Some(1));
    assert!(!output_dir.exists());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("\"code\":\"UBI0010\""));
}

#[test]
fn operational_errors_stay_on_stderr_even_in_json_mode() {
    let project = TempDir::new();
    let output = ubi(&[
        "check",
        "../outside.ubi",
        "--root",
        project.path().to_str().unwrap(),
        "--json",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("canonical root-relative .ubi source ID"));
}

#[test]
fn source_size_limit_is_checked_before_loading_and_reported_as_operational() {
    let project = TempDir::new();
    project.write_bytes("main.ubi", &vec![b' '; 1_048_577]);

    let output = check(project.path(), "main.ubi");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("UBI0090"));
}

#[test]
fn symlinked_imports_cannot_escape_the_project_root() {
    let project = TempDir::new();
    let outside = TempDir::new();
    outside.write("secret.ubi", "export fn secret() -> int { 42 }");
    project.write(
        "main.ubi",
        "import { secret } from \"./link.ubi\"; export fn answer() -> int { secret() }",
    );
    if symlink_file(
        outside.path().join("secret.ubi"),
        project.path().join("link.ubi"),
    )
    .is_err()
    {
        return;
    }

    let output = check(project.path(), "main.ubi");

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("\"code\":\"UBI0012\""));
}

#[test]
fn build_refuses_to_overwrite_an_artifact_symlink() {
    let project = TempDir::new();
    let outside = TempDir::new();
    project.write("main.ubi", "export fn answer() -> int { 42 }");
    outside.write("main.mjs", "preserve this file");
    fs::create_dir_all(project.path().join(".ubi-build")).unwrap();
    if symlink_file(
        outside.path().join("main.mjs"),
        project.path().join(".ubi-build/main.mjs"),
    )
    .is_err()
    {
        return;
    }

    let output = ubi(&[
        "build",
        "main.ubi",
        "--root",
        project.path().to_str().unwrap(),
        "--json",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("refusing to overwrite output symlink"));
    assert_eq!(
        fs::read_to_string(outside.path().join("main.mjs")).unwrap(),
        "preserve this file"
    );
}

#[test]
fn build_refuses_parent_symlinks_that_escape_the_output_root() {
    let project = TempDir::new();
    let outside = TempDir::new();
    project.write("nested/main.ubi", "export fn answer() -> int { 42 }");
    outside.write("main.mjs", "preserve this file");
    fs::create_dir_all(project.path().join(".ubi-build")).unwrap();
    if symlink_dir(outside.path(), project.path().join(".ubi-build/nested")).is_err() {
        return;
    }

    let output = ubi(&[
        "build",
        "nested/main.ubi",
        "--root",
        project.path().to_str().unwrap(),
        "--json",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("escapes canonical output root"));
    assert_eq!(
        fs::read_to_string(outside.path().join("main.mjs")).unwrap(),
        "preserve this file"
    );
}

fn assert_json_parses(json: &str) {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("node")
        .args([
            "-e",
            "let s='';process.stdin.on('data',d=>s+=d);process.stdin.on('end',()=>JSON.parse(s));",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(json.as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "invalid JSON: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn project_config(entry: &str, targets: &str) -> String {
    format!(
        "{{\"schemaVersion\":1,\"name\":\"contract-test\",\"entry\":\"{entry}\",\"targets\":{targets}}}"
    )
}

fn assert_project_operational_error(project: &TempDir, output: Output, case: &str) {
    assert_eq!(output.status.code(), Some(2), "{case}: {output:?}");
    assert!(output.stdout.is_empty(), "{case}: {output:?}");
    assert!(!output.stderr.is_empty(), "{case}: {output:?}");
    assert!(!project.path().join(".ubi-build").exists(), "{case}");
}

#[test]
fn project_single_target_check_and_build_load_config_from_root_or_cwd() {
    for command in ["check", "build"] {
        for explicit_root in [false, true] {
            let project = TempDir::new();
            let cwd = TempDir::new();
            project.write("ubi.json", &project_config("src/main.ubi", "[\"cli\"]"));
            project.write("src/main.ubi", "export fn answer() -> int { 42 }");
            let output = if explicit_root {
                ubi_in(
                    cwd.path(),
                    &[
                        command,
                        "--root",
                        project.path().to_str().unwrap(),
                        "--json",
                    ],
                )
            } else {
                ubi_in(project.path(), &[command, "--json"])
            };
            assert_eq!(output.status.code(), Some(0), "{output:?}");
            assert!(output.stderr.is_empty(), "{output:?}");
            let json = std::str::from_utf8(&output.stdout).unwrap();
            assert!(json.contains("\"schemaVersion\":1"), "{json}");
            assert!(json.contains("\"id\":\"src/main.ubi\""), "{json}");
            assert_json_parses(json);
            assert_eq!(
                project.path().join(".ubi-build/cli/src/main.mjs").is_file(),
                command == "build"
            );
            assert!(!cwd.path().join(".ubi-build").exists());
        }
    }
}

#[test]
fn project_all_targets_emit_the_same_esm_and_preserve_module_layout() {
    let project = TempDir::new();
    project.write(
        "ubi.json",
        &project_config("src/main.ubi", "[\"web\",\"mobile\",\"desktop\",\"cli\"]"),
    );
    project.write(
        "src/main.ubi",
        "import { base } from \"../lib/math.ubi\"; export fn answer() -> int { base() + 1 }",
    );
    project.write("lib/math.ubi", "export fn base() -> int { 41 }");
    let mut previous = None;
    for target in ["web", "mobile", "desktop", "cli"] {
        let output = ubi_in(project.path(), &["build", "--json", "--target", target]);
        assert_eq!(output.status.code(), Some(0), "{target}: {output:?}");
        assert!(output.stderr.is_empty());
        assert_json_parses(std::str::from_utf8(&output.stdout).unwrap());
        let directory = project.path().join(".ubi-build").join(target);
        let main = fs::read_to_string(directory.join("src/main.mjs")).unwrap();
        let library = fs::read_to_string(directory.join("lib/math.mjs")).unwrap();
        assert!(main.contains("../lib/math.mjs"), "{main}");
        assert!(main.contains("export"), "{main}");
        if let Some(expected) = &previous {
            assert_eq!(&(main.clone(), library.clone()), expected);
        }
        previous = Some((main, library));
    }
}

#[test]
fn project_output_override_is_relative_to_process_cwd_and_preserves_user_files() {
    let project = TempDir::new();
    let cwd = TempDir::new();
    project.write("ubi.json", &project_config("src/main.ubi", "[\"desktop\"]"));
    project.write("src/main.ubi", "export fn answer() -> int { 42 }");
    cwd.write("generated/keep.txt", "preserve");
    let output = ubi_in(
        cwd.path(),
        &[
            "build",
            "--root",
            project.path().to_str().unwrap(),
            "--out-dir",
            "generated",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(cwd.path().join("generated/src/main.mjs").is_file());
    assert_eq!(
        fs::read_to_string(cwd.path().join("generated/keep.txt")).unwrap(),
        "preserve"
    );
    assert!(!project.path().join(".ubi-build").exists());
    assert!(!project.path().join("generated").exists());
}

#[test]
fn project_rejects_invalid_configs_before_compilation_or_artifact_creation() {
    let valid = project_config("main.ubi", "[\"web\"]");
    let cases = [
        ("malformed", "{".to_owned()),
        ("non-object", "[]".to_owned()),
        ("missing fields", "{\"schemaVersion\":1}".to_owned()),
        (
            "unknown field",
            valid.replace("\"name\":", "\"extra\":0,\"name\":"),
        ),
        (
            "duplicate field",
            valid.replace(
                "\"schemaVersion\":1",
                "\"schemaVersion\":1,\"schemaVersion\":1",
            ),
        ),
        (
            "unsupported schema",
            valid.replace("\"schemaVersion\":1", "\"schemaVersion\":2"),
        ),
        (
            "wrong schema type",
            valid.replace("\"schemaVersion\":1", "\"schemaVersion\":\"1\""),
        ),
        ("blank name", valid.replace("contract-test", "  ")),
        ("control name", valid.replace("contract-test", "bad\\nname")),
        ("empty targets", project_config("main.ubi", "[]")),
        (
            "duplicate targets",
            project_config("main.ubi", "[\"web\",\"web\"]"),
        ),
        ("unknown target", project_config("main.ubi", "[\"server\"]")),
        (
            "case-sensitive target",
            project_config("main.ubi", "[\"Web\"]"),
        ),
        ("wrong targets type", project_config("main.ubi", "\"web\"")),
        ("trailing content", format!("{valid} false")),
    ];
    for (case, config) in cases {
        for command in ["check", "build"] {
            let project = TempDir::new();
            project.write("ubi.json", &config);
            project.write("main.ubi", "export fn answer() -> int { missing }");
            let output = ubi_in(project.path(), &[command, "--json"]);
            assert_project_operational_error(&project, output, case);
        }
    }
}

#[test]
fn project_rejects_noncanonical_entry_paths() {
    for entry in [
        "",
        "/main.ubi",
        "C:/main.ubi",
        "../main.ubi",
        "./main.ubi",
        "src//main.ubi",
        "src/../main.ubi",
        "src\\\\main.ubi",
        "main.js",
        "bad\\n.ubi",
        "bad:name.ubi",
    ] {
        let project = TempDir::new();
        project.write("ubi.json", &project_config(entry, "[\"web\"]"));
        project.write("main.ubi", "export fn answer() -> int { 42 }");
        let output = ubi_in(project.path(), &["build", "--json"]);
        assert_project_operational_error(&project, output, entry);
    }
}

#[test]
fn project_config_requires_regular_utf8_file_with_64_kib_limit() {
    for case in [
        "missing",
        "directory",
        "invalid utf8",
        "over limit",
        "missing entry",
    ] {
        let project = TempDir::new();
        match case {
            "directory" => fs::create_dir(project.path().join("ubi.json")).unwrap(),
            "invalid utf8" => project.write_bytes("ubi.json", &[0xff]),
            "over limit" => {
                let mut config = project_config("main.ubi", "[\"cli\"]").into_bytes();
                config.resize(65_537, b' ');
                project.write_bytes("ubi.json", &config);
            }
            "missing entry" => {
                project.write("ubi.json", &project_config("missing.ubi", "[\"cli\"]"))
            }
            _ => {}
        }
        project.write("main.ubi", "export fn answer() -> int { 42 }");
        let output = ubi_in(project.path(), &["build", "--json"]);
        assert_project_operational_error(&project, output, case);
    }
    let project = TempDir::new();
    let mut config = project_config("main.ubi", "[\"cli\"]").into_bytes();
    config.resize(65_536, b' ');
    project.write_bytes("ubi.json", &config);
    project.write("main.ubi", "export fn answer() -> int { 42 }");
    let output = ubi_in(project.path(), &["check", "--json"]);
    assert_eq!(output.status.code(), Some(0), "64 KiB boundary: {output:?}");
}

#[test]
fn project_target_selection_errors_are_operational() {
    for (targets, selection) in [
        ("[\"web\",\"cli\"]", None),
        ("[\"web\"]", Some("cli")),
        ("[\"web\"]", Some("server")),
        ("[\"web\"]", Some("Web")),
    ] {
        for command in ["check", "build"] {
            let project = TempDir::new();
            project.write("ubi.json", &project_config("main.ubi", targets));
            project.write("main.ubi", "export fn answer() -> int { missing }");
            let mut args = vec![command, "--json"];
            if let Some(target) = selection {
                args.extend(["--target", target]);
            }
            let output = ubi_in(project.path(), &args);
            assert_project_operational_error(&project, output, targets);
        }
    }
}

#[test]
fn project_does_not_search_parent_directory_for_config() {
    let project = TempDir::new();
    project.write("ubi.json", &project_config("main.ubi", "[\"cli\"]"));
    project.write("main.ubi", "export fn answer() -> int { 42 }");
    fs::create_dir(project.path().join("child")).unwrap();
    let output = ubi_in(&project.path().join("child"), &["build", "--json"]);
    assert_project_operational_error(&project, output, "no parent search");
    assert!(!project.path().join("child/.ubi-build").exists());
}

#[test]
fn explicit_entry_ignores_config_and_rejects_target_option() {
    for command in ["check", "build"] {
        let project = TempDir::new();
        project.write("ubi.json", "invalid config");
        project.write("main.ubi", "export fn answer() -> int { 42 }");
        let output = ubi_in(project.path(), &[command, "main.ubi", "--json"]);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert_json_parses(std::str::from_utf8(&output.stdout).unwrap());
        assert_eq!(
            project.path().join(".ubi-build/main.mjs").is_file(),
            command == "build"
        );
        assert!(!project.path().join(".ubi-build/cli").exists());

        let fresh = TempDir::new();
        fresh.write("main.ubi", "export fn answer() -> int { 42 }");
        let output = ubi_in(
            fresh.path(),
            &[command, "--target", "web", "main.ubi", "--json"],
        );
        assert_project_operational_error(&fresh, output, "explicit entry plus target");
    }
}

#[test]
fn project_source_errors_retain_diagnostic_envelope_and_exit_one() {
    for command in ["check", "build"] {
        let project = TempDir::new();
        project.write("ubi.json", &project_config("src/main.ubi", "[\"mobile\"]"));
        project.write("src/main.ubi", "export fn answer() -> int { missing }");
        let output = ubi_in(project.path(), &[command, "--json"]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        let json = std::str::from_utf8(&output.stdout).unwrap();
        assert!(json.contains("\"schemaVersion\":1"), "{json}");
        assert!(json.contains("\"offsetUnit\":\"utf8-byte\""), "{json}");
        assert!(json.contains("\"sourceId\":\"src/main.ubi\""), "{json}");
        assert!(json.contains("\"code\":\"UBI0010\""), "{json}");
        assert_json_parses(json);
        assert!(!project.path().join(".ubi-build").exists());
    }
}

#[test]
fn project_config_and_entry_symlinks_must_stay_inside_root() {
    for linked_config in [true, false] {
        let project = TempDir::new();
        let outside = TempDir::new();
        let config = project_config("main.ubi", "[\"cli\"]");
        let (source, destination) = if linked_config {
            outside.write("ubi.json", &config);
            project.write("main.ubi", "export fn answer() -> int { 42 }");
            (
                outside.path().join("ubi.json"),
                project.path().join("ubi.json"),
            )
        } else {
            project.write("ubi.json", &config);
            outside.write("main.ubi", "export fn answer() -> int { 42 }");
            (
                outside.path().join("main.ubi"),
                project.path().join("main.ubi"),
            )
        };
        if let Err(error) = symlink_file(source, destination) {
            eprintln!("SKIP project symlink containment (config={linked_config}): {error}");
            continue;
        }
        let output = ubi_in(project.path(), &["build", "--json"]);
        assert_project_operational_error(&project, output, "symlink escapes root");
        assert!(!outside.path().join(".ubi-build").exists());
    }

    let project = TempDir::new();
    project.write("config.json", &project_config("main.ubi", "[\"cli\"]"));
    project.write("actual.ubi", "export fn answer() -> int { 42 }");
    for (source, destination) in [("config.json", "ubi.json"), ("actual.ubi", "main.ubi")] {
        if let Err(error) = symlink_file(
            project.path().join(source),
            project.path().join(destination),
        ) {
            eprintln!("SKIP in-root project symlink acceptance: {error}");
            return;
        }
    }
    let output = ubi_in(project.path(), &["build", "--json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(project.path().join(".ubi-build/cli/main.mjs").is_file());
}
