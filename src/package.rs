use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

const TEMPLATE: &[(&str, &str)] = &[
    ("ubi.json", include_str!("../examples/v1/ubi.json")),
    (".gitignore", include_str!("../examples/v1/.gitignore")),
    ("README.md", include_str!("../examples/v1/README.md")),
    ("src/main.ubi", include_str!("../examples/v1/src/main.ubi")),
    (
        "web/index.html",
        include_str!("../examples/v1/web/index.html"),
    ),
    ("web/app.js", include_str!("../examples/v1/web/app.js")),
    (
        "web/style.css",
        include_str!("../examples/v1/web/style.css"),
    ),
    (
        "web/storage.js",
        include_str!("../examples/v1/web/storage.js"),
    ),
    ("cli/run.mjs", include_str!("../examples/v1/cli/run.mjs")),
    (
        "desktop/package.json",
        include_str!("../examples/v1/desktop/package.json"),
    ),
    (
        "desktop/package-lock.json",
        include_str!("../examples/v1/desktop/package-lock.json"),
    ),
    (
        "desktop/electron.cjs",
        include_str!("../examples/v1/desktop/electron.cjs"),
    ),
    (
        "mobile/package.json",
        include_str!("../examples/v1/mobile/package.json"),
    ),
    (
        "mobile/package-lock.json",
        include_str!("../examples/v1/mobile/package-lock.json"),
    ),
    (
        "mobile/capacitor.config.json",
        include_str!("../examples/v1/mobile/capacitor.config.json"),
    ),
    (
        "mobile/native.mjs",
        include_str!("../examples/v1/mobile/native.mjs"),
    ),
    (
        "mobile/storage.mjs",
        include_str!("../examples/v1/mobile/storage.mjs"),
    ),
    (
        "mobile/native-storage.js",
        include_str!("../examples/v1/mobile/native-storage.js"),
    ),
];

/// Create the four-host reference app, refusing every existing template file.
pub(crate) fn init(destination: &Path) -> Result<(), String> {
    let destination = absolute(destination)?;
    reject_links(&destination)?;
    for (name, _) in TEMPLATE {
        let path = destination.join(name);
        reject_links(&path)?;
        if fs::symlink_metadata(&path).is_ok() {
            return Err(format!("init refuses existing file: {}", path.display()));
        }
        check_parents(&path)?;
    }
    for (name, contents) in TEMPLATE {
        let path = destination.join(name);
        fs::create_dir_all(path.parent().unwrap())
            .map_err(|error| format!("cannot create app directory: {error}"))?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
        file.write_all(contents.as_bytes())
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }
    Ok(())
}

