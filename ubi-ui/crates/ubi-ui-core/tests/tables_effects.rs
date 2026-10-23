use serde_json::json;
use ubi_ui_core::View;

#[test]
fn accepts_table_header_rows_without_row_keys_and_marks_sortable_headers_actionable() {
    let view = View::decode(json!({
        "schemaVersion": 1,
        "root": "root",
        "nodes": [
            {"key":"root","kind":{"tag":"Table","values":[]},"props":[{"tag":"Label","values":["People"]}],"children":["headers","row"]},
            {"key":"headers","kind":{"tag":"TableRow","values":[]},"props":[],"children":["name-header","age-header"]},
            {"key":"name-header","kind":{"tag":"TableHeader","values":[]},"props":[{"tag":"Option","values":["name"]},{"tag":"Label","values":["Name"]},{"tag":"Change","values":[{"tag":"Sort","values":[]}]}],"children":[]},
            {"key":"age-header","kind":{"tag":"TableHeader","values":[]},"props":[{"tag":"Option","values":["age"]},{"tag":"Label","values":["Age"]},{"tag":"Disabled","values":[true]}],"children":[]},
            {"key":"row","kind":{"tag":"TableRow","values":[]},"props":[{"tag":"Option","values":["person-1"]}],"children":["name-cell","age-cell"]},
            {"key":"name-cell","kind":{"tag":"TableCell","values":[]},"props":[{"tag":"Option","values":["name"]},{"tag":"Content","values":["Ada"]}],"children":[]},
            {"key":"age-cell","kind":{"tag":"TableCell","values":[]},"props":[{"tag":"Option","values":["age"]},{"tag":"Content","values":["37"]}],"children":[]}
        ]
    }))
    .expect("valid table contract should decode");
    assert!(view.nodes["name-header"].focusable());
    assert!(!view.nodes["age-header"].focusable());
    assert_eq!(view.nodes["name-header"].semantic_role(), "columnheader");
}

#[test]
fn rejects_table_column_mismatch_and_duplicate_option_values() {
    let invalid = [
        json!([
            {"key":"root","kind":{"tag":"Table","values":[]},"props":[],"children":["headers","row"]},
            {"key":"headers","kind":{"tag":"TableRow","values":[]},"props":[],"children":["h"]},
            {"key":"h","kind":{"tag":"TableHeader","values":[]},"props":[{"tag":"Option","values":["a"]},{"tag":"Label","values":["A"]}],"children":[]},
            {"key":"row","kind":{"tag":"TableRow","values":[]},"props":[{"tag":"Option","values":["r"]}],"children":["cell"]},
            {"key":"cell","kind":{"tag":"TableCell","values":[]},"props":[{"tag":"Option","values":["b"]}],"children":[]}
        ]),
        json!([
            {"key":"root","kind":{"tag":"Table","values":[]},"props":[],"children":["headers"]},
            {"key":"headers","kind":{"tag":"TableRow","values":[]},"props":[],"children":["h1","h2"]},
            {"key":"h1","kind":{"tag":"TableHeader","values":[]},"props":[{"tag":"Option","values":["same"]},{"tag":"Label","values":["A"]}],"children":[]},
            {"key":"h2","kind":{"tag":"TableHeader","values":[]},"props":[{"tag":"Option","values":["same"]},{"tag":"Label","values":["B"]}],"children":[]}
        ]),
    ];
    for nodes in invalid {
        assert!(View::decode(json!({"schemaVersion":1,"root":"root","nodes":nodes})).is_err());
    }
}

#[test]
fn validates_effect_property_pairs_and_index_ranges() {
    let view = |props, children| {
        View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
            {"key":"root","kind":{"tag":"EffectSurface","values":[]},"props":props,"children":children}
        ]}))
    };
    assert!(view(json!([{ "tag":"Effect", "values":[{"tag":"CardStack","values":[]}] }, {"tag":"Index","values":[1]}]), json!([])).is_err());
    assert!(view(json!([{ "tag":"Effect", "values":[{"tag":"NumberTicker","values":[]}] }, {"tag":"Number","values":[1]}]), json!([])).is_ok());
    assert!(view(
        json!([{ "tag":"Effect", "values":[{"tag":"Shimmer","values":[]}] }]),
        json!([])
    )
    .is_err());
}
