use crate::analyzer::check;
use crate::source::{SourceFile, SourceSet};

fn sources(items: &[(&str, &[u8])]) -> SourceSet {
    let mut sources = SourceSet::default();
    for (id, bytes) in items {
        sources
            .insert(SourceFile::new(id, bytes.to_vec()).unwrap())
            .unwrap();
    }
    sources
}

fn codes(diagnostics: &[crate::diagnostics::Diagnostic]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

#[test]
fn checks_public_core_modules_and_private_return_inference() {
    let project = sources(&[
        (
            "core/arithmetic.ubi",
            include_bytes!("../examples/core/arithmetic.ubi"),
        ),
        (
            "core/bindings.ubi",
            include_bytes!("../examples/core/bindings.ubi"),
        ),
        (
            "core/control.ubi",
            include_bytes!("../examples/core/control.ubi"),
        ),
        (
            "core/floats.ubi",
            include_bytes!("../examples/core/floats.ubi"),
        ),
        (
            "core/short-circuit.ubi",
            include_bytes!("../examples/core/short-circuit.ubi"),
        ),
        (
            "modules/app/main.ubi",
            include_bytes!("../examples/modules/app/main.ubi"),
        ),
        (
            "modules/lib/math.ubi",
            include_bytes!("../examples/modules/lib/math.ubi"),
        ),
        (
            "faults/overflow.ubi",
            include_bytes!("../examples/faults/overflow.ubi"),
        ),
        (
            "faults/zero-divisor.ubi",
            include_bytes!("../examples/faults/zero-divisor.ubi"),
        ),
        (
            "faults/evaluation-order.ubi",
            include_bytes!("../examples/faults/evaluation-order.ubi"),
        ),
    ]);
    let diagnostics = check(&project);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn reports_required_name_type_binding_and_arity_diagnostics() {
    let cases = [
        (
            "unknown.ubi",
            include_bytes!("../examples/invalid/unknown-unicode.ubi").as_slice(),
            "UBI0010",
            "missing",
        ),
        (
            "mixed.ubi",
            include_bytes!("../examples/invalid/mixed-numeric.ubi").as_slice(),
            "UBI0020",
            "2.0",
        ),
        (
            "immutable.ubi",
            include_bytes!("../examples/invalid/immutable-binding.ubi").as_slice(),
            "UBI0022",
            "value",
        ),
        (
            "duplicate.ubi",
            include_bytes!("../examples/invalid/duplicate-binding.ubi").as_slice(),
            "UBI0011",
            "value",
        ),
        (
            "annotation.ubi",
            include_bytes!("../examples/invalid/return-annotation.ubi").as_slice(),
            "UBI0021",
            "run",
        ),
        (
            "arity.ubi",
            include_bytes!("../examples/invalid/call-arity.ubi").as_slice(),
            "UBI0023",
            "identity()",
        ),
        (
            "range.ubi",
            include_bytes!("../examples/invalid/integer-literal.ubi").as_slice(),
            "UBI0004",
            "2147483648",
        ),
    ];

    for (id, bytes, required_code, snippet) in cases {
        let project = sources(&[(id, bytes)]);
        let diagnostics = check(&project);
        assert!(
            codes(&diagnostics).contains(&required_code),
            "{id}: {diagnostics:#?}"
        );
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == required_code
                    && std::str::from_utf8(bytes)
                        .unwrap()
                        .get(diagnostic.primary.start..diagnostic.primary.end)
                        == Some(snippet)
            }),
            "{id}: wrong primary span for {required_code}: {diagnostics:#?}"
        );
    }
}

