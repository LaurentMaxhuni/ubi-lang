use serde_json::json;
use ubi_ui_core::View;
use ubi_ui_input::{Input, Key};
use ubi_ui_layout::{Layout, TextMeasure};

struct Text;
impl TextMeasure for Text {
    fn measure(&mut self, _: &str, _: f32, _: f32) -> (f32, f32) {
        (30.0, 20.0)
    }
}

#[test]
fn modal_focus_is_contained_and_escape_dispatches_controlled_dismissal() {
    let view = View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["outside","dialog"]},
        {"key":"outside","kind":{"tag":"Button","values":[]},"props":[{"tag":"Label","values":["Outside"]}],"children":[]},
        {"key":"dialog","kind":{"tag":"Overlay","values":[]},"props":[{"tag":"OverlayKind","values":[{"tag":"Dialog","values":[]}]},{"tag":"Label","values":["Settings"]},{"tag":"Open","values":[true]},{"tag":"Dismiss","values":[{"tag":"Close","values":[]}]}],"children":["first","second"]},
        {"key":"first","kind":{"tag":"Button","values":[]},"props":[{"tag":"Activate","values":[{"tag":"Save","values":[]}]}],"children":[]},
        {"key":"second","kind":{"tag":"Button","values":[]},"props":[{"tag":"Activate","values":[{"tag":"Cancel","values":[]}]}],"children":[]}
    ]})).unwrap();
    let scene = Layout::default()
        .compute(&view, 480.0, 320.0, &mut Text)
        .unwrap();
    let mut input = Input::default();
    input.sync(&view);
    assert_eq!(input.focused.as_deref(), Some("first"));
    assert_eq!(Input::hit(&view, &scene, 20.0, 80.0), None);
    input.key(&view, Key::Tab, true, false);
    assert_eq!(input.focused.as_deref(), Some("second"));
    let close = input
        .key(&view, Key::Escape, true, false)
        .expect("Escape dismisses Dialog");
    assert_eq!(close.key, "dialog");
    assert_eq!(close.value["tag"], "Activate");
    assert_eq!(close.value["values"][0]["tag"], "Close");
}

#[test]
fn alert_dialog_blocks_escape_and_outside_pointer_dismissal_requires_a_pair() {
    let view = View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["trigger","alert"]},
        {"key":"trigger","kind":{"tag":"Button","values":[]},"props":[{"tag":"Label","values":["Open"]}],"children":[]},
        {"key":"alert","kind":{"tag":"Overlay","values":[]},"props":[{"tag":"OverlayKind","values":[{"tag":"AlertDialog","values":[]}]},{"tag":"Label","values":["Delete item?"]},{"tag":"Open","values":[true]},{"tag":"Dismiss","values":[{"tag":"Cancel"}]}],"children":[]}
    ]})).unwrap();
    let scene = Layout::default()
        .compute(&view, 480.0, 320.0, &mut Text)
        .unwrap();
    let mut input = Input::default();
    input.sync(&view);
    assert!(input.key(&view, Key::Escape, true, false).is_none());
    input.pointer_move(&view, &scene, 10.0, 10.0);
    assert!(input.pointer_down(&view, &scene).is_none());
    assert!(input.pointer_up(&view, &scene).is_none());
}

