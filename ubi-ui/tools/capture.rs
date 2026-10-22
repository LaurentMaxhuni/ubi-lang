//! Reproducible documentation captures from the actual Ubi gallery and GPU host.
use serde_json::json;
use ubi_lang::ui::NativeApplication;
use ubi_ui_platform::{Application, Event};
mod sources;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let first = args
        .next()
        .ok_or("usage: native-ui-capture [--components] <existing-empty-directory>")?;
    if first == "--components" {
        let out = std::path::PathBuf::from(args.next().ok_or("missing capture directory")?);
        if args.next().is_some() {
            return Err("unexpected capture argument".into());
        }
        return components(&out);
    }
    let out = std::path::PathBuf::from(first);
    if args.next().is_some() {
        return Err("unexpected capture argument".into());
    }
    let c = sources::compiler("examples/gallery.ubi")?;
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

fn components(out: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    if !out.is_dir() {
        return Err("capture directory must already exist".into());
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ubi-ui");
    let catalog: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("catalog.json"))?)?;
    let entries = catalog.as_array().ok_or("catalog must be an array")?;
    let mut session = ubi_ui_platform::CaptureSession::new()?;
    for entry in entries {
        let slug = entry["slug"]
            .as_str()
            .filter(|s| {
                !s.is_empty()
                    && s.bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            })
            .ok_or("invalid component slug")?;
        let source = entry["examples"][0]["source"]
            .as_str()
            .ok_or("missing component example")?;
        let compiler = sources::compiler(source)?;
        let runtime = compiler.runtime().map_err(|e| format!("{slug}: {e:?}"))?;
        let app = NativeApplication::new(runtime, source)?;
        let preview = component_preview(app.view());
        for (mode, dark) in [("light", false), ("dark", true)] {
            session.write(
                &preview,
                &out.join(format!("{slug}-{mode}.png")),
                900,
                480,
                dark,
            )?;
        }
        println!(
            "{slug}: checked Ubi app, {} nodes, 2 GPU captures",
            app.view().nodes.len()
        );
    }
    println!(
        "{} native components captured on {}",
        entries.len(),
        session.adapter_name()
    );
    Ok(())
}

/// Keep the real component subtree and use the application's Window as a
/// centered preview stage. No browser component or synthetic control is drawn.
fn component_preview(view: &ubi_ui_platform::View) -> ubi_ui_platform::View {
    let mut preview = view.clone();
    if view.root == "demo" || !view.nodes.contains_key("demo") {
        return preview;
    }
    let mut keep = std::collections::BTreeSet::new();
    fn collect(
        view: &ubi_ui_platform::View,
        key: &str,
        keep: &mut std::collections::BTreeSet<String>,
    ) {
        keep.insert(key.into());
        for child in &view.nodes[key].children {
            collect(view, child, keep);
        }
    }
    collect(view, "demo", &mut keep);
    keep.insert(view.root.clone());
    preview.nodes.retain(|key, _| keep.contains(key));
    preview.order.retain(|key| keep.contains(key));
    let root = preview.nodes.get_mut(&view.root).unwrap();
    root.children = vec!["demo".into()];
    root.style.center = true;
    root.style.gap = 0.0;
    preview
}
