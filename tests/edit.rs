use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};
use ubi_lang::{Compiler, EditError, FunctionBodyEdit};

fn compiler(sources: &[(&str, &str)]) -> Compiler {
    let mut compiler = Compiler::new();
    for (id, text) in sources {
        compiler.add_source(*id, text.as_bytes().to_vec()).unwrap();
    }
    compiler
}

fn request(compiler: &Compiler, source: &str, function: &str, body: &str) -> FunctionBodyEdit {
    FunctionBodyEdit {
        schema_version: 1,
        source_id: source.into(),
        function: function.into(),
        source_revision: compiler
            .source_revisions()
            .into_iter()
            .find(|item| item.id == source)
            .unwrap()
            .revision,
        body: body.into(),
    }
}

#[test]
fn prepared_edit_preserves_unicode_crlf_comments_and_final_newline_exactly() {
    let original = "// café 🦀\r\n/// documented\r\nexport fn answer() -> int /* signature */ { 1 } // tail\r\nfn untouched() -> string { \"keep\" }";
    let checked = compiler(&[("main.ubi", original)]);
    assert!(checked.check().diagnostics.is_empty());
    let edit = request(&checked, "main.ubi", "answer", " \r\n{\n  42\n}\r\n ");
    let prepared = checked.prepare_function_body_edit(&edit).unwrap();
    let expected = original.replacen("{ 1 }", "{\n  42\n}", 1);
    assert_eq!(prepared.original_source(), original);
    assert_eq!(prepared.updated_source(), expected);
    let output = serde_json::to_value(&prepared).unwrap();
    let start = original.find("{ 1 }").unwrap();
    assert_eq!(output["sourceId"], "main.ubi");
    assert_eq!(output["function"], "answer");
    assert_eq!(output["start"], start);
    assert_eq!(output["end"], start + "{ 1 }".len());
    assert_eq!(output["replacement"], "{\n  42\n}");
    assert_eq!(output["sourceRevision"], edit.source_revision);
    let changed = compiler(&[("main.ubi", &expected)]);
    assert_eq!(
        output["resultingRevision"],
        changed.source_revisions()[0].revision
    );
    assert!(output.get("originalSource").is_none());
    assert!(output.get("updatedSource").is_none());
    let diff = output["diff"].as_str().unwrap();
    assert!(
        diff.starts_with("--- a/main.ubi\n+++ b/main.ubi\n@@ "),
        "{diff}"
    );
    assert!(diff.contains("\n-export fn answer()"));
    assert!(diff.contains("\n+export fn answer()"));
    assert!(diff.contains("\\ No newline at end of file"));
    assert_eq!(checked.source_revisions()[0].revision, edit.source_revision);
}

#[test]
fn private_function_body_edit_can_repair_an_existing_type_error() {
    let original = "fn helper() -> int { \"wrong\" } export fn answer() -> int { helper() }";
    let checked = compiler(&[("main.ubi", original)]);
    assert!(!checked.check().diagnostics.is_empty());
    let prepared = checked
        .prepare_function_body_edit(&request(&checked, "main.ubi", "helper", "{ 42 }"))
        .unwrap();
    assert_eq!(
        prepared.updated_source(),
        original.replace("{ \"wrong\" }", "{ 42 }")
    );
    assert!(compiler(&[("main.ubi", prepared.updated_source())])
        .check()
        .diagnostics
        .is_empty());
}

#[test]
fn stale_unknown_and_malformed_requests_never_mutate_the_compiler() {
    let original = "export fn answer() -> int { 1 } export record Item { value: int }";
    let checked = compiler(&[("main.ubi", original)]);
    let before = checked.source_revisions();
    for revision in [
        "sha256:bad".into(),
        format!("sha256:{}", "0".repeat(64)),
        format!("sha256:{}", "A".repeat(64)),
    ] {
        let mut edit = request(&checked, "main.ubi", "answer", "{ 2 }");
        edit.source_revision = revision;
        assert!(matches!(
            checked.prepare_function_body_edit(&edit),
            Err(EditError::Request(_))
        ));
    }
    for target in ["missing", "Item"] {
        let edit = request(&checked, "main.ubi", target, "{ 2 }");
        assert!(matches!(
            checked.prepare_function_body_edit(&edit),
            Err(EditError::Request(_))
        ));
    }
    let mut edit = request(&checked, "main.ubi", "answer", "{ 2 }");
    edit.source_id = "../outside.ubi".into();
    assert!(matches!(
        checked.prepare_function_body_edit(&edit),
        Err(EditError::Request(_))
    ));
    let mut edit = request(&checked, "main.ubi", "answer", "{ 2 }");
    edit.schema_version = 2;
    assert!(matches!(
        checked.prepare_function_body_edit(&edit),
        Err(EditError::Request(_))
    ));
    assert_eq!(checked.source_revisions(), before);
}