#[test]
fn popover_dismissal_requires_both_pointers_outside_and_ignores_stale_openings() {
    let view = View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["trigger","popover"]},
        {"key":"trigger","kind":{"tag":"Button","values":[]},"props":[{"tag":"Label","values":["Open"]}],"children":[]},
        {"key":"popover","kind":{"tag":"Overlay","values":[]},"props":[{"tag":"OverlayKind","values":[{"tag":"Popover","values":[]}]},{"tag":"Label","values":["Actions"]},{"tag":"Anchor","values":["trigger"]},{"tag":"Open","values":[true]},{"tag":"Dismiss","values":[{"tag":"Close"}]}],"children":["inside"]},
        {"key":"inside","kind":{"tag":"Button","values":[]},"props":[{"tag":"Label","values":["Apply"]}],"children":[]}
    ]})).unwrap();
    let scene = Layout::default()
        .compute(&view, 480.0, 320.0, &mut Text)
        .unwrap();
    let inside = scene.boxes["inside"].bounds;
    let mut input = Input::default();
    input.sync(&view);
    input.pointer_move(&view, &scene, inside.x + 1.0, inside.y + 1.0);
    input.pointer_down(&view, &scene);
    input.pointer_move(&view, &scene, 470.0, 310.0);
    assert!(
        input.pointer_up(&view, &scene).is_none(),
        "inside-down outside-up does not dismiss"
    );

    input.pointer_move(&view, &scene, 470.0, 310.0);
    input.pointer_down(&view, &scene);
    let mut closed = view.clone();
    closed.nodes.get_mut("popover").unwrap().open = false;
    input.sync(&closed);
    let mut reopened = view.clone();
    reopened.nodes.get_mut("popover").unwrap().open = true;
    input.sync(&reopened);
    assert!(
        input.pointer_up(&reopened, &scene).is_none(),
        "old opening cannot dismiss its replacement"
    );

    input.pointer_move(&reopened, &scene, 470.0, 310.0);
    input.pointer_down(&reopened, &scene);
    let dismiss = input
        .pointer_up(&reopened, &scene)
        .expect("outside down-up dismisses once");
    assert_eq!(dismiss.key, "popover");
    assert_eq!(dismiss.value["values"][0]["tag"], "Close");
}

#[test]
fn sortable_table_headers_and_option_items_emit_select_events() {
    let view = View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["table","options"]},
        {"key":"table","kind":{"tag":"Table","values":[]},"props":[],"children":["headers"]},
        {"key":"headers","kind":{"tag":"TableRow","values":[]},"props":[],"children":["header"]},
        {"key":"header","kind":{"tag":"TableHeader","values":[]},"props":[{"tag":"Label","values":["Name"]},{"tag":"Option","values":["name"]},{"tag":"Change","values":[{"tag":"Sort"}]}],"children":[]},
        {"key":"options","kind":{"tag":"Row","values":[]},"props":[],"children":["option"]},
        {"key":"option","kind":{"tag":"OptionItem","values":[]},"props":[{"tag":"Label","values":["Ada"]},{"tag":"Option","values":["ada"]},{"tag":"Change","values":[{"tag":"Choose"}]}],"children":[]}
    ]})).unwrap();
    let mut input = Input::default();
    input.sync(&view);
    assert_eq!(input.focused, None);
    input.key(&view, Key::Tab, true, false);
    assert_eq!(input.focused.as_deref(), Some("header"));
    input.key(&view, Key::Enter, true, false);
    let sorted = input
        .key(&view, Key::Enter, false, false)
        .expect("sortable header activation");
    assert_eq!(sorted.value["tag"], "Select");
    assert_eq!(sorted.value["values"][1], "name");
    input.key(&view, Key::Tab, true, false);
    assert_eq!(input.focused.as_deref(), Some("option"));
    input.key(&view, Key::Space, true, false);
    let selected = input
        .key(&view, Key::Space, false, false)
        .expect("option selection");
    assert_eq!(selected.value["tag"], "Select");
    assert_eq!(selected.value["values"][1], "ada");
}

#[test]
fn carousel_hiding_focused_content_moves_focus_to_available_navigation() {
    let mut view = View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        {"key":"root","kind":{"tag":"Column","values":[]},"props":[],"children":["previous","slides","next"]},
        {"key":"previous","kind":{"tag":"Button","values":[]},"props":[{"tag":"Content","values":["Previous"]}],"children":[]},
        {"key":"slides","kind":{"tag":"EffectSurface","values":[]},"props":[{"tag":"Effect","values":[{"tag":"Carousel","values":[]}]},{"tag":"Index","values":[0]}],"children":["first","second"]},
        {"key":"first","kind":{"tag":"Button","values":[]},"props":[{"tag":"Content","values":["First"]}],"children":[]},
        {"key":"second","kind":{"tag":"Button","values":[]},"props":[{"tag":"Content","values":["Second"]}],"children":[]},
        {"key":"next","kind":{"tag":"Button","values":[]},"props":[{"tag":"Content","values":["Next"]}],"children":[]}
    ]})).unwrap();
    let mut layout = Layout::default();
    let scene = layout.compute(&view, 480.0, 320.0, &mut Text).unwrap();
    let first = scene.boxes["first"].bounds;
    let mut input = Input::default();
    input.pointer_move(
        &view,
        &scene,
        first.x + first.width * 0.5,
        first.y + first.height * 0.5,
    );
    input.pointer_down(&view, &scene);
    assert_eq!(input.focused.as_deref(), Some("first"));

    view.nodes.get_mut("slides").unwrap().index = 1;
    input.sync(&view);
    assert_eq!(input.focused.as_deref(), Some("previous"));
}

