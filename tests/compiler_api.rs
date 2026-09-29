use ubi_lang::Compiler;

#[test]
fn check_exposes_sorted_source_revisions_and_import_requests() {
    let mut compiler = Compiler::new();
    compiler
        .add_source(
            "app/main.ubi",
            b"import { base } from \"../lib/math.ubi\"; export fn answer() -> int { base() + 1 }"
                .to_vec(),
        )
        .unwrap();
    compiler
        .add_source("lib/math.ubi", b"export fn base() -> int { 41 }".to_vec())
        .unwrap();

    let check = compiler.check();

    assert!(check.diagnostics.is_empty(), "{:?}", check.diagnostics);
    assert_eq!(
        compiler
            .source_revisions()
            .iter()
            .map(|source| source.id.as_str())
            .collect::<Vec<_>>(),
        ["app/main.ubi", "lib/math.ubi"]
    );
    assert_eq!(check.imports.len(), 1);
    assert_eq!(check.imports[0].importer_id, "app/main.ubi");
    assert_eq!(check.imports[0].path, "../lib/math.ubi");
    assert_eq!(check.imports[0].target_id.as_deref(), Some("lib/math.ubi"));

    let build = compiler.build();
    assert!(build.diagnostics.is_empty(), "{:?}", build.diagnostics);
    let modules = build.javascript.expect("valid program builds");
    assert!(modules.contains_key("app/main.mjs"));
    assert!(modules.contains_key("lib/math.mjs"));
    assert!(modules["app/main.mjs"].contains("../lib/math.mjs"));
}

#[test]
fn build_suppresses_artifacts_when_compilation_has_errors() {
    let mut compiler = Compiler::new();
    compiler
        .add_source(
            "main.ubi",
            b"export fn answer() -> int { missing }".to_vec(),
        )
        .unwrap();

    let build = compiler.build();

    assert!(build.javascript.is_none());
    assert_eq!(build.diagnostics.len(), 1);
    assert_eq!(build.diagnostics[0].code, "UBI0010");
    assert_eq!(build.diagnostics[0].primary.source_id, "main.ubi");
}

#[test]
fn compiler_reports_source_resource_limits_as_diagnostics() {
    let mut compiler = Compiler::new();

    let error = compiler
        .add_source("large.ubi", vec![b' '; 1_048_577])
        .unwrap_err();

    match error {
        ubi_lang::SourceError::Diagnostic(diagnostic) => {
            assert_eq!(diagnostic.code, "UBI0090");
            assert_eq!(diagnostic.primary.source_id, "large.ubi");
        }
        other => panic!("unexpected source error: {other:?}"),
    }
}
