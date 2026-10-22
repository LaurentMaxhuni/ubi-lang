use serde_json::Value;
use std::collections::BTreeSet;

const ADDITIONS: &[&str] = &[
    "dialog",
    "alert-dialog",
    "drawer",
    "popover",
    "tooltip",
    "toast",
    "select",
    "combobox",
    "dropdown-menu",
    "context-menu",
    "command-palette",
    "pagination",
    "stepper",
    "timeline",
    "table",
    "spotlight-card",
    "tilt-card",
    "card-stack",
    "carousel",
    "dock",
    "marquee",
    "number-ticker",
    "text-reveal",
    "rotating-text",
    "shimmer-button",
    "border-beam",
    "aurora-background",
    "grid-background",
    "scroll-reveal",
    "animated-list",
];

#[test]
fn expanded_catalog_contains_thirty_real_source_components() {
    let catalog: Vec<Value> = serde_json::from_str(include_str!("../ubi-ui/catalog.json")).unwrap();
    assert_eq!(catalog.len(), 70);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ubi-ui");
    let slugs: BTreeSet<_> = catalog
        .iter()
        .map(|entry| entry["slug"].as_str().unwrap())
        .collect();
    assert_eq!(slugs.len(), catalog.len(), "duplicate component slugs");
    for slug in ADDITIONS {
        let entry = catalog
            .iter()
            .find(|entry| entry["slug"] == *slug)
            .unwrap_or_else(|| panic!("missing component {slug}"));
        assert_eq!(
            entry["module"],
            format!("packages/ubi-ui/components/{slug}.ubi")
        );
        assert_eq!(
            entry["examples"][0]["source"],
            format!("examples/components/{slug}.ubi")
        );
        for file in [
            entry["module"].as_str().unwrap(),
            entry["examples"][0]["source"].as_str().unwrap(),
        ] {
            assert!(
                root.join(file).is_file(),
                "missing source for {slug}: {file}"
            );
        }
    }
}
