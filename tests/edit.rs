use serde_json::{json, Value};
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
