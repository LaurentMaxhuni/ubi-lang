use serde_json::{json, Value};

#[path = "../ubi-ui/tools/sources.rs"]
mod sources;

const EMPTY_TABLE_ENTRY: &str = "examples/components/source-contract-table.ubi";
const EMPTY_TABLE_SOURCE: &str = r#"
import { View, Event, TableColumn } from "../../packages/ubi-ui/core.ubi";
import { table } from "../../packages/ubi-ui/components/table.ubi";
export enum Message { Sort }
export fn init() -> int { 0 }
export fn update(model: int, event: Event<Message>) -> int { model }
export fn view(model: int) -> View<Message> {
    table("items", "Items", [
        TableColumn { key: "name", label: "Name", sortable: false },
        TableColumn { key: "status", label: "Status", sortable: false },
        TableColumn { key: "owner", label: "Owner", sortable: false }
    ], [], Message.Sort, [])
}
"#;

fn runtime(entry: &str) -> (ubi_lang::Runtime, Value) {
    let compiler = sources::compiler(entry).unwrap();
    let runtime = compiler.runtime().unwrap();
    let model = runtime.invoke(entry, "init", &[]).unwrap();
    (runtime, model)
}

fn view(runtime: &ubi_lang::Runtime, entry: &str, model: &Value) -> Value {
    runtime
        .invoke(entry, "view", std::slice::from_ref(model))
        .unwrap()
}

fn node<'a>(view: &'a Value, key: &str) -> &'a Value {
    view["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["key"] == key)
        .unwrap_or_else(|| panic!("missing node {key}"))
}

fn prop(node: &Value, tag: &str) -> Value {
    node["props"]
        .as_array()
        .unwrap()
        .iter()
        .find(|prop| prop["tag"] == tag)
        .map(|prop| prop["values"][0].clone())
        .unwrap_or(Value::Null)
}