#[test]
fn context_menu_open_requests_resolve_pointer_and_keyboard_triggers() {
    let view = View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["trigger","menu"]},
        {"key":"trigger","kind":{"tag":"Button","values":[]},"props":[{"tag":"Label","values":["Project"]}],"children":[]},
        {"key":"menu","kind":{"tag":"Overlay","values":[]},"props":[{"tag":"OverlayKind","values":[{"tag":"ContextMenu","values":[]}]},{"tag":"Label","values":["Project actions"]},{"tag":"Anchor","values":["trigger"]},{"tag":"Open","values":[false]},{"tag":"OpenRequest","values":[{"tag":"Request","values":[]}]},{"tag":"Dismiss","values":[{"tag":"Dismiss","values":[]}]}],"children":[]}
    ]})).unwrap();
    let scene = Layout::default()
        .compute(&view, 480.0, 320.0, &mut Text)
        .unwrap();
    let trigger = scene.boxes["trigger"].bounds;
    let point = (
        trigger.x + trigger.width * 0.5,
        trigger.y + trigger.height * 0.5,
    );
    let mut input = Input::default();
    let (menu, pointer) = input
        .context_request_at(&view, &scene, point.0, point.1)
        .expect("right-click target has a controlled request");
    assert_eq!(menu, "menu");
    assert_eq!(pointer.value["tag"], "Activate");
    assert_eq!(pointer.value["values"][0]["tag"], "Request");
    input.focused = Some("trigger".into());
    assert_eq!(input.context_request_focused(&view).unwrap().0, "menu");
}

#[test]
fn menu_typeahead_cycles_enabled_prefixes_and_expires_after_500ms() {
    let view = View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["trigger","menu"]},
        {"key":"trigger","kind":{"tag":"Button","values":[]},"props":[{"tag":"Label","values":["Open"]}],"children":[]},
        {"key":"menu","kind":{"tag":"Overlay","values":[]},"props":[{"tag":"OverlayKind","values":[{"tag":"DropdownMenu","values":[]}]},{"tag":"Label","values":["Actions"]},{"tag":"Anchor","values":["trigger"]},{"tag":"Open","values":[true]}],"children":["start","disabled","bravo","beta"]},
        {"key":"start","kind":{"tag":"MenuItem","values":[]},"props":[{"tag":"Label","values":["Start"]},{"tag":"Option","values":["start"]},{"tag":"Activate","values":[{"tag":"Start"}]}],"children":[]},
        {"key":"disabled","kind":{"tag":"MenuItem","values":[]},"props":[{"tag":"Label","values":["Beta disabled"]},{"tag":"Option","values":["disabled"]},{"tag":"Disabled","values":[true]},{"tag":"Activate","values":[{"tag":"BetaDisabled"}]}],"children":[]},
        {"key":"bravo","kind":{"tag":"MenuItem","values":[]},"props":[{"tag":"Label","values":["Bravo"]},{"tag":"Option","values":["bravo"]},{"tag":"Activate","values":[{"tag":"Bravo"}]}],"children":[]},
        {"key":"beta","kind":{"tag":"MenuItem","values":[]},"props":[{"tag":"Label","values":["Beta"]},{"tag":"Option","values":["beta"]},{"tag":"Activate","values":[{"tag":"Beta"}]}],"children":[]}
    ]})).unwrap();
    let mut input = Input::default();
    input.focused = Some("start".into());
    assert!(input.typeahead(&view, "B", 0));
    assert_eq!(input.focused.as_deref(), Some("bravo"));
    assert!(input.typeahead(&view, "b", 100));
    assert_eq!(input.focused.as_deref(), Some("beta"));
    assert!(input.typeahead(&view, "BR", 700));
    assert_eq!(input.focused.as_deref(), Some("bravo"));
}

