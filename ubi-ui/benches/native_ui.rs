//! Standalone measured Ubi state/view/reconciliation benchmark; no GPU timing claims.
use serde_json::json;
use std::{hint::black_box, time::Instant};
use ubi_lang::{ui::NativeApplication, Compiler};
use ubi_ui_platform::{Application, Event};
fn main() {
    let mut c = Compiler::new();
    for (id, source) in [
        (
            "examples/gallery.ubi",
            include_str!("../examples/gallery.ubi"),
        ),
        (
            "packages/ubi-ui/core.ubi",
            include_str!("../packages/ubi-ui/core.ubi"),
        ),
        (
            "packages/ubi-ui/components.ubi",
            include_str!("../packages/ubi-ui/components.ubi"),
        ),
    ] {
        c.add_source(id, source).unwrap();
    }
    let start = Instant::now();
    let runtime = c.runtime().unwrap();
    println!("checked native program: {:?}", start.elapsed());
    let mut app = NativeApplication::new(runtime, "examples/gallery.ubi").unwrap();
    let increment = Event {
        key: "increment".into(),
        value: json!({"tag":"Activate","values":[{"tag":"Increment","values":[]}]}),
    };
    for _ in 0..20 {
        black_box(app.dispatch(&increment).unwrap());
    }
    let mut times = Vec::new();
    for _ in 0..500 {
        let t = Instant::now();
        let delta = app.dispatch(&increment).unwrap();
        assert_eq!(delta.updated, ["count"]);
        times.push(t.elapsed());
        black_box(delta);
    }
    times.sort();
    println!(
        "49-node Ubi update + derived view + keyed reconciliation: p50={:?}, p95={:?}, n=500",
        times[250], times[475]
    );
    assert_eq!(app.model()["count"], 520);
    let reset = Event {
        key: "reset".into(),
        value: json!({"tag":"Activate","values":[{"tag":"Reset","values":[]}]}),
    };
    app.dispatch(&reset).unwrap();
    let before = app.view_evaluations;
    let t = Instant::now();
    for _ in 0..500 {
        assert!(black_box(app.dispatch(&reset).unwrap()).is_empty());
    }
    assert_eq!(app.view_evaluations, before);
    println!(
        "equal-state reducer, view evaluations skipped: {:?}/call",
        t.elapsed() / 500
    );
}
