//! Reproducible documentation captures from the actual Ubi gallery and GPU host.
use serde_json::json;
use ubi_lang::{ui::NativeApplication, Compiler};
use ubi_ui_platform::{Application, Event};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .ok_or("usage: native-ui-capture <existing-output-directory>")?;
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
        c.add_source(id, source).map_err(|e| format!("{e:?}"))?;
    }
    let mut app = NativeApplication::new(
        c.runtime().map_err(|e| format!("{e:?}"))?,
        "examples/gallery.ubi",
    )?;
    for (name, w, h) in [
        ("native-light.png", 1100, 1000),
        ("native-narrow.png", 420, 1000),
    ] {
        let adapter = ubi_ui_platform::capture(&app, &out.join(name), w, h, false)?;
        println!("{name}: {w}x{h}, {adapter}");
    }
    app.dispatch(&Event {
        key: "theme".into(),
        value: json!({"tag":"Toggle","values":[{"tag":"Theme","values":[]},true]}),
    })?;
    let adapter = ubi_ui_platform::capture(&app, &out.join("native-dark.png"), 1100, 1000, true)?;
    println!("native-dark.png: 1100x1000, {adapter}");
    Ok(())
}