/// Package already checked, freshly generated modules with the project's hosts.
pub(crate) fn package(
    root: &Path,
    entry: &str,
    target: &str,
    out_dir: &Path,
    modules: &BTreeMap<String, String>,
) -> Result<(), String> {
    if !crate::project::valid_target(target) {
        return Err(format!("unknown package target: {target}"));
    }
    if !crate::cli::valid_source_id(entry) {
        return Err("invalid package entry".to_owned());
    }
    let root =
        fs::canonicalize(root).map_err(|error| format!("cannot resolve package root: {error}"))?;
    let entry_module = format!("{}.mjs", entry.strip_suffix(".ubi").unwrap());
    let module_url = module_url(&entry_module);
    if !modules.contains_key(&entry_module) {
        return Err("compiled package entry is missing".to_owned());
    }
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let prefix = if target == "mobile" { "www/" } else { "" };
    for (name, contents) in modules {
        validate_relative(name)?;
        files.insert(
            format!("{prefix}modules/{name}"),
            contents.as_bytes().to_vec(),
        );
    }
    if target != "cli" {
        collect(&root, &root.join("web"), prefix, &mut files)?;
        for name in ["index.html", "app.js", "style.css", "storage.js"] {
            let key = format!("{prefix}{name}");
            if !files.contains_key(&key) {
                return Err(format!("missing web package asset: web/{name}"));
            }
        }
        let key = format!("{prefix}app.js");
        let app = String::from_utf8(files[&key].clone())
            .map_err(|_| "web/app.js must be UTF-8".to_owned())?;
        let app = app
            .replace(
                &format!("/__ubi/modules/{module_url}"),
                &format!("./modules/{module_url}"),
            )
            .replace(
                "/__ubi/modules/src/main.mjs",
                &format!("./modules/{module_url}"),
            );
        files.insert(key, app.into_bytes());
    }
    match target {
        "cli" => {
            let mut host = BTreeMap::new();
            collect(&root, &root.join("cli"), "", &mut host)?;
            let contents = host
                .get_mut("run.mjs")
                .ok_or("missing CLI host: cli/run.mjs")?;
            let text = String::from_utf8(contents.clone())
                .map_err(|_| "cli/run.mjs must be UTF-8".to_owned())?;
            *contents = text
                .replace("./modules/src/main.mjs", &format!("./modules/{module_url}"))
                .into_bytes();
            merge(&mut files, host)?;
        }
        "desktop" => {
            let mut host = BTreeMap::new();
            collect(&root, &root.join("desktop"), "", &mut host)?;
            for name in ["electron.cjs", "package.json", "package-lock.json"] {
                if !host.contains_key(name) {
                    return Err(format!("missing desktop host: desktop/{name}"));
                }
            }
            merge(&mut files, host)?;
        }
        "mobile" => {
            let mut host = BTreeMap::new();
            collect(&root, &root.join("mobile"), "", &mut host)?;
            for name in [
                "native.mjs",
                "capacitor.config.json",
                "package.json",
                "package-lock.json",
                "native-storage.js",
            ] {
                if !host.contains_key(name) {
                    return Err(format!("missing mobile host: mobile/{name}"));
                }
            }
            files.insert(
                "www/storage.js".to_owned(),
                host.remove("native-storage.js").unwrap(),
            );
            merge(&mut files, host)?;
        }
        _ => {}
    }
    let out_dir = absolute(out_dir)?;
    reject_links(&out_dir)?;
    // Complete input and destination validation before touching existing outputs.
    for name in files.keys() {
        validate_relative(name)?;
        let path = out_dir.join(name);
        reject_links(&path)?;
        check_parents(&path)?;
        match fs::symlink_metadata(&path) {
            Ok(metadata) if !metadata.is_file() => {
                return Err(format!(
                    "output path is not a regular file: {}",
                    path.display()
                ))
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot inspect {}: {error}", path.display())),
        }
    }
    for (name, contents) in files {
        let path = out_dir.join(name);
        fs::create_dir_all(path.parent().unwrap())
            .map_err(|error| format!("cannot create package directory: {error}"))?;
        crate::cli::replace_artifact(&path, &contents)?;
    }
    Ok(())
}

fn merge(
    files: &mut BTreeMap<String, Vec<u8>>,
    other: BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    for (name, contents) in other {
        if files.insert(name.clone(), contents).is_some() {
            return Err(format!(
                "host asset collides with generated artifact: {name}"
            ));
        }
    }
    Ok(())
}

fn collect(
    root: &Path,
    source: &Path,
    prefix: &str,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    reject_links(source)?;
    let canonical = fs::canonicalize(source)
        .map_err(|error| format!("cannot read package assets {}: {error}", source.display()))?;
    if !canonical.starts_with(root) {
        return Err(format!("asset escapes project root: {}", source.display()));
    }
    for entry in fs::read_dir(source).map_err(|error| format!("cannot read assets: {error}"))? {
        let entry = entry.map_err(|error| format!("cannot inspect asset: {error}"))?;
        let name = entry
            .file_name()
            .to_str()
            .ok_or("asset path must be UTF-8")?
            .to_owned();
        if (source == root.join("desktop") || source == root.join("mobile"))
            && matches!(
                name.as_str(),
                "node_modules" | "out" | "android" | "ios" | ".ubi-build" | ".ubi-dist"
            )
        {
            continue;
        }
        validate_relative(&name)?;
        let path = entry.path();
        reject_links(&path)?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("cannot inspect asset: {error}"))?;
        let key = format!("{prefix}{name}");
        if metadata.is_dir() {
            collect(root, &path, &format!("{key}/"), files)?;
        } else if metadata.is_file() {
            let data = fs::read(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            if files.insert(key.clone(), data).is_some() {
                return Err(format!("asset collides with generated artifact: {key}"));
            }
        } else {
            return Err(format!("asset is not a regular file: {}", path.display()));
        }
    }
    Ok(())
}

fn validate_relative(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.contains(['\\', ':'])
        || name.chars().any(char::is_control)
        || name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || !Path::new(name)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(format!("invalid package asset path: {name}"));
    }
    Ok(())
}

fn module_url(name: &str) -> String {
    let mut url = String::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
            url.push(char::from(byte));
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    url
}

fn absolute(path: &Path) -> Result<PathBuf, String> {
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    let mut result = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !result.pop() {
                    return Err("output path escapes filesystem root".to_owned());
                }
            }
            _ => result.push(part.as_os_str()),
        }
    }
    Ok(result)
}

fn reject_links(path: &Path) -> Result<(), String> {
    let mut current = PathBuf::new();
    for part in path.components() {
        current.push(part.as_os_str());
        if matches!(part, Component::Prefix(_)) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!("package refuses symlink: {}", current.display()))
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot inspect {}: {error}", current.display())),
        }
    }
    Ok(())
}

fn check_parents(path: &Path) -> Result<(), String> {
    for parent in path.ancestors().skip(1) {
        match fs::symlink_metadata(parent) {
            Ok(metadata) if !metadata.is_dir() => {
                return Err(format!(
                    "output parent is not a directory: {}",
                    parent.display()
                ))
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot inspect {}: {error}", parent.display())),
        }
    }
    Ok(())
}
