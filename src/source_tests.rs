use crate::source::{SourceError, SourceFile};

#[test]
fn source_keeps_exact_bytes_and_hashes_them() {
    let source = SourceFile::new("nested/main.ubi", b"fn f() {}\r\n".to_vec()).unwrap();

    assert_eq!(source.id(), "nested/main.ubi");
    assert_eq!(source.bytes(), b"fn f() {}\r\n");
    assert_eq!(source.text().unwrap(), "fn f() {}\r\n");
    assert_eq!(
        source.revision(),
        "sha256:ce83e95873fd22632754a494d6ca5a4be9299e8dbb9da20eb586b63469ba5af1"
    );
}

#[test]
fn source_hashes_inputs_crossing_sha256_blocks() {
    let source = SourceFile::new("main.ubi", vec![b'a'; 100]).unwrap();
    assert_eq!(
        source.revision(),
        "sha256:2816597888e4a0d3a36b82b83316ab32680eb8f00f8cd3b904d681246d285a0e"
    );
}

#[test]
fn source_reports_bom_and_malformed_utf8_as_lexical_errors() {
    let bom = SourceFile::new("bom.ubi", b"\xef\xbb\xbffn f() {}".to_vec()).unwrap();
    let error = bom.text().unwrap_err();
    assert_eq!(error.code, "UBI0001");
    assert_eq!((error.primary.start, error.primary.end), (0, 3));

    let malformed = SourceFile::new("bad.ubi", b"x\xf0\x9f".to_vec()).unwrap();
    let error = malformed.text().unwrap_err();
    assert_eq!(error.code, "UBI0001");
    assert_eq!((error.primary.start, error.primary.end), (1, 3));
}

#[test]
fn source_rejects_noncanonical_ids() {
    for id in [
        "",
        "/main.ubi",
        "../main.ubi",
        "a/../main.ubi",
        "a\\main.ubi",
        "main.txt",
    ] {
        assert!(SourceFile::new(id, Vec::new()).is_err(), "accepted {id:?}");
    }
}

#[test]
fn source_rejects_files_over_the_compilation_budget_before_hashing() {
    let error = SourceFile::new("large.ubi", vec![0; 1_048_577]).unwrap_err();
    match error {
        SourceError::Diagnostic(error) => {
            assert_eq!(error.code, "UBI0090");
            assert_eq!((error.primary.start, error.primary.end), (0, 0));
        }
        other => panic!("expected resource diagnostic, got {other:?}"),
    }
}

#[test]
fn source_set_rejects_duplicates_and_iterates_by_source_id() {
    let mut sources = crate::source::SourceSet::default();
    sources
        .insert(SourceFile::new("z.ubi", Vec::new()).unwrap())
        .unwrap();
    sources
        .insert(SourceFile::new("a.ubi", Vec::new()).unwrap())
        .unwrap();
    assert!(sources
        .insert(SourceFile::new("a.ubi", b"different".to_vec()).unwrap())
        .is_err());
    assert_eq!(
        sources.iter().map(SourceFile::id).collect::<Vec<_>>(),
        ["a.ubi", "z.ubi"]
    );
    assert!(sources.get("a.ubi").is_some());
}

#[test]
fn source_set_enforces_project_byte_and_module_budgets() {
    let mut bytes_limited = crate::source::SourceSet::default();
    for index in 0..4 {
        bytes_limited
            .insert(SourceFile::new(&format!("{index}.ubi"), vec![0; 1_048_576]).unwrap())
            .unwrap();
    }
    assert!(matches!(
        bytes_limited.insert(SourceFile::new("extra.ubi", b"x".to_vec()).unwrap()),
        Err(SourceError::Diagnostic(error)) if error.code == "UBI0090"
    ));

    let mut module_limited = crate::source::SourceSet::default();
    for index in 0..64 {
        module_limited
            .insert(SourceFile::new(&format!("{index}.ubi"), Vec::new()).unwrap())
            .unwrap();
    }
    assert!(matches!(
        module_limited.insert(SourceFile::new("extra.ubi", Vec::new()).unwrap()),
        Err(SourceError::Diagnostic(error)) if error.code == "UBI0090"
    ));
}
