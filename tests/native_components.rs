#![cfg(feature = "native-ui")]
use serde_json::{json, Value};
use ubi_lang::ui::NativeApplication;
use ubi_ui_platform::{Application, Event, Input, UiKey};

#[path = "../ubi-ui/tools/sources.rs"]
mod sources;

fn example(slug: &str) -> NativeApplication {
    let entry = format!("examples/components/{slug}.ubi");
    let compiler = sources::compiler(&entry).unwrap();
    NativeApplication::new(compiler.runtime().unwrap(), entry).unwrap()
}

fn event(key: &str, tag: &str, message: &str, value: Option<Value>) -> Event {
    let mut values = vec![json!({"tag": message, "values": []})];
    values.extend(value);
    Event {
        key: key.into(),
        value: json!({"tag": tag, "values": values}),
    }
}

#[test]
fn every_catalog_entry_compiles_and_mounts_its_individual_source_example() {
    let catalog: Vec<Value> = serde_json::from_str(include_str!("../ubi-ui/catalog.json")).unwrap();
    assert!(
        catalog.len() >= 40,
        "original component catalog must remain available"
    );
    let mut slugs = std::collections::BTreeSet::new();
    for item in catalog {
        let slug = item["slug"].as_str().unwrap();
        assert!(slugs.insert(slug.to_owned()), "duplicate {slug}");
        let module = item["module"].as_str().unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ubi-ui");
        assert!(root.join(module).is_file(), "{slug}: missing source");
        let source = item["examples"][0]["source"].as_str().unwrap();
        assert!(root.join(source).is_file(), "{slug}: missing example");
        let mut compiler = sources::compiler(source).unwrap();
        let original = std::fs::read_to_string(root.join(source)).unwrap();
        let function = item["function"].as_str().unwrap();
        let symbol = if function == "view" {
            "viewComponent"
        } else {
            function
        };
        let direct_import = format!("from \"../../{module}\"");
        let mut facade_lines = Vec::new();
        for line in original.lines() {
            if line.contains(&direct_import) {
                let symbols = line
                    .strip_prefix("import { ")
                    .unwrap()
                    .split(" } from")
                    .next()
                    .unwrap();
                let remaining: Vec<_> = symbols.split(", ").filter(|s| *s != symbol).collect();
                if !remaining.is_empty() {
                    facade_lines.push(line.replace(symbols, &remaining.join(", ")));
                }
                facade_lines.push(format!(
                    "import {{ {function} }} from \"../../packages/ubi-ui/components.ubi\";"
                ));
            } else {
                facade_lines.push(line.replace("export fn view(", "export fn render("));
            }
        }
        let mut facade_source = facade_lines.join("\n");
        if function == "view" {
            facade_source = facade_source.replace("viewComponent(", "view(");
        }
        let facade_entry = format!("examples/components/facade-{slug}.ubi");
        compiler.add_source(&facade_entry, facade_source).unwrap();
        let runtime = compiler
            .runtime()
            .unwrap_or_else(|error| panic!("{slug}: {error:?}"));
        let model = runtime.invoke(source, "init", &[]).unwrap();
        let direct = runtime
            .invoke(source, "view", std::slice::from_ref(&model))
            .unwrap();
        let facade = runtime.invoke(&facade_entry, "render", &[model]).unwrap();
        assert_eq!(direct, facade, "{slug}: facade changed public behavior");
        let app = NativeApplication::new(runtime, source)
            .unwrap_or_else(|error| panic!("{slug}: {error}"));
        assert_eq!(app.view().root, "app");
        assert!(app.view().nodes.len() >= 3, "{slug}");
    }
}

#[test]
fn source_selection_components_round_trip_controlled_state() {
    for slug in ["radio-group", "segmented-control", "tabs"] {
        let mut app = example(slug);
        let choice_prefix = if slug == "tabs" { "demo-tab-" } else { "demo-" };
        let day = format!("{choice_prefix}day");
        let week = format!("{choice_prefix}week");
        let year = format!("{choice_prefix}year");
        assert!(app.view().nodes[&day].checked, "{slug}");
        app.dispatch(&event(&week, "Select", "Select", Some(json!("week"))))
            .unwrap();
        assert_eq!(app.model(), &json!("week"), "{slug}");
        assert!(app.view().nodes[&week].checked, "{slug}");
        assert!(!app.view().nodes[&day].checked, "{slug}");
        assert!(app
            .dispatch(&event(&year, "Select", "Select", Some(json!("year"))))
            .is_err());
        if slug == "tabs" {
            assert_eq!(
                app.view().nodes["active-panel"].content,
                "Active digest: week"
            );
            assert_eq!(app.view().nodes["demo-list"].role, "tablist");
            assert_eq!(app.view().nodes["demo-panel-week"].role, "tabpanel");
            assert_eq!(app.view().nodes["demo-panel-week"].group, "demo");
        }
    }
    let mut app = example("toggle-group");
    app.dispatch(&event("demo-week", "Select", "Select", Some(json!("week"))))
        .unwrap();
    assert_eq!(app.model(), &json!(["day", "week"]));
    app.dispatch(&event("demo-day", "Select", "Select", Some(json!("day"))))
        .unwrap();
    assert_eq!(app.model(), &json!(["week"]));
    assert!(!app.view().nodes["demo-day"].checked);
}

