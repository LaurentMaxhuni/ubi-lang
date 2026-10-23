//! Reproducible documentation captures from the actual Ubi gallery and GPU host.
use serde_json::json;
use ubi_lang::ui::NativeApplication;
use ubi_ui_platform::{Application, Event};
mod sources;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let first = args
        .next()
        .ok_or("usage: native-ui-capture [--components|--motion] <existing-empty-directory>")?;
    if first == "--components" || first == "--motion" {
        let out = std::path::PathBuf::from(args.next().ok_or("missing capture directory")?);
        if args.next().is_some() {
            return Err("unexpected capture argument".into());
        }
        return if first == "--motion" {
            motion_samples(&out)
        } else {
            components(&out)
        };
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

/// Sample real source examples with the same deterministic presentation path
/// used by the desktop host. Logical state stays committed at every timestamp.
fn motion_samples(out: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    if !out.is_dir() {
        return Err("capture directory must already exist".into());
    }
    let mut session = ubi_ui_platform::CaptureSession::new()?;
    for (slug, key, repeats) in [
        ("switch", "demo", 1),
        ("number-ticker", "increment", 64),
        ("text-reveal", "replay", 1),
        ("animated-list", "reorder", 1),
        ("carousel", "highlights-next", 1),
    ] {
        let source = format!("examples/components/{slug}.ubi");
        let compiler = sources::compiler(&source)?;
        let mut app = NativeApplication::new(
            compiler.runtime().map_err(|e| format!("{slug}: {e:?}"))?,
            &source,
        )?;
        let from = app.view().clone();
        for _ in 0..repeats {
            let control = app.view().nodes.get(key).ok_or("missing sample action")?;
            let event = if let Some(message) = &control.activate {
                json!({"tag":"Activate","values":[message]})
            } else if let Some(message) = &control.toggle {
                json!({"tag":"Toggle","values":[message,!control.checked]})
            } else {
                return Err(format!("{slug}: missing sample binding").into());
            };
            app.dispatch(&Event {
                key: key.into(),
                value: event,
            })?;
        }
        for time in [0, 110, 440] {
            session.write_at(
                &app,
                &from,
                &out.join(format!("{slug}-{time:03}ms.png")),
                900,
                480,
                true,
                time,
                ubi_ui_platform::MotionPreference::Full,
            )?;
        }
        println!("{slug}: checked source, sampled at 0/110/440ms");
    }
    println!(
        "Native motion samples captured on {}",
        session.adapter_name()
    );
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
        let mut app = NativeApplication::new(runtime, source)?;
        if app
            .view()
            .nodes
            .values()
            .any(|node| node.overlay_kind.is_some() && !node.open)
        {
            let (key, message) = app
                .view()
                .order
                .iter()
                .find_map(|key| {
                    app.view().nodes[key]
                        .activate
                        .as_ref()
                        .map(|message| (key.clone(), message.clone()))
                })
                .ok_or("overlay example needs an explicit opening action")?;
            app.dispatch(&Event {
                key,
                value: json!({"tag":"Activate","values":[message]}),
            })?;
            if !app
                .view()
                .nodes
                .values()
                .any(|node| node.overlay_kind.is_some() && node.open)
            {
                return Err(format!("{slug}: opening action did not open the overlay").into());
            }
        }
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
