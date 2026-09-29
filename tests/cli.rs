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
