//! Independent contracts for the source-owned Ubi UI library.
use crate::generics_tests::{analysis, checked, node, run};
use crate::interpreter::Value;
use crate::Analysis;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy)]
enum ExpectedValue<'a> {
    String(&'a str),
    Int(i32),
    Bool(bool),
    Enum(&'a str, &'a [ExpectedValue<'a>]),
}

#[derive(Clone, Copy)]
struct ExpectedProp<'a> {
    tag: &'a str,
    values: &'a [ExpectedValue<'a>],
}

#[derive(Clone, Copy)]
struct ExpectedNode<'a> {
    key: &'a str,
    kind: &'a str,
    props: &'a [ExpectedProp<'a>],
    children: &'a [&'a str],
}

fn ui_source(path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/ui/src")
        .join(path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()))
}

fn call(a: &Analysis, name: &str) -> Value {
    run(a, name, vec![]).unwrap()
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    let Value::Record { fields, .. } = value else {
        panic!("record expected for field {key}: {value:?}");
    };
    fields
        .get(key)
        .unwrap_or_else(|| panic!("missing field {key}: {value:?}"))
}

fn record_keys(value: &Value) -> Vec<&str> {
    let Value::Record { fields, .. } = value else {
        panic!("record expected: {value:?}");
    };
    fields.keys().map(String::as_str).collect()
}

fn list(value: &Value) -> &[Value] {
    let Value::List { elements, .. } = value else {
        panic!("list expected: {value:?}");
    };
    elements
}

fn string(value: &Value) -> &str {
    let Value::String(value) = value else {
        panic!("string expected: {value:?}");
    };
    value
}

fn enum_parts(value: &Value) -> (&str, &[Value]) {
    let Value::Enum { tag, values, .. } = value else {
        panic!("enum expected: {value:?}");
    };
    (tag, values)
}

fn assert_value(actual: &Value, expected: ExpectedValue<'_>) {
    match expected {
        ExpectedValue::String(expected) => assert_eq!(string(actual), expected),
        ExpectedValue::Int(expected) => {
            let Value::Int(actual) = actual else {
                panic!("integer expected: {actual:?}");
            };
            assert_eq!(*actual, expected);
        }
        ExpectedValue::Bool(expected) => {
            let Value::Bool(actual) = actual else {
                panic!("boolean expected: {actual:?}");
            };
            assert_eq!(*actual, expected);
        }
        ExpectedValue::Enum(expected_tag, expected_values) => {
            let (tag, values) = enum_parts(actual);
            assert_eq!(tag, expected_tag);
            assert_eq!(values.len(), expected_values.len());
            for (value, expected) in values.iter().zip(expected_values) {
                assert_value(value, *expected);
            }
        }
    }
}