#[test]
fn only_one_complete_block_is_accepted_but_literal_and_comment_braces_are_valid() {
    let checked = compiler(&[("main.ubi", "export fn answer() -> string { \"old\" }")]);
    for body in [
        "",
        "42",
        "{",
        "{ \"ok\" } export fn injected() -> int { 3 }",
        "/* outside */ { \"ok\" }",
        "{ \"ok\" } // outside",
        "{ \"ok\" };",
        "{ \"ok\" } { \"second\" }",
    ] {
        let edit = request(&checked, "main.ubi", "answer", body);
        assert!(
            matches!(
                checked.prepare_function_body_edit(&edit),
                Err(EditError::Request(_))
            ),
            "{body}"
        );
    }
    let body = "{ /* } { */ let text = \"} {\"; // }\n text }";
    let prepared = checked
        .prepare_function_body_edit(&request(&checked, "main.ubi", "answer", body))
        .unwrap();
    assert!(prepared.updated_source().ends_with(body));
}

#[test]
fn candidate_type_and_effect_errors_report_candidate_revisions_without_success() {
    let original = "export fn answer() -> int { 1 }";
    let checked = compiler(&[("main.ubi", original)]);
    let edit = request(&checked, "main.ubi", "answer", "{ missing }");
    let expected = compiler(&[("main.ubi", "export fn answer() -> int { missing }")]);
    match checked.prepare_function_body_edit(&edit).unwrap_err() {
        EditError::Diagnostics {
            sources,
            diagnostics,
        } => {
            assert_eq!(sources, expected.source_revisions());
            assert_eq!(diagnostics, expected.check().diagnostics);
            assert_ne!(sources[0].revision, edit.source_revision);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    let pure = compiler(&[(
        "main.ubi",
        "export fn output(console: Console) -> unit { () }",
    )]);
    assert!(pure.check().diagnostics.is_empty());
    match pure
        .prepare_function_body_edit(&request(
            &pure,
            "main.ubi",
            "output",
            "{ consoleLog(console, \"forbidden\"); () }",
        ))
        .unwrap_err()
    {
        EditError::Diagnostics { diagnostics, .. } => assert!(
            diagnostics.iter().any(|item| item.code == "UBI0040"),
            "{diagnostics:?}"
        ),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn dependency_errors_block_edits_and_target_syntax_errors_use_original_revisions() {
    let checked = compiler(&[
        (
            "main.ubi",
            "import { helper } from \"./lib.ubi\"; export fn answer() -> int { helper() }",
        ),
        ("lib.ubi", "export fn helper() -> int { missing }"),
    ]);
    assert!(matches!(
        checked.prepare_function_body_edit(&request(&checked, "main.ubi", "answer", "{ 3 }")),
        Err(EditError::Diagnostics { .. })
    ));
    let broken = compiler(&[("main.ubi", "export fn answer() -> int { 1 ")]);
    match broken
        .prepare_function_body_edit(&request(&broken, "main.ubi", "answer", "{ 3 }"))
        .unwrap_err()
    {
        EditError::Diagnostics {
            sources,
            diagnostics,
        } => {
            assert_eq!(sources, broken.source_revisions());
            assert!(!diagnostics.is_empty());
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn no_op_keeps_revision_and_diff_empty_and_request_deserialization_is_strict() {
    let checked = compiler(&[("main.ubi", "export fn answer() -> int { 1 }\n")]);
    let edit = request(&checked, "main.ubi", "answer", " \n{ 1 }\t");
    let prepared = checked.prepare_function_body_edit(&edit).unwrap();
    assert_eq!(prepared.original_source(), prepared.updated_source());
    let value = serde_json::to_value(prepared).unwrap();
    assert_eq!(value["sourceRevision"], value["resultingRevision"]);
    assert_eq!(value["diff"], "");
    let valid = request_json(&edit);
    assert!(serde_json::from_value::<FunctionBodyEdit>(valid.clone()).is_ok());
    let mut unknown = valid.clone();
    unknown["unexpected"] = json!(true);
    assert!(serde_json::from_value::<FunctionBodyEdit>(unknown).is_err());
    let mut missing = valid.clone();
    missing.as_object_mut().unwrap().remove("body");
    assert!(serde_json::from_value::<FunctionBodyEdit>(missing).is_err());
    let duplicate = format!(
        "{{\"body\":\"{{ 2 }}\",{}",
        serde_json::to_string(&valid)
            .unwrap()
            .trim_start_matches('{')
    );
    assert!(serde_json::from_str::<FunctionBodyEdit>(&duplicate).is_err());
}

fn request_json(edit: &FunctionBodyEdit) -> Value {
    json!({"schemaVersion":edit.schema_version,"sourceId":edit.source_id,"function":edit.function,"sourceRevision":edit.source_revision,"body":edit.body})
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ubi-edit-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, path: &str, source: impl AsRef<[u8]>) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }

    fn run(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ubi"))
            .current_dir(&self.0)
            .args(arguments)
            .output()
            .unwrap()
    }

    fn edit_request(&self, source: &str, function: &str, body: &str) {
        let text = fs::read_to_string(self.0.join(source)).unwrap();
        let checked = compiler(&[(source, &text)]);
        self.write(
            "request.json",
            serde_json::to_vec(&request_json(&request(&checked, source, function, body))).unwrap(),
        );
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut result = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            result.extend(files(&path));
        } else {
            result.push((path.clone(), fs::read(&path).unwrap()));
        }
    }
    result.sort();
    result
}

fn success(output: Output) -> Value {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn cli_preview_writes_nothing_and_apply_changes_only_the_selected_body() {
    let project = TempDir::new();
    let original = "// café\r\nexport fn answer() -> int { 1 } // preserve\r\n";
    project.write("lib.ubi", original);
    project.write(
        "main.ubi",
        "import { answer } from \"./lib.ubi\"; export fn main() -> int { answer() }",
    );
    project.write("unrelated.ubi", "invalid unrelated source");
    project.write("keep.txt", "user data");
    project.edit_request("lib.ubi", "answer", "{ 42 }");
    let before = files(&project.0);
    let preview =
        success(project.run(&["edit", "main.ubi", "--request", "request.json", "--json"]));
    assert_eq!(preview["schemaVersion"], 1);
    assert_eq!(preview["applied"], false);
    assert_eq!(preview["edit"]["sourceId"], "lib.ubi");
    assert_eq!(files(&project.0), before);
    let applied = success(project.run(&[
        "edit",
        "main.ubi",
        "--request",
        "request.json",
        "--apply",
        "--json",
    ]));
    assert_eq!(applied["applied"], true);
    assert_eq!(applied["edit"], preview["edit"]);
    let expected = original.replace("{ 1 }", "{ 42 }");
    let after = files(&project.0);
    let expected_files: Vec<_> = before
        .into_iter()
        .map(|(path, bytes)| {
            if path == project.0.join("lib.ubi") {
                (path, expected.as_bytes().to_vec())
            } else {
                (path, bytes)
            }
        })
        .collect();
    assert_eq!(after, expected_files);
    let stale = project.run(&[
        "edit",
        "main.ubi",
        "--request",
        "request.json",
        "--apply",
        "--json",
    ]);
    assert_eq!(stale.status.code(), Some(2));
    assert!(stale.stdout.is_empty());
    assert_eq!(files(&project.0), after);
}

#[test]
fn cli_candidate_diagnostics_match_edited_bytes_and_leave_all_original_files() {
    let project = TempDir::new();
    project.write("main.ubi", "export fn answer() -> int { 1 }");
    project.edit_request("main.ubi", "answer", "{ missing }");
    let before = files(&project.0);
    let output = project.run(&[
        "edit",
        "main.ubi",
        "--request",
        "request.json",
        "--apply",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    let expected = compiler(&[("main.ubi", "export fn answer() -> int { missing }")]);
    assert_eq!(
        result["sources"][0]["revision"],
        expected.source_revisions()[0].revision
    );
    assert!(result.get("edit").is_none());
    assert!(result["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "UBI0010"));
    assert_eq!(files(&project.0), before);
}

#[test]
fn cli_config_mode_human_preview_and_no_op_apply_follow_the_contract() {
    let project = TempDir::new();
    project.write(
        "ubi.json",
        r#"{"schemaVersion":1,"name":"edit-test","entry":"src/main.ubi","targets":["web","cli"]}"#,
    );
    project.write("src/main.ubi", "export fn answer() -> int { 1 }\n");
    project.edit_request("src/main.ubi", "answer", "{ 2 }");
    let preview = project.run(&["edit", "--target", "cli", "--request", "request.json"]);
    assert_eq!(
        preview.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    assert!(String::from_utf8(preview.stdout)
        .unwrap()
        .starts_with("--- a/src/main.ubi\n+++ b/src/main.ubi\n"));
    project.edit_request("src/main.ubi", "answer", "{ 1 }");
    let path = project.0.join("src/main.ubi");
    let permissions = fs::metadata(&path).unwrap().permissions();
    let mut readonly = permissions.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let before = files(&project.0);
    let output = project.run(&[
        "edit",
        "--target",
        "cli",
        "--request",
        "request.json",
        "--apply",
        "--json",
    ]);
    fs::set_permissions(&path, permissions).unwrap();
    let applied = success(output);
    assert_eq!(applied["applied"], true);
    assert_eq!(applied["edit"]["diff"], "");
    assert_eq!(
        applied["edit"]["sourceRevision"],
        applied["edit"]["resultingRevision"]
    );
    assert_eq!(files(&project.0), before);
}

#[test]
fn readonly_apply_failure_preserves_permissions_sources_and_leaves_no_staging_files() {
    let project = TempDir::new();
    project.write("main.ubi", "export fn answer() -> int { 1 }\n");
    project.edit_request("main.ubi", "answer", "{ 2 }");
    let path = project.0.join("main.ubi");
    let permissions = fs::metadata(&path).unwrap().permissions();
    let mut readonly = permissions.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let before = files(&project.0);
    let output = project.run(&[
        "edit",
        "main.ubi",
        "--request",
        "request.json",
        "--apply",
        "--json",
    ]);
    let stayed_readonly = fs::metadata(&path).unwrap().permissions().readonly();
    fs::set_permissions(&path, permissions).unwrap();
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(stayed_readonly);
    assert_eq!(files(&project.0), before);
}

#[test]
fn invalid_cli_requests_and_usage_leave_sources_untouched() {
    let project = TempDir::new();
    project.write("main.ubi", "export fn answer() -> int { 1 }");
    project.write("unreachable.ubi", "export fn answer() -> int { 2 }");
    project.edit_request("main.ubi", "answer", "{ 3 }");
    let valid = fs::read(project.0.join("request.json")).unwrap();
    let value: Value = serde_json::from_slice(&valid).unwrap();
    let mut cases = vec![b"not JSON".to_vec(), b"{\"body\":\"\\ud800\"}".to_vec()];
    for (key, bad) in [
        ("unexpected", json!(true)),
        ("schemaVersion", json!(2)),
        ("sourceId", json!("unreachable.ubi")),
        ("sourceRevision", json!("bad")),
        ("body", json!("{ 2 } export fn injected() -> int { 3 }")),
    ] {
        let mut modified = value.clone();
        modified[key] = bad;
        cases.push(serde_json::to_vec(&modified).unwrap());
    }
    cases.push(vec![b' '; 1_048_577]);
    for bytes in cases {
        project.write("request.json", bytes);
        let before = files(&project.0);
        let output = project.run(&[
            "edit",
            "main.ubi",
            "--request",
            "request.json",
            "--apply",
            "--json",
        ]);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        assert_eq!(files(&project.0), before);
    }
    project.write("request.json", valid);
    for arguments in [
        vec!["edit", "main.ubi"],
        vec!["edit", "main.ubi", "--request"],
        vec![
            "edit",
            "main.ubi",
            "--request",
            "request.json",
            "--request",
            "request.json",
        ],
        vec![
            "edit",
            "main.ubi",
            "--request",
            "request.json",
            "--apply",
            "--apply",
        ],
        vec!["edit", "../outside.ubi", "--request", "request.json"],
        vec![
            "edit",
            "main.ubi",
            "--request",
            "request.json",
            "--out-dir",
            "out",
        ],
        vec!["edit", "main.ubi", "--request", "."],
        vec!["check", "main.ubi", "--apply"],
        vec!["check", "main.ubi", "--request", "request.json"],
    ] {
        let before = files(&project.0);
        let output = project.run(&arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}: {output:?}");
        assert!(output.stdout.is_empty());
        assert_eq!(files(&project.0), before);
    }
}

#[test]
fn library_body_source_and_project_budgets_reject_before_any_mutation() {
    let checked = compiler(&[("main.ubi", "export fn answer() -> int { 1 }")]);
    let huge_body = format!("{{ /*{}*/ 2 }}", "x".repeat(1_048_576));
    assert!(matches!(
        checked.prepare_function_body_edit(&request(&checked, "main.ubi", "answer", &huge_body)),
        Err(EditError::Request(_))
    ));
    let nearly_full = format!(
        "//{}\nexport fn answer() -> int {{ 1 }}",
        "x".repeat(1_048_500)
    );
    let checked = compiler(&[("main.ubi", &nearly_full)]);
    let growing_body = format!("{{ /*{}*/ 2 }}", "x".repeat(200));
    assert!(matches!(
        checked.prepare_function_body_edit(&request(&checked, "main.ubi", "answer", &growing_body)),
        Err(EditError::Request(_))
    ));
    let mut checked = Compiler::new();
    for (name, size) in [
        ("main.ubi", 500_000),
        ("a.ubi", 900_000),
        ("b.ubi", 900_000),
        ("c.ubi", 900_000),
        ("d.ubi", 900_000),
    ] {
        checked
            .add_source(
                name,
                format!("//{}\nexport fn answer() -> int {{ 1 }}", "x".repeat(size)).into_bytes(),
            )
            .unwrap();
    }
    let before = checked.source_revisions();
    let growing_body = format!("{{ /*{}*/ 2 }}", "x".repeat(100_000));
    assert!(matches!(
        checked.prepare_function_body_edit(&request(&checked, "main.ubi", "answer", &growing_body)),
        Err(EditError::Request(_))
    ));
    assert_eq!(checked.source_revisions(), before);
}

#[test]
fn validation_does_not_execute_the_proposed_body() {
    let checked = compiler(&[("main.ubi", "export fn answer() -> int { 1 }")]);
    let prepared = checked
        .prepare_function_body_edit(&request(&checked, "main.ubi", "answer", "{ 1 / 0 }"))
        .unwrap();
    assert!(prepared.updated_source().contains("{ 1 / 0 }"));
}

#[test]
fn parser_nesting_budget_uses_candidate_diagnostics_and_cli_exit_one() {
    fn nested_body(depth: usize) -> String {
        format!(
            "{{ {}1{} }}",
            "if (true) { ".repeat(depth),
            " } else { 1 }".repeat(depth)
        )
    }
    let original = "export fn answer() -> int { 1 }";
    let shallow = original.replace("{ 1 }", &nested_body(4));
    assert!(compiler(&[("main.ubi", &shallow)])
        .check()
        .diagnostics
        .is_empty());
    let body = nested_body(40);
    let candidate = original.replace("{ 1 }", &body);
    let expected = compiler(&[("main.ubi", &candidate)]);
    assert!(expected
        .check()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "UBI0090"));
    let original_compiler = compiler(&[("main.ubi", original)]);
    match original_compiler
        .prepare_function_body_edit(&request(&original_compiler, "main.ubi", "answer", &body))
        .unwrap_err()
    {
        EditError::Diagnostics {
            sources,
            diagnostics,
        } => {
            assert_eq!(sources, expected.source_revisions());
            assert_eq!(diagnostics, expected.check().diagnostics);
        }
        other => panic!("parser resource failures require candidate diagnostics: {other:?}"),
    }
    let project = TempDir::new();
    project.write("main.ubi", original);
    project.edit_request("main.ubi", "answer", &body);
    let before = files(&project.0);
    let output = project.run(&[
        "edit",
        "main.ubi",
        "--request",
        "request.json",
        "--apply",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        result["sources"][0]["revision"],
        expected.source_revisions()[0].revision
    );
    assert!(result["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "UBI0090"));
    assert!(result.get("edit").is_none());
    assert_eq!(files(&project.0), before);
}

#[test]
fn cli_rejects_outside_root_symlink_target_without_changing_external_data() {
    let project = TempDir::new();
    let external = TempDir::new();
    let source = "export fn answer() -> int { 1 }";
    external.write("outside.ubi", source);
    let link = project.0.join("main.ubi");
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(external.0.join("outside.ubi"), &link);
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_file(external.0.join("outside.ubi"), &link);
    if let Err(error) = linked {
        #[cfg(windows)]
        if error.raw_os_error() == Some(1314) {
            eprintln!("symlink contract unavailable: Windows privilege 1314");
            return;
        }
        panic!("cannot create symlink fixture: {error}");
    }
    let checked = compiler(&[("main.ubi", source)]);
    project.write(
        "request.json",
        serde_json::to_vec(&request_json(&request(
            &checked, "main.ubi", "answer", "{ 42 }",
        )))
        .unwrap(),
    );
    let output = project.run(&[
        "edit",
        "main.ubi",
        "--request",
        "request.json",
        "--apply",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty());
    assert_eq!(
        fs::read(external.0.join("outside.ubi")).unwrap(),
        source.as_bytes()
    );
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
}