#[test]
fn combobox_arrows_focus_options_and_selection_restores_the_query_anchor() {
    let view = View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["query","combo"]},
        {"key":"query","kind":{"tag":"TextInput","values":[]},"props":[{"tag":"Label","values":["Country"]},{"tag":"Value","values":["ca"]},{"tag":"Input","values":[{"tag":"Query"}]}],"children":[]},
        {"key":"combo","kind":{"tag":"Overlay","values":[]},"props":[{"tag":"OverlayKind","values":[{"tag":"Combobox","values":[]}]},{"tag":"Label","values":["Countries"]},{"tag":"Anchor","values":["query"]},{"tag":"Open","values":[true]}],"children":["disabled","first","last"]},
        {"key":"disabled","kind":{"tag":"OptionItem","values":[]},"props":[{"tag":"Label","values":["Albania"]},{"tag":"Option","values":["al"]},{"tag":"Disabled","values":[true]},{"tag":"Change","values":[{"tag":"SelectCountry"}]}],"children":[]},
        {"key":"first","kind":{"tag":"OptionItem","values":[]},"props":[{"tag":"Label","values":["Brazil"]},{"tag":"Option","values":["br"]},{"tag":"Change","values":[{"tag":"SelectCountry"}]}],"children":[]},
        {"key":"last","kind":{"tag":"OptionItem","values":[]},"props":[{"tag":"Label","values":["Canada"]},{"tag":"Option","values":["ca"]},{"tag":"Change","values":[{"tag":"SelectCountry"}]}],"children":[]}
    ]})).unwrap();
    let mut input = Input::default();
    input.focused = Some("query".into());
    input.sync(&view);

    input.key(&view, Key::Up, true, false);
    assert_eq!(
        input.active_option(),
        Some("last"),
        "Up enters at the final enabled option"
    );
    assert_eq!(input.focused.as_deref(), Some("query"));
    input.key(&view, Key::Home, true, false);
    assert_eq!(
        input.active_option(),
        Some("first"),
        "Home skips disabled options"
    );
    input.key(&view, Key::Down, true, false);
    assert_eq!(input.active_option(), Some("last"));
    input.key(&view, Key::Enter, true, false);
    let selected = input
        .key(&view, Key::Enter, false, false)
        .expect("Enter commits the active descendant");
    assert_eq!(selected.key, "last");
    assert_eq!(selected.value["tag"], "Select");
    assert_eq!(input.focused.as_deref(), Some("query"));

    let scene = Layout::default()
        .compute(&view, 480.0, 320.0, &mut Text)
        .unwrap();
    input.caret = 1;
    let bounds = scene.boxes["last"].bounds;
    input.pointer_move(
        &view,
        &scene,
        bounds.x + bounds.width * 0.5,
        bounds.y + bounds.height * 0.5,
    );
    input.pointer_down(&view, &scene);
    assert_eq!(input.focused.as_deref(), Some("query"));
    assert_eq!(input.caret, 1);
    let pointer_selected = input.pointer_up(&view, &scene).unwrap();
    assert_eq!(pointer_selected.key, "last");
    assert_eq!(pointer_selected.value["tag"], "Select");
    assert_eq!(input.focused.as_deref(), Some("query"));
    assert_eq!(input.caret, 1);

    let mut closed = view.clone();
    closed.nodes.get_mut("combo").unwrap().open = false;
    input.sync(&closed);
    assert_eq!(input.focused.as_deref(), Some("query"));
    assert_eq!(input.active_option(), None);

    input.focused = Some("query".into());
    input.sync(&closed);
    input.key(&closed, Key::Down, true, false);
    assert_eq!(
        input.focused.as_deref(),
        Some("query"),
        "closed combobox leaves text input navigation alone"
    );
}