#[test]
fn tabs_reserve_wrapper_keys_and_key_panel_by_selection() {
    let entry = "examples/components/source-contract-tabs.ubi";
    let mut compiler = sources::compiler("examples/components/tabs.ubi").unwrap();
    compiler
        .add_source(
            entry,
            r#"
import { View, Event, Choice } from "../../packages/ubi-ui/core.ubi";
import { tabs } from "../../packages/ubi-ui/components/tabs.ubi";
export enum Message { Select }
export fn init() -> string { "list" }
export fn update(model: string, event: Event<Message>) -> string {
    match (event) { Event.Select(Message.Select, value) => value, _ => model }
}
export fn view(model: string) -> View<Message> {
    tabs("demo", "Reserved values", [
        Choice { value: "list", label: "List", disabled: false },
        Choice { value: "panel", label: "Panel", disabled: false }
    ], model, Message.Select, [], [])
}
"#,
        )
        .unwrap();
    let runtime = compiler.runtime().unwrap();
    let model = runtime.invoke(entry, "init", &[]).unwrap();
    let first = view(&runtime, entry, &model);
    let keys: Vec<_> = first["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["key"].as_str().unwrap())
        .collect();
    assert_eq!(
        keys.len(),
        keys.iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    );
    assert!(keys.contains(&"demo-tab-list"));
    assert!(keys.contains(&"demo-tab-panel"));
    assert!(keys.contains(&"demo-panel-list"));
    assert_eq!(
        prop(node(&first, "demo-panel-list"), "Group"),
        json!("demo")
    );

    let selected = runtime
        .invoke(
            entry,
            "update",
            &[
                model,
                json!({"tag":"Select","values":[{"tag":"Select","values":[]},"panel"]}),
            ],
        )
        .unwrap();
    let second = view(&runtime, entry, &selected);
    assert!(second["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["key"] != "demo-panel-list"));
    assert_eq!(
        prop(node(&second, "demo-panel-panel"), "Group"),
        json!("demo")
    );
}

#[test]
fn timeline_staggers_entrances_with_a_bounded_delay() {
    let entry = "examples/components/source-contract-timeline.ubi";
    let children = (0..32)
        .map(|index| format!("textComponent(\"item-{index}\", \"Item {index}\")"))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        r#"
import {{ View }} from "../../packages/ubi-ui/core.ubi";
import {{ textComponent }} from "../../packages/ubi-ui/components/text.ubi";
import {{ timeline }} from "../../packages/ubi-ui/components/timeline.ubi";
export fn init() -> int {{ 0 }}
export fn view(model: int) -> View<int> {{ timeline("timeline", [], [{children}]) }}
"#
    );
    let mut compiler = sources::compiler("examples/components/timeline.ubi").unwrap();
    compiler.add_source(entry, source).unwrap();
    let runtime = compiler.runtime().unwrap();
    let model = runtime.invoke(entry, "init", &[]).unwrap();
    let tree = view(&runtime, entry, &model);
    for index in [0, 1, 16, 17, 31] {
        let item = node(&tree, &format!("timeline-item-item-{index}"));
        assert_eq!(prop(item, "Enter")["tag"], "Rise");
        let motion = prop(item, "Motion");
        assert_eq!(motion["durationMs"], 220);
        assert_eq!(motion["delayMs"], (index * 60).min(1000));
        assert_eq!(motion["easing"]["tag"], "EaseOut");
    }
}

#[test]
fn dock_has_a_named_navigation_role() {
    let entry = "examples/components/dock.ubi";
    let (runtime, model) = runtime(entry);
    let tree = view(&runtime, entry, &model);
    assert_eq!(
        prop(node(&tree, "workspace-dock"), "Role"),
        json!("navigation")
    );
}

#[test]
fn command_palette_search_sits_outside_its_labelled_menu() {
    let entry = "examples/components/command-palette.ubi";
    let (runtime, model) = runtime(entry);
    let tree = view(&runtime, entry, &model);
    let overlay_children = node(&tree, "commands")["children"].as_array().unwrap();
    assert!(overlay_children
        .iter()
        .any(|child| child == "commands-search"));
    assert!(overlay_children
        .iter()
        .any(|child| child == "commands-menu"));
    let menu = node(&tree, "commands-menu");
    assert_eq!(menu["kind"]["tag"], "Column");
    assert_eq!(prop(menu, "Role"), json!("menu"));
    assert_eq!(prop(menu, "Label"), json!("Command palette"));
    let menu_children = menu["children"].as_array().unwrap();
    assert!(menu_children
        .iter()
        .any(|child| child == "commands-item-settings"));
    assert!(menu_children
        .iter()
        .any(|child| child == "commands-item-help"));
    assert!(!menu_children.iter().any(|child| child == "commands-search"));
}

#[test]
fn empty_table_renders_a_full_width_readable_row() {
    let entry = EMPTY_TABLE_ENTRY;
    let mut compiler = sources::compiler("examples/components/table.ubi").unwrap();
    compiler.add_source(entry, EMPTY_TABLE_SOURCE).unwrap();
    let runtime = compiler.runtime().unwrap();
    let model = runtime.invoke(entry, "init", &[]).unwrap();
    let tree = view(&runtime, entry, &model);
    let rows = node(&tree, "items")["children"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "header row and readable empty row");
    let empty_row = node(&tree, "items-empty-row");
    assert_eq!(prop(empty_row, "Option"), json!("empty"));
    let cells = empty_row["children"].as_array().unwrap();
    assert_eq!(cells.len(), 3);
    let contents: Vec<_> = cells
        .iter()
        .map(|key| prop(node(&tree, key.as_str().unwrap()), "Content"))
        .collect();
    assert_eq!(contents, [json!("No results."), json!(""), json!("")]);
    let column_keys: Vec<_> = cells
        .iter()
        .map(|key| prop(node(&tree, key.as_str().unwrap()), "Option"))
        .collect();
    assert_eq!(
        column_keys,
        [json!("name"), json!("status"), json!("owner")]
    );
}

#[cfg(feature = "native-ui")]
#[test]
fn native_host_accepts_the_synthetic_empty_table_row() {
    use ubi_ui_platform::Application;

    let mut compiler = sources::compiler("examples/components/table.ubi").unwrap();
    compiler
        .add_source(EMPTY_TABLE_ENTRY, EMPTY_TABLE_SOURCE)
        .unwrap();
    let runtime = compiler.runtime().unwrap();
    let app = ubi_lang::ui::NativeApplication::new(runtime, EMPTY_TABLE_ENTRY).unwrap();
    let row = &app.view().nodes["items-empty-row"];
    assert_eq!(row.children.len(), 3);
}