fn assert_view(value: &Value, root: &str, expected_nodes: &[ExpectedNode<'_>]) {
    assert_eq!(record_keys(value), ["nodes", "root", "schemaVersion"]);
    assert_value(field(value, "schemaVersion"), ExpectedValue::Int(1));
    assert_value(field(value, "root"), ExpectedValue::String(root));

    let nodes = list(field(value, "nodes"));
    assert_eq!(nodes.len(), expected_nodes.len());
    for (node, expected) in nodes.iter().zip(expected_nodes) {
        assert_eq!(record_keys(node), ["children", "key", "kind", "props"]);
        assert_value(field(node, "key"), ExpectedValue::String(expected.key));
        assert_value(field(node, "kind"), ExpectedValue::Enum(expected.kind, &[]));

        let props = list(field(node, "props"));
        assert_eq!(props.len(), expected.props.len(), "node {}", expected.key);
        for (prop, expected_prop) in props.iter().zip(expected.props) {
            let (tag, values) = enum_parts(prop);
            assert_eq!(tag, expected_prop.tag, "node {}", expected.key);
            assert_eq!(values.len(), expected_prop.values.len(), "prop {tag}");
            for (value, expected) in values.iter().zip(expected_prop.values) {
                assert_value(value, *expected);
            }
        }

        let children = list(field(node, "children"));
        assert_eq!(
            children.len(),
            expected.children.len(),
            "node {}",
            expected.key
        );
        for (child, expected_child) in children.iter().zip(expected.children) {
            assert_value(child, ExpectedValue::String(expected_child));
        }
    }
}

fn assert_views(values: &[Value], roots_and_nodes: &[(&str, &[ExpectedNode<'_>])]) {
    assert_eq!(values.len(), roots_and_nodes.len());
    for (value, (root, nodes)) in values.iter().zip(roots_and_nodes) {
        assert_view(value, root, nodes);
    }
}

#[test]
fn flat_composition_preserves_keys_and_order() {
    let core = ui_source("ui/core.ubi");
    let a = checked(&[
        ("ui/core.ubi", &core),
        (
            "main.ubi",
            r#"
import { View, Kind, Prop, node, group } from "./ui/core.ubi";
export enum Message { Save, EditName }
export fn flat() -> View<Message> {
  group("settings", Kind.Column, [], [
    node("title", Kind.Text, [Prop.Content("Workspace settings")]),
    node("save", Kind.Button, [Prop.Content("Save"), Prop.Activate(Message.Save)])
  ])
}
"#,
        ),
    ]);

    const SAVE: &[ExpectedValue<'static>] = &[ExpectedValue::Enum("Save", &[])];
    const FLAT_NODES: &[ExpectedNode<'static>] = &[
        ExpectedNode {
            key: "settings",
            kind: "Column",
            props: &[],
            children: &["title", "save"],
        },
        ExpectedNode {
            key: "title",
            kind: "Text",
            props: &[ExpectedProp {
                tag: "Content",
                values: &[ExpectedValue::String("Workspace settings")],
            }],
            children: &[],
        },
        ExpectedNode {
            key: "save",
            kind: "Button",
            props: &[
                ExpectedProp {
                    tag: "Content",
                    values: &[ExpectedValue::String("Save")],
                },
                ExpectedProp {
                    tag: "Activate",
                    values: SAVE,
                },
            ],
            children: &[],
        },
    ];
    assert_view(&call(&a, "flat"), "settings", FLAT_NODES);
    node(
        &a,
        "assert.deepEqual(m.flat(), {schemaVersion:1,root:'settings',nodes:[{key:'settings',kind:{tag:'Column',values:[]},props:[],children:['title','save']},{key:'title',kind:{tag:'Text',values:[]},props:[{tag:'Content',values:['Workspace settings']}],children:[]},{key:'save',kind:{tag:'Button',values:[]},props:[{tag:'Content',values:['Save']},{tag:'Activate',values:[{tag:'Save',values:[]}]}],children:[]}]});",
    );
}

#[test]
fn message_type_is_preserved_across_imports() {
    let core = ui_source("ui/core.ubi");
    let a = checked(&[
        ("ui/core.ubi", &core),
        (
            "ui/fragment.ubi",
            r#"
import { View, Kind, Prop, node } from "./core.ubi";
export fn passive<M>() -> View<M> {
  node("title", Kind.Text, [Prop.Content("Workspace")])
}
"#,
        ),
        (
            "main.ubi",
            r#"
import { View, Kind, Prop, node, group } from "./ui/core.ubi";
import { passive } from "./ui/fragment.ubi";
export enum Message { Save }
export fn imported() -> View<Message> {
  let title: View<Message> = passive();
  group("settings", Kind.Column, [], [
    title,
    node("save", Kind.Button, [Prop.Content("Save"), Prop.Activate(Message.Save)])
  ])
}
"#,
        ),
    ]);

    const SAVE: &[ExpectedValue<'static>] = &[ExpectedValue::Enum("Save", &[])];
    const IMPORTED_NODES: &[ExpectedNode<'static>] = &[
        ExpectedNode {
            key: "settings",
            kind: "Column",
            props: &[],
            children: &["title", "save"],
        },
        ExpectedNode {
            key: "title",
            kind: "Text",
            props: &[ExpectedProp {
                tag: "Content",
                values: &[ExpectedValue::String("Workspace")],
            }],
            children: &[],
        },
        ExpectedNode {
            key: "save",
            kind: "Button",
            props: &[
                ExpectedProp {
                    tag: "Content",
                    values: &[ExpectedValue::String("Save")],
                },
                ExpectedProp {
                    tag: "Activate",
                    values: SAVE,
                },
            ],
            children: &[],
        },
    ];
    assert_view(&call(&a, "imported"), "settings", IMPORTED_NODES);
    node(
        &a,
        "assert.deepEqual(m.imported(), {schemaVersion:1,root:'settings',nodes:[{key:'settings',kind:{tag:'Column',values:[]},props:[],children:['title','save']},{key:'title',kind:{tag:'Text',values:[]},props:[{tag:'Content',values:['Workspace']}],children:[]},{key:'save',kind:{tag:'Button',values:[]},props:[{tag:'Content',values:['Save']},{tag:'Activate',values:[{tag:'Save',values:[]}]}],children:[]}]});",
    );
}

#[test]
fn context_infers_passive_component_message() {
    let core = ui_source("ui/core.ubi");
    let a = checked(&[
        ("ui/core.ubi", &core),
        (
            "main.ubi",
            r#"
import { View, Kind, Prop, node } from "./ui/core.ubi";
export enum Message { Save }
export fn inferred() -> View<Message> {
  let title: View<Message> = node("title", Kind.Text, [Prop.Content("No action")]);
  title
}
"#,
        ),
    ]);

    const INFERRED_NODES: &[ExpectedNode<'static>] = &[ExpectedNode {
        key: "title",
        kind: "Text",
        props: &[ExpectedProp {
            tag: "Content",
            values: &[ExpectedValue::String("No action")],
        }],
        children: &[],
    }];
    assert_view(&call(&a, "inferred"), "title", INFERRED_NODES);
    node(
        &a,
        "assert.deepEqual(m.inferred(), {schemaVersion:1,root:'title',nodes:[{key:'title',kind:{tag:'Text',values:[]},props:[{tag:'Content',values:['No action']}],children:[]}]});",
    );
}

#[test]
fn nested_fragments_stay_flat() {
    let core = ui_source("ui/core.ubi");
    let a = checked(&[
        ("ui/core.ubi", &core),
        (
            "main.ubi",
            r#"
import { View, Kind, Prop, Space, node, group } from "./ui/core.ubi";
export enum Message { Save }
export fn nested() -> View<Message> {
  group("settings", Kind.Column, [Prop.Gap(Space.Md)], [
    group("profile", Kind.Card, [], [
      node("title", Kind.Heading, [Prop.Content("Workspace"), Prop.Level(2)])
    ]),
    group("actions", Kind.Row, [Prop.Gap(Space.Sm)], [
      node("save", Kind.Button, [Prop.Content("Save"), Prop.Activate(Message.Save)])
    ])
  ])
}
"#,
        ),
    ]);

    const SAVE: &[ExpectedValue<'static>] = &[ExpectedValue::Enum("Save", &[])];
    const NODES: &[ExpectedNode<'static>] = &[
        ExpectedNode {
            key: "settings",
            kind: "Column",
            props: &[ExpectedProp {
                tag: "Gap",
                values: &[ExpectedValue::Enum("Md", &[])],
            }],
            children: &["profile", "actions"],
        },
        ExpectedNode {
            key: "profile",
            kind: "Card",
            props: &[],
            children: &["title"],
        },
        ExpectedNode {
            key: "title",
            kind: "Heading",
            props: &[
                ExpectedProp {
                    tag: "Content",
                    values: &[ExpectedValue::String("Workspace")],
                },
                ExpectedProp {
                    tag: "Level",
                    values: &[ExpectedValue::Int(2)],
                },
            ],
            children: &[],
        },
        ExpectedNode {
            key: "actions",
            kind: "Row",
            props: &[ExpectedProp {
                tag: "Gap",
                values: &[ExpectedValue::Enum("Sm", &[])],
            }],
            children: &["save"],
        },
        ExpectedNode {
            key: "save",
            kind: "Button",
            props: &[
                ExpectedProp {
                    tag: "Content",
                    values: &[ExpectedValue::String("Save")],
                },
                ExpectedProp {
                    tag: "Activate",
                    values: SAVE,
                },
            ],
            children: &[],
        },
    ];
    assert_view(&call(&a, "nested"), "settings", NODES);
    node(
        &a,
        "assert.deepEqual(m.nested(), {schemaVersion:1,root:'settings',nodes:[{key:'settings',kind:{tag:'Column',values:[]},props:[{tag:'Gap',values:[{tag:'Md',values:[]}]}],children:['profile','actions']},{key:'profile',kind:{tag:'Card',values:[]},props:[],children:['title']},{key:'title',kind:{tag:'Heading',values:[]},props:[{tag:'Content',values:['Workspace']},{tag:'Level',values:[2]}],children:[]},{key:'actions',kind:{tag:'Row',values:[]},props:[{tag:'Gap',values:[{tag:'Sm',values:[]}]}],children:['save']},{key:'save',kind:{tag:'Button',values:[]},props:[{tag:'Content',values:['Save']},{tag:'Activate',values:[{tag:'Save',values:[]}]}],children:[]}]});",
    );
}

#[test]
fn incompatible_and_unresolved_message_types_are_rejected() {
    let core = ui_source("ui/core.ubi");
    let incompatible = analysis(&[
        ("ui/core.ubi", &core),
        (
            "main.ubi",
            r#"
import { View, Kind, Prop, node, group } from "./ui/core.ubi";
enum Left { Save }
enum Right { Cancel }
fn left() -> View<Left> { node("left", Kind.Button, [Prop.Activate(Left.Save)]) }
fn right() -> View<Right> { node("right", Kind.Button, [Prop.Activate(Right.Cancel)]) }
fn invalid() { group("root", Kind.Row, [], [left(), right()]); }
"#,
        ),
    ]);
    assert!(
        !incompatible.diagnostics.is_empty(),
        "incompatible messages accepted"
    );
    assert!(crate::ir::lower(&incompatible).is_err());

    let unresolved = analysis(&[
        ("ui/core.ubi", &core),
        (
            "main.ubi",
            r#"
import { Kind, Prop, node } from "./ui/core.ubi";
fn invalid() { node("title", Kind.Text, [Prop.Content("No context")]); }
"#,
        ),
    ]);
    assert!(
        !unresolved.diagnostics.is_empty(),
        "unresolved messages accepted"
    );
    assert!(crate::ir::lower(&unresolved).is_err());
}

fn components_analysis(main: &str) -> Analysis {
    let core = ui_source("ui/core.ubi");
    let components = ui_source("ui/components.ubi");
    checked(&[
        ("ui/core.ubi", &core),
        ("ui/components.ubi", &components),
        ("main.ubi", main),
    ])
}

#[test]
fn components_emit_fixed_semantics() {
    let a = components_analysis(
        r#"
import { View, ButtonVariant, Tone } from "./ui/core.ubi";
import { heading, text, badge, button } from "./ui/components.ubi";
export enum Message { Save }
export fn headings() -> List<View<Message>> {
  [heading("h1", "One", 1), heading("h2", "Two", 2), heading("h3", "Three", 3),
   heading("h4", "Four", 4), heading("h5", "Five", 5), heading("h6", "Six", 6)]
}
export fn unclampedHeading() -> View<Message> { heading("h0", "Zero", 0) }
export fn unicode() -> List<View<Message>> {
  [text("unicode-text", "Café 東京 🧭"),
   button("unicode-button", "Save 🔐", ButtonVariant.Ghost, false, false, Message.Save),
   badge("unicode-badge", "Prêt ✓", Tone.Success)]
}
export fn variants() -> List<View<Message>> {
  [button("primary", "Primary", ButtonVariant.Primary, false, false, Message.Save),
   button("secondary", "Secondary", ButtonVariant.Secondary, false, false, Message.Save),
   button("outline", "Outline", ButtonVariant.Outline, false, false, Message.Save),
   button("ghost", "Ghost", ButtonVariant.Ghost, false, false, Message.Save),
   button("destructive", "Destructive", ButtonVariant.Destructive, false, false, Message.Save)]
}
"#,
    );

    let headings = call(&a, "headings");
    let heading_values = list(&headings);
    assert_eq!(heading_values.len(), 6);
    assert_view(
        &heading_values[0],
        "h1",
        &[ExpectedNode {
            key: "h1",
            kind: "Heading",
            props: &[
                ExpectedProp {
                    tag: "Content",
                    values: &[ExpectedValue::String("One")],
                },
                ExpectedProp {
                    tag: "Level",
                    values: &[ExpectedValue::Int(1)],
                },
            ],
            children: &[],
        }],
    );
    assert_view(
        &heading_values[1],
        "h2",
        &[ExpectedNode {
            key: "h2",
            kind: "Heading",
            props: &[
                ExpectedProp {
                    tag: "Content",
                    values: &[ExpectedValue::String("Two")],
                },
                ExpectedProp {
                    tag: "Level",
                    values: &[ExpectedValue::Int(2)],
                },
            ],
            children: &[],
        }],
    );
    assert_view(
        &heading_values[2],
        "h3",
        &[ExpectedNode {
            key: "h3",
            kind: "Heading",
            props: &[
                ExpectedProp {
                    tag: "Content",
                    values: &[ExpectedValue::String("Three")],
                },
                ExpectedProp {
                    tag: "Level",
                    values: &[ExpectedValue::Int(3)],
                },
            ],
            children: &[],
        }],
    );
    assert_view(
        &heading_values[3],
        "h4",
        &[ExpectedNode {
            key: "h4",
            kind: "Heading",
            props: &[
                ExpectedProp {
                    tag: "Content",
                    values: &[ExpectedValue::String("Four")],
                },
                ExpectedProp {
                    tag: "Level",
                    values: &[ExpectedValue::Int(4)],
                },
            ],
            children: &[],
        }],
    );
    assert_view(
        &heading_values[4],
        "h5",
        &[ExpectedNode {
            key: "h5",
            kind: "Heading",
            props: &[
                ExpectedProp {
                    tag: "Content",
                    values: &[ExpectedValue::String("Five")],
                },
                ExpectedProp {
                    tag: "Level",
                    values: &[ExpectedValue::Int(5)],
                },
            ],
            children: &[],
        }],
    );
    assert_view(
        &heading_values[5],
        "h6",
        &[ExpectedNode {
            key: "h6",
            kind: "Heading",
            props: &[
                ExpectedProp {
                    tag: "Content",
                    values: &[ExpectedValue::String("Six")],
                },
                ExpectedProp {
                    tag: "Level",
                    values: &[ExpectedValue::Int(6)],
                },
            ],
            children: &[],
        }],
    );
    const ZERO: &[ExpectedNode<'static>] = &[ExpectedNode {
        key: "h0",
        kind: "Heading",
        props: &[
            ExpectedProp {
                tag: "Content",
                values: &[ExpectedValue::String("Zero")],
            },
            ExpectedProp {
                tag: "Level",
                values: &[ExpectedValue::Int(0)],
            },
        ],
        children: &[],
    }];
    assert_view(&call(&a, "unclampedHeading"), "h0", ZERO);

    let unicode = call(&a, "unicode");
    const SAVE: &[ExpectedValue<'static>] = &[ExpectedValue::Enum("Save", &[])];
    let expected_unicode = [
        (
            "unicode-text",
            &[ExpectedNode {
                key: "unicode-text",
                kind: "Text",
                props: &[ExpectedProp {
                    tag: "Content",
                    values: &[ExpectedValue::String("Café 東京 🧭")],
                }],
                children: &[],
            }][..],
        ),
        (
            "unicode-button",
            &[ExpectedNode {
                key: "unicode-button",
                kind: "Button",
                props: &[
                    ExpectedProp {
                        tag: "Content",
                        values: &[ExpectedValue::String("Save 🔐")],
                    },
                    ExpectedProp {
                        tag: "Variant",
                        values: &[ExpectedValue::Enum("Ghost", &[])],
                    },
                    ExpectedProp {
                        tag: "Disabled",
                        values: &[ExpectedValue::Bool(false)],
                    },
                    ExpectedProp {
                        tag: "Busy",
                        values: &[ExpectedValue::Bool(false)],
                    },
                    ExpectedProp {
                        tag: "Activate",
                        values: SAVE,
                    },
                ],
                children: &[],
            }][..],
        ),
        (
            "unicode-badge",
            &[ExpectedNode {
                key: "unicode-badge",
                kind: "Badge",
                props: &[
                    ExpectedProp {
                        tag: "Content",
                        values: &[ExpectedValue::String("Prêt ✓")],
                    },
                    ExpectedProp {
                        tag: "Tone",
                        values: &[ExpectedValue::Enum("Success", &[])],
                    },
                ],
                children: &[],
            }][..],
        ),
    ];
    assert_views(list(&unicode), &expected_unicode);

    let variants = call(&a, "variants");
    let expected_variants = [
        (
            "primary",
            &[ExpectedNode {
                key: "primary",
                kind: "Button",
                children: &[],
                props: &[
                    ExpectedProp {
                        tag: "Content",
                        values: &[ExpectedValue::String("Primary")],
                    },
                    ExpectedProp {
                        tag: "Variant",
                        values: &[ExpectedValue::Enum("Primary", &[])],
                    },
                    ExpectedProp {
                        tag: "Disabled",
                        values: &[ExpectedValue::Bool(false)],
                    },
                    ExpectedProp {
                        tag: "Busy",
                        values: &[ExpectedValue::Bool(false)],
                    },
                    ExpectedProp {
                        tag: "Activate",
                        values: SAVE,
                    },
                ],
            }][..],
        ),
        (
            "secondary",
            &[ExpectedNode {
                key: "secondary",
                kind: "Button",
                children: &[],
                props: &[
                    ExpectedProp {
                        tag: "Content",
                        values: &[ExpectedValue::String("Secondary")],
                    },
                    ExpectedProp {
                        tag: "Variant",
                        values: &[ExpectedValue::Enum("Secondary", &[])],
                    },
                    ExpectedProp {
                        tag: "Disabled",
                        values: &[ExpectedValue::Bool(false)],
                    },
                    ExpectedProp {
                        tag: "Busy",
                        values: &[ExpectedValue::Bool(false)],
                    },
                    ExpectedProp {
                        tag: "Activate",
                        values: SAVE,
                    },
                ],
            }][..],
        ),
        (
            "outline",
            &[ExpectedNode {
                key: "outline",
                kind: "Button",
                children: &[],
                props: &[
                    ExpectedProp {
                        tag: "Content",
                        values: &[ExpectedValue::String("Outline")],
                    },
                    ExpectedProp {
                        tag: "Variant",
                        values: &[ExpectedValue::Enum("Outline", &[])],
                    },
                    ExpectedProp {
                        tag: "Disabled",
                        values: &[ExpectedValue::Bool(false)],
                    },
                    ExpectedProp {
                        tag: "Busy",
                        values: &[ExpectedValue::Bool(false)],
                    },
                    ExpectedProp {
                        tag: "Activate",
                        values: SAVE,
                    },
                ],
            }][..],
        ),
        (
            "ghost",
            &[ExpectedNode {
                key: "ghost",
                kind: "Button",
                children: &[],
                props: &[
                    ExpectedProp {
                        tag: "Content",
                        values: &[ExpectedValue::String("Ghost")],
                    },
                    ExpectedProp {
                        tag: "Variant",
                        values: &[ExpectedValue::Enum("Ghost", &[])],
                    },
                    ExpectedProp {
                        tag: "Disabled",
                        values: &[ExpectedValue::Bool(false)],
                    },
                    ExpectedProp {
                        tag: "Busy",
                        values: &[ExpectedValue::Bool(false)],
                    },
                    ExpectedProp {
                        tag: "Activate",
                        values: SAVE,
                    },
                ],
            }][..],
        ),
        (
            "destructive",
            &[ExpectedNode {
                key: "destructive",
                kind: "Button",
                children: &[],
                props: &[
                    ExpectedProp {
                        tag: "Content",
                        values: &[ExpectedValue::String("Destructive")],
                    },
                    ExpectedProp {
                        tag: "Variant",
                        values: &[ExpectedValue::Enum("Destructive", &[])],
                    },
                    ExpectedProp {
                        tag: "Disabled",
                        values: &[ExpectedValue::Bool(false)],
                    },
                    ExpectedProp {
                        tag: "Busy",
                        values: &[ExpectedValue::Bool(false)],
                    },
                    ExpectedProp {
                        tag: "Activate",
                        values: SAVE,
                    },
                ],
            }][..],
        ),
    ];
    assert_views(list(&variants), &expected_variants);

    node(
        &a,
        "assert.deepEqual(m.headings(), [{schemaVersion:1,root:'h1',nodes:[{key:'h1',kind:{tag:'Heading',values:[]},props:[{tag:'Content',values:['One']},{tag:'Level',values:[1]}],children:[]}]},{schemaVersion:1,root:'h2',nodes:[{key:'h2',kind:{tag:'Heading',values:[]},props:[{tag:'Content',values:['Two']},{tag:'Level',values:[2]}],children:[]}]},{schemaVersion:1,root:'h3',nodes:[{key:'h3',kind:{tag:'Heading',values:[]},props:[{tag:'Content',values:['Three']},{tag:'Level',values:[3]}],children:[]}]},{schemaVersion:1,root:'h4',nodes:[{key:'h4',kind:{tag:'Heading',values:[]},props:[{tag:'Content',values:['Four']},{tag:'Level',values:[4]}],children:[]}]},{schemaVersion:1,root:'h5',nodes:[{key:'h5',kind:{tag:'Heading',values:[]},props:[{tag:'Content',values:['Five']},{tag:'Level',values:[5]}],children:[]}]},{schemaVersion:1,root:'h6',nodes:[{key:'h6',kind:{tag:'Heading',values:[]},props:[{tag:'Content',values:['Six']},{tag:'Level',values:[6]}],children:[]}]}]); assert.deepEqual(m.unclampedHeading(), {schemaVersion:1,root:'h0',nodes:[{key:'h0',kind:{tag:'Heading',values:[]},props:[{tag:'Content',values:['Zero']},{tag:'Level',values:[0]}],children:[]}]}); assert.deepEqual(m.unicode(), [{schemaVersion:1,root:'unicode-text',nodes:[{key:'unicode-text',kind:{tag:'Text',values:[]},props:[{tag:'Content',values:['Café 東京 🧭']}],children:[]}]},{schemaVersion:1,root:'unicode-button',nodes:[{key:'unicode-button',kind:{tag:'Button',values:[]},props:[{tag:'Content',values:['Save 🔐']},{tag:'Variant',values:[{tag:'Ghost',values:[]}]},{tag:'Disabled',values:[false]},{tag:'Busy',values:[false]},{tag:'Activate',values:[{tag:'Save',values:[]}]}],children:[]}]},{schemaVersion:1,root:'unicode-badge',nodes:[{key:'unicode-badge',kind:{tag:'Badge',values:[]},props:[{tag:'Content',values:['Prêt ✓']},{tag:'Tone',values:[{tag:'Success',values:[]}]}],children:[]}]}]); assert.deepEqual(m.variants().map(v=>v.nodes[0].props[1].values[0].tag), ['Primary','Secondary','Outline','Ghost','Destructive']);",
    );
}

#[test]
fn themed_replaces_only_root_theme() {
    let a = components_analysis(
        r#"
import { View, Space, Theme, ButtonVariant } from "./ui/core.ubi";
import { column, button, themed } from "./ui/components.ubi";
export enum Message { Save }
export fn themedView() -> View<Message> {
  let content = column("settings", Space.Md, [
    button("save", "Save", ButtonVariant.Primary, false, false, Message.Save)
  ]);
  themed(themed(content, Theme.Light), Theme.Dark)
}
"#,
    );

    const SAVE: &[ExpectedValue<'static>] = &[ExpectedValue::Enum("Save", &[])];
    const NODES: &[ExpectedNode<'static>] = &[
        ExpectedNode {
            key: "settings",
            kind: "Column",
            props: &[
                ExpectedProp {
                    tag: "Gap",
                    values: &[ExpectedValue::Enum("Md", &[])],
                },
                ExpectedProp {
                    tag: "Theme",
                    values: &[ExpectedValue::Enum("Dark", &[])],
                },
            ],
            children: &["save"],
        },
        ExpectedNode {
            key: "save",
            kind: "Button",
            props: &[
                ExpectedProp {
                    tag: "Content",
                    values: &[ExpectedValue::String("Save")],
                },
                ExpectedProp {
                    tag: "Variant",
                    values: &[ExpectedValue::Enum("Primary", &[])],
                },
                ExpectedProp {
                    tag: "Disabled",
                    values: &[ExpectedValue::Bool(false)],
                },
                ExpectedProp {
                    tag: "Busy",
                    values: &[ExpectedValue::Bool(false)],
                },
                ExpectedProp {
                    tag: "Activate",
                    values: SAVE,
                },
            ],
            children: &[],
        },
    ];
    assert_view(&call(&a, "themedView"), "settings", NODES);
    node(
        &a,
        "assert.deepEqual(m.themedView(), {schemaVersion:1,root:'settings',nodes:[{key:'settings',kind:{tag:'Column',values:[]},props:[{tag:'Gap',values:[{tag:'Md',values:[]}]},{tag:'Theme',values:[{tag:'Dark',values:[]}]}],children:['save']},{key:'save',kind:{tag:'Button',values:[]},props:[{tag:'Content',values:['Save']},{tag:'Variant',values:[{tag:'Primary',values:[]}]},{tag:'Disabled',values:[false]},{tag:'Busy',values:[false]},{tag:'Activate',values:[{tag:'Save',values:[]}]}],children:[]}]});",
    );
}

#[test]
fn pressed_replaces_only_root_pressed() {
    let a = components_analysis(
        r#"
import { View, ButtonVariant } from "./ui/core.ubi";
import { button, pressed } from "./ui/components.ubi";
export enum Message { Save }
export fn pressedButton() -> View<Message> {
  let base = button("save", "Save", ButtonVariant.Outline, false, false, Message.Save);
  pressed(pressed(base, true), false)
}
"#,
    );

    const SAVE: &[ExpectedValue<'static>] = &[ExpectedValue::Enum("Save", &[])];
    const NODES: &[ExpectedNode<'static>] = &[ExpectedNode {
        key: "save",
        kind: "Button",
        props: &[
            ExpectedProp {
                tag: "Content",
                values: &[ExpectedValue::String("Save")],
            },
            ExpectedProp {
                tag: "Variant",
                values: &[ExpectedValue::Enum("Outline", &[])],
            },
            ExpectedProp {
                tag: "Disabled",
                values: &[ExpectedValue::Bool(false)],
            },
            ExpectedProp {
                tag: "Busy",
                values: &[ExpectedValue::Bool(false)],
            },
            ExpectedProp {
                tag: "Activate",
                values: SAVE,
            },
            ExpectedProp {
                tag: "Pressed",
                values: &[ExpectedValue::Bool(false)],
            },
        ],
        children: &[],
    }];
    assert_view(&call(&a, "pressedButton"), "save", NODES);
    node(
        &a,
        "assert.deepEqual(m.pressedButton(), {schemaVersion:1,root:'save',nodes:[{key:'save',kind:{tag:'Button',values:[]},props:[{tag:'Content',values:['Save']},{tag:'Variant',values:[{tag:'Outline',values:[]}]},{tag:'Disabled',values:[false]},{tag:'Busy',values:[false]},{tag:'Activate',values:[{tag:'Save',values:[]}]},{tag:'Pressed',values:[false]}],children:[]}]});",
    );
}