#[test]
fn nonmodal_selection_and_menu_focus_return_respects_external_focus() {
    let select_closed = View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["trigger","outside","select"]},
        {"key":"trigger","kind":{"tag":"Button","values":[]},"props":[{"tag":"Label","values":["Choose"]}],"children":[]},
        {"key":"outside","kind":{"tag":"Button","values":[]},"props":[{"tag":"Label","values":["Outside"]}],"children":[]},
        {"key":"select","kind":{"tag":"Overlay","values":[]},"props":[{"tag":"OverlayKind","values":[{"tag":"Select","values":[]}]},{"tag":"Label","values":["Choices"]},{"tag":"Anchor","values":["trigger"]},{"tag":"Open","values":[false]}],"children":["first","selected"]},
        {"key":"first","kind":{"tag":"OptionItem","values":[]},"props":[{"tag":"Label","values":["First"]},{"tag":"Option","values":["first"]},{"tag":"Change","values":[{"tag":"Choose"}]}],"children":[]},
        {"key":"selected","kind":{"tag":"OptionItem","values":[]},"props":[{"tag":"Label","values":["Selected"]},{"tag":"Option","values":["selected"]},{"tag":"Selected","values":[true]},{"tag":"Change","values":[{"tag":"Choose"}]}],"children":[]}
    ]})).unwrap();
    let mut input = Input::default();
    input.focused = Some("trigger".into());
    input.sync(&select_closed);
    let mut select_open = select_closed.clone();
    select_open.nodes.get_mut("select").unwrap().open = true;
    input.sync(&select_open);
    assert_eq!(input.focused.as_deref(), Some("selected"));

    let mut select_closed_again = select_open.clone();
    select_closed_again.nodes.get_mut("select").unwrap().open = false;
    input.sync(&select_closed_again);
    assert_eq!(input.focused.as_deref(), Some("trigger"));

    input.sync(&select_open);
    input.focused = Some("outside".into());
    input.sync(&select_open);
    input.sync(&select_closed_again);
    assert_eq!(input.focused.as_deref(), Some("outside"));

    for kind in ["DropdownMenu", "ContextMenu"] {
        let menu_closed = View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
            {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["trigger","menu"]},
            {"key":"trigger","kind":{"tag":"Button","values":[]},"props":[{"tag":"Label","values":["Menu"]}],"children":[]},
            {"key":"menu","kind":{"tag":"Overlay","values":[]},"props":[{"tag":"OverlayKind","values":[{"tag":kind,"values":[]}]},{"tag":"Label","values":["Actions"]},{"tag":"Anchor","values":["trigger"]},{"tag":"Open","values":[false]}],"children":["disabled","first"]},
            {"key":"disabled","kind":{"tag":"MenuItem","values":[]},"props":[{"tag":"Label","values":["Disabled"]},{"tag":"Option","values":["disabled"]},{"tag":"Disabled","values":[true]},{"tag":"Activate","values":[{"tag":"DisabledAction"}]}],"children":[]},
            {"key":"first","kind":{"tag":"MenuItem","values":[]},"props":[{"tag":"Label","values":["First"]},{"tag":"Option","values":["first"]},{"tag":"Activate","values":[{"tag":"FirstAction"}]}],"children":[]}
        ]})).unwrap();
        let mut menu_open = menu_closed.clone();
        menu_open.nodes.get_mut("menu").unwrap().open = true;
        input.focused = Some("trigger".into());
        input.sync(&menu_closed);
        input.sync(&menu_open);
        assert_eq!(
            input.focused.as_deref(),
            Some("first"),
            "{kind} skips disabled entries"
        );
        input.sync(&menu_closed);
        assert_eq!(
            input.focused.as_deref(),
            Some("trigger"),
            "{kind} restores its source trigger"
        );
    }
}

#[test]
fn command_menu_navigation_follows_items_nested_below_the_menu_role() {
    let view = View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["menu"]},
        {"key":"menu","kind":{"tag":"Column","values":[]},"props":[{"tag":"Role","values":["menu"]}],"children":["group"]},
        {"key":"group","kind":{"tag":"Row","values":[]},"props":[],"children":["one","two"]},
        {"key":"one","kind":{"tag":"MenuItem","values":[]},"props":[{"tag":"Label","values":["Open"]},{"tag":"Option","values":["open"]},{"tag":"Activate","values":[{"tag":"Open"}]}],"children":[]},
        {"key":"two","kind":{"tag":"MenuItem","values":[]},"props":[{"tag":"Label","values":["Close"]},{"tag":"Option","values":["close"]},{"tag":"Activate","values":[{"tag":"Close"}]}],"children":[]}
    ]})).unwrap();
    let mut input = Input::default();
    input.focused = Some("one".into());
    assert!(input.key(&view, Key::Down, true, false).is_none());
    assert_eq!(input.focused.as_deref(), Some("two"));
    assert!(input.key(&view, Key::Home, true, false).is_none());
    assert_eq!(input.focused.as_deref(), Some("one"));
}