#[test]
fn keyboard_selection_skips_disabled_choices_and_restores_group_tab_stop() {
    for slug in ["radio-group", "tabs"] {
        let mut app = example(slug);
        let choice_prefix = if slug == "tabs" { "demo-tab-" } else { "demo-" };
        let day = format!("{choice_prefix}day");
        let week = format!("{choice_prefix}week");
        let year = format!("{choice_prefix}year");
        let mut input = Input::default();
        input.key(app.view(), UiKey::Tab, true, false);
        assert_eq!(input.focused.as_deref(), Some(day.as_str()), "{slug}");
        let change = input.key(app.view(), UiKey::Right, true, false).unwrap();
        app.dispatch(&change).unwrap();
        assert_eq!(input.focused.as_deref(), Some(week.as_str()));
        assert!(app.view().nodes[&week].checked);
        let change = input.key(app.view(), UiKey::Right, true, false).unwrap();
        app.dispatch(&change).unwrap();
        assert_eq!(input.focused.as_deref(), Some(day.as_str()));
        assert!(app.view().nodes[&year].disabled);
    }
    for slug in ["segmented-control", "toggle-group"] {
        let mut app = example(slug);
        let initial = app.model().clone();
        let mut input = Input::default();
        input.key(app.view(), UiKey::Tab, true, false);
        assert_eq!(input.focused.as_deref(), Some("demo-day"), "{slug}");
        for (key, target) in [
            (UiKey::Right, "demo-week"),
            (UiKey::Right, "demo-day"),
            (UiKey::End, "demo-week"),
            (UiKey::Home, "demo-day"),
            (UiKey::Right, "demo-week"),
        ] {
            assert!(input.key(app.view(), key, true, false).is_none());
            assert_eq!(input.focused.as_deref(), Some(target), "{slug}");
            assert_eq!(
                app.model(),
                &initial,
                "{slug}: navigation changed selection"
            );
            assert!(!app.view().nodes["demo-week"].checked);
        }
        assert!(input.key(app.view(), UiKey::Space, true, false).is_none());
        let change = input.key(app.view(), UiKey::Space, false, false).unwrap();
        app.dispatch(&change).unwrap();
        assert!(app.view().nodes["demo-week"].checked, "{slug}");
        if slug == "toggle-group" {
            assert_eq!(app.model(), &json!(["day", "week"]));
            assert!(input.key(app.view(), UiKey::Enter, true, false).is_none());
            let change = input.key(app.view(), UiKey::Enter, false, false).unwrap();
            app.dispatch(&change).unwrap();
            assert_eq!(app.model(), &json!(["day"]));
            assert!(!app.view().nodes["demo-week"].checked);
        } else {
            assert_eq!(app.model(), &json!("week"));
        }
        assert!(app.view().nodes["demo-year"].disabled);
    }
}

#[test]
fn disclosures_keep_closed_containers_and_publish_expanded_state() {
    for slug in ["accordion", "collapsible"] {
        let mut app = example(slug);
        assert!(app.view().nodes.contains_key("answer"));
        assert_eq!(app.view().nodes["demo-content"].disclosure, Some(true));
        assert_eq!(app.view().nodes["demo-trigger"].expanded, Some(true));
        app.dispatch(&event(
            "demo-trigger",
            "Toggle",
            "Toggle",
            Some(json!(false)),
        ))
        .unwrap();
        assert_eq!(app.model(), &json!(false));
        assert!(!app.view().nodes.contains_key("answer"));
        assert_eq!(app.view().nodes["demo-content"].disclosure, Some(false));
        assert_eq!(app.view().nodes["demo-trigger"].expanded, Some(false));
        app.dispatch(&event(
            "demo-trigger",
            "Toggle",
            "Toggle",
            Some(json!(true)),
        ))
        .unwrap();
        assert!(app.view().nodes.contains_key("answer"));
        assert_eq!(app.view().nodes["demo-content"].disclosure, Some(true));
    }
}

#[test]
fn native_keyboard_steps_numeric_values_and_edits_multiline_source_models() {
    let mut app = example("number-input");
    let mut input = Input::default();
    input.focused = Some("demo".into());
    let change = input.key(app.view(), UiKey::Up, true, false).unwrap();
    app.dispatch(&change).unwrap();
    assert_eq!(app.model(), &json!("14"));
    app.dispatch(&event("demo", "Input", "Input", Some(json!("100"))))
        .unwrap();
    let change = input.key(app.view(), UiKey::Up, true, false).unwrap();
    app.dispatch(&change).unwrap();
    assert_eq!(app.model(), &json!("100"));

    let mut app = example("textarea");
    input.focused = Some("demo".into());
    input.caret = 0;
    let change = input.key(app.view(), UiKey::Enter, true, false).unwrap();
    app.dispatch(&change).unwrap();
    assert!(app.model().as_str().unwrap().starts_with('\n'));
    let change = input.text(app.view(), "Hello").unwrap();
    app.dispatch(&change).unwrap();
    assert!(app.model().as_str().unwrap().starts_with("\nHello"));
}

#[test]
fn new_leaf_controls_and_composite_children_use_real_source_reducers() {
    let mut app = example("toggle");
    app.dispatch(&event("demo", "Toggle", "Toggle", Some(json!(true))))
        .unwrap();
    assert_eq!(app.model(), &json!(true));
    assert!(app.view().nodes["demo"].checked);
    let mut app = example("link");
    app.dispatch(&event("demo", "Activate", "Action", None))
        .unwrap();
    assert_eq!(app.model(), &json!(1));
    assert_eq!(app.view().nodes["demo"].content, "Open project / 1");
    for (slug, key) in [("search-input", "demo"), ("field", "workspace")] {
        let mut app = example(slug);
        app.dispatch(&event(key, "Input", "Input", Some(json!("Orbit team"))))
            .unwrap();
        assert_eq!(app.model(), &json!("Orbit team"));
        assert_eq!(app.view().nodes[key].value, "Orbit team");
    }
}