#[test]
fn resolves_import_errors_at_the_import_token_and_reports_cycles() {
    let missing = sources(&[
        (
            "invalid/missing-export/main.ubi",
            include_bytes!("../examples/invalid/missing-export/main.ubi"),
        ),
        (
            "invalid/missing-export/library.ubi",
            include_bytes!("../examples/invalid/missing-export/library.ubi"),
        ),
    ]);
    let diagnostics = check(&missing);
    let missing_export = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "UBI0012")
        .unwrap();
    let missing_main = std::str::from_utf8(include_bytes!(
        "../examples/invalid/missing-export/main.ubi"
    ))
    .unwrap();
    assert_eq!(
        &missing_main[missing_export.primary.start..missing_export.primary.end],
        "secret"
    );

    let absolute = sources(&[(
        "invalid/absolute-import.ubi",
        include_bytes!("../examples/invalid/absolute-import.ubi"),
    )]);
    let diagnostics = check(&absolute);
    let invalid_path = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "UBI0012")
        .unwrap();
    let absolute_source =
        std::str::from_utf8(include_bytes!("../examples/invalid/absolute-import.ubi")).unwrap();
    assert_eq!(
        &absolute_source[invalid_path.primary.start..invalid_path.primary.end],
        "\"/library.ubi\""
    );

    let cycle = sources(&[
        (
            "invalid/cycle/a.ubi",
            include_bytes!("../examples/invalid/cycle/a.ubi"),
        ),
        (
            "invalid/cycle/b.ubi",
            include_bytes!("../examples/invalid/cycle/b.ubi"),
        ),
    ]);
    let diagnostics = check(&cycle);
    let cycle_error = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "UBI0012")
        .unwrap();
    let first_edge =
        std::str::from_utf8(include_bytes!("../examples/invalid/cycle/a.ubi")).unwrap();
    assert_eq!(
        &first_edge[cycle_error.primary.start..cycle_error.primary.end],
        "\"./b.ubi\""
    );
    assert_eq!(cycle_error.related.len(), 1);
    let other_edge =
        std::str::from_utf8(include_bytes!("../examples/invalid/cycle/b.ubi")).unwrap();
    assert_eq!(
        &other_edge[cycle_error.related[0].span.start..cycle_error.related[0].span.end],
        "\"./a.ubi\""
    );
}

#[test]
fn diagnostics_sort_by_source_id_then_primary_span() {
    let project = sources(&[
        ("z/main.ubi", b"fn f() { later; first; }"),
        ("a/main.ubi", b"fn f() { missing; }"),
    ]);
    let diagnostics = check(&project);
    assert_eq!(diagnostics.len(), 3);
    assert_eq!(diagnostics[0].primary.source_id, "a/main.ubi");
    assert_eq!(diagnostics[1].primary.source_id, "z/main.ubi");
    assert!(diagnostics[1].primary.start < diagnostics[2].primary.start);
}

#[test]
fn duplicate_import_names_are_reported_even_when_the_first_path_is_invalid() {
    let main = b"import { item } from \"/bad.ubi\";\nimport { item } from \"./lib.ubi\";\nfn run() { item() }";
    let library = b"export fn item() -> int { 1 }";
    let project = sources(&[("main.ubi", main), ("lib.ubi", library)]);
    let diagnostics = check(&project);
    let duplicate = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "UBI0011")
        .expect("duplicate import name diagnostic");
    assert_eq!(
        &main[duplicate.primary.start..duplicate.primary.end],
        b"item"
    );
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "UBI0012"));
}

#[test]
fn mutual_recursion_requires_annotations_on_every_function() {
    let invalid = b"fn even(n: int) { odd(n) }\nfn odd(n: int) { even(n) }";
    let project = sources(&[("main.ubi", invalid)]);
    let diagnostics = check(&project);
    let missing: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "UBI0021")
        .collect();
    assert_eq!(missing.len(), 2);

    let valid = b"fn even(n: int) -> bool { if (n == 0) { true } else { odd(n - 1) } }\nfn odd(n: int) -> bool { if (n == 0) { false } else { even(n - 1) } }";
    let project = sources(&[("main.ubi", valid)]);
    assert!(check(&project).is_empty());
}

#[test]
fn numeric_literal_limits_preserve_direct_negative_spans() {
    let direct = b"export fn minimum() -> int { - /* gap */ 2147483648 }";
    assert!(check(&sources(&[("main.ubi", direct)])).is_empty());

    let grouped = b"export fn invalid() -> int { -(2147483648) }";
    let diagnostics = check(&sources(&[("main.ubi", grouped)]));
    let error = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "UBI0004")
        .unwrap();
    assert_eq!(
        &grouped[error.primary.start..error.primary.end],
        b"2147483648"
    );

    let negative_overflow = b"export fn invalid() -> int { - 2147483649 }";
    let diagnostics = check(&sources(&[("main.ubi", negative_overflow)]));
    let error = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "UBI0004")
        .unwrap();
    assert_eq!(
        &negative_overflow[error.primary.start..error.primary.end],
        b"- 2147483649"
    );

    let float_overflow = b"export fn invalid() -> float { -1e400 }";
    let diagnostics = check(&sources(&[("main.ubi", float_overflow)]));
    let error = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "UBI0004")
        .unwrap();
    assert_eq!(
        &float_overflow[error.primary.start..error.primary.end],
        b"-1e400"
    );
}

#[test]
fn invalid_source_encoding_is_a_lexical_diagnostic_before_semantics() {
    let project = sources(&[("bad.ubi", b"fn f() { missing; \xf0\x9f")]);
    let diagnostics = check(&project);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "UBI0001");
    assert_eq!(
        (diagnostics[0].primary.start, diagnostics[0].primary.end),
        (18, 20)
    );
}
