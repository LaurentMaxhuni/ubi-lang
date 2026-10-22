//! Export the checked Ubi examples as portable, standalone browser applications.
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Component, Path};

pub(crate) mod sources;

fn main() {
    let destination = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "ubi-ui/docs/public/previews".into());
    match export(Path::new(&destination)) {
        Ok(count) => println!("Exported {count} checked Ubi browser applications to {destination}"),
        Err(error) => {
            eprintln!("Ubi web export: {error}");
            std::process::exit(1);
        }
    }
}

pub fn export(destination: &Path) -> Result<usize, String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let catalog: Vec<Value> =
        serde_json::from_str(include_str!("../catalog.json")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    if std::fs::symlink_metadata(destination)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("output directory must not be a symlink".into());
    }
    let mut entries = Vec::new();
    for component in catalog {
        entries.push((
            component["slug"]
                .as_str()
                .ok_or("missing component slug")?
                .to_owned(),
            component["name"]
                .as_str()
                .ok_or("missing component name")?
                .to_owned(),
            component["examples"][0]["source"]
                .as_str()
                .ok_or("missing component example")?
                .to_owned(),
        ));
    }
    entries.push((
        "gallery".into(),
        "Ubi UI gallery".into(),
        "examples/gallery.ubi".into(),
    ));
    let mut manifest = Vec::new();
    let mut used = BTreeSet::new();
    for (slug, name, entry) in entries {
        if slug.is_empty()
            || !slug
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || !used.insert(slug.clone())
        {
            return Err(format!("invalid or duplicate application slug: {slug}"));
        }
        let compiler = sources::compiler(&entry)?;
        let build = compiler.build();
        let files = build
            .javascript
            .ok_or_else(|| format!("{entry}: {:?}", build.diagnostics))?;
        // Every app gets its own checked type environment. Copy only reachable
        // modules, avoiding unrelated components and cross-app type collisions.
        let mut pending = vec![entry.clone()];
        let mut reachable = BTreeSet::new();
        while let Some(module) = pending.pop() {
            if !reachable.insert(module.clone()) {
                continue;
            }
            for import in build
                .imports
                .iter()
                .filter(|import| import.importer_id == module)
            {
                pending.push(
                    import
                        .target_id
                        .clone()
                        .ok_or("unresolved checked import")?,
                );
            }
        }
        for module in reachable {
            let file = format!(
                "{}.mjs",
                module
                    .strip_suffix(".ubi")
                    .ok_or("invalid source extension")?
            );
            write(
                destination,
                &format!("{slug}/modules/{file}"),
                files.get(&file).ok_or("missing emitted module")?.as_bytes(),
            )?;
        }
        let module = format!("{}.mjs", entry.strip_suffix(".ubi").ok_or("invalid entry")?);
        write(
            destination,
            &format!("{slug}/app.mjs"),
            format!("export {{ init, update, view }} from \"./modules/{module}\";\n").as_bytes(),
        )?;
        let html = format!(
            r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="light dark"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self'; connect-src 'none'; base-uri 'none'; form-action 'none'"><title>{name} — Ubi UI</title><link rel="stylesheet" href="../runtime/renderer.css"><link rel="stylesheet" href="../runtime/shell.css"></head><body><main id="app" data-slug="{slug}" aria-label="{name} example"></main><p id="error" role="alert" hidden></p><script type="module" src="../runtime/boot.mjs"></script></body></html>
"#,
            name = escape(&name)
        );
        write(destination, &format!("{slug}/index.html"), html.as_bytes())?;
        manifest.push(
            json!({"slug":slug,"name":name,"source":entry,"module":format!("{slug}/app.mjs")}),
        );
    }
    for file in ["renderer.mjs", "validate.mjs", "renderer.css"] {
        write(
            destination,
            &format!("runtime/{file}"),
            &std::fs::read(repository.join("ubi-ui/web").join(file))
                .map_err(|e| format!("{file}: {e}"))?,
        )?;
    }
    write(
        destination,
        "runtime/codec.mjs",
        include_bytes!("../../examples/ui/web/codec.mjs"),
    )?;
    write(
        destination,
        "runtime/boot.mjs",
        include_bytes!("web_boot.mjs"),
    )?;
    write(
        destination,
        "runtime/shell.css",
        include_bytes!("web_shell.css"),
    )?;
    write(
        destination,
        "runtime/Inter.ttf",
        include_bytes!("../assets/Inter.ttf"),
    )?;
    write(
        destination,
        "runtime/OFL.txt",
        include_bytes!("../assets/OFL.txt"),
    )?;
    write(
        destination,
        "manifest.json",
        serde_json::to_string_pretty(&manifest)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )?;
    let links = manifest
        .iter()
        .map(|app| {
            format!(
                "<li><a href=\"./{}/index.html\">{}</a></li>",
                app["slug"].as_str().unwrap(),
                escape(app["name"].as_str().unwrap())
            )
        })
        .collect::<String>();
    write(destination, "index.html", format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Ubi UI browser examples</title><h1>Ubi UI browser examples</h1><p>Compiled from the same Ubi source used by the native desktop host.</p><ul>{links}</ul></html>").as_bytes())?;
    Ok(manifest.len())
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn write(root: &Path, relative: &str, data: &[u8]) -> Result<(), String> {
    let mut path = root.to_owned();
    let components: Vec<_> = Path::new(relative).components().collect();
    for (index, part) in components.iter().enumerate() {
        let Component::Normal(part) = part else {
            return Err("invalid output path".into());
        };
        path.push(part);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("output path contains a symlink".into())
            }
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(error.to_string())
            }
            _ => {}
        }
        if index + 1 < components.len() {
            std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(path, data).map_err(|e| e.to_string())
}
