//! Independent public contracts for project scaffolding aliases.

use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "ubi-scaffold-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn run(&self, arguments: &[&str]) -> Output {
        run(&self.0, arguments)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ubi"))
        .current_dir(root)
        .args(arguments)
        .output()
        .unwrap()
}

fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(root: &Path, directory: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(root, &path, result);
            } else {
                result.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    collect(root, root, &mut result);
    result
}

#[test]
fn new_and_init_create_identical_complete_four_target_starters_and_next_steps() {
    let project = Project::new();
    let destinations = ["new app & data; keep", "init app & data; keep"];
    let mut snapshots = Vec::new();
    for (command, destination) in ["new", "init"].into_iter().zip(destinations) {
        let output = project.run(&[command, destination]);
        assert_eq!(output.status.code(), Some(0), "{command}: {output:?}");
        assert!(output.stderr.is_empty());
        let instructions = String::from_utf8(output.stdout).unwrap();
        assert!(instructions.contains(destination), "{instructions}");
        for step in [
            "ubi dev",
            "ubi check --target web",
            "ubi package --target cli",
        ] {
            assert!(
                instructions.contains(step),
                "missing {step}: {instructions}"
            );
        }
        let root = project.0.join(destination);
        let snapshot = files(&root);
        for path in [
            "ubi.json",
            "README.md",
            ".gitignore",
            "src/main.ubi",
            "web/index.html",
            "web/app.js",
            "web/style.css",
            "web/storage.js",
            "cli/run.mjs",
            "desktop/package.json",
            "desktop/package-lock.json",
            "desktop/electron.cjs",
            "mobile/package.json",
            "mobile/package-lock.json",
            "mobile/capacitor.config.json",
            "mobile/native.mjs",
            "mobile/storage.mjs",
            "mobile/native-storage.js",
        ] {
            assert!(
                snapshot.contains_key(Path::new(path)),
                "missing {command}/{path}"
            );
        }
        let config: Value = serde_json::from_slice(&snapshot[Path::new("ubi.json")]).unwrap();
        let mut targets: Vec<_> = config["targets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|target| target.as_str().unwrap())
            .collect();
        targets.sort();
        assert_eq!(targets, ["cli", "desktop", "mobile", "web"]);
        assert_eq!(config["entry"], "src/main.ubi");
        for implicit in [".git", "node_modules", ".ubi-build", ".ubi-dist"] {
            assert!(
                !root.join(implicit).exists(),
                "unexpected implicit operation: {implicit}"
            );
        }
        let checked = run(&root, &["check", "--target", "web", "--json"]);
        assert_eq!(checked.status.code(), Some(0), "{command}: {checked:?}");
        let diagnostics: Value = serde_json::from_slice(&checked.stdout).unwrap();
        assert_eq!(diagnostics["diagnostics"], serde_json::json!([]));
        assert_eq!(files(&root), snapshot);
        snapshots.push(snapshot);
    }
    assert_eq!(snapshots[0], snapshots[1]);
    assert_eq!(fs::read_dir(&project.0).unwrap().count(), 2);
}

#[test]
fn both_aliases_refuse_late_template_collisions_without_clobber_or_partial_creation() {
    for command in ["new", "init"] {
        let project = Project::new();
        fs::create_dir_all(project.0.join("existing app/web")).unwrap();
        fs::write(project.0.join("existing app/web/app.js"), "user source").unwrap();
        fs::write(
            project.0.join("existing app/keep.txt"),
            "unrelated user data",
        )
        .unwrap();
        let before = files(&project.0);
        let output = project.run(&[command, "existing app"]);
        assert_eq!(output.status.code(), Some(2), "{command}: {output:?}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        assert_eq!(files(&project.0), before);
        assert!(!project.0.join("existing app/ubi.json").exists());
        assert!(!project.0.join("existing app/src").exists());
    }
}

#[test]
fn missing_extra_and_unsupported_scaffold_arguments_create_nothing() {
    for command in ["new", "init"] {
        let project = Project::new();
        for arguments in [
            vec![command],
            vec![command, "would create", "extra"],
            vec![command, "would create", "--json"],
            vec![command, "would create", "--target", "web"],
            vec![command, "would create", "--root", "."],
            vec![command, "--apply"],
        ] {
            let output = project.run(&arguments);
            assert_eq!(output.status.code(), Some(2), "{arguments:?}: {output:?}");
            assert!(output.stdout.is_empty());
            assert!(!output.stderr.is_empty());
            assert_eq!(
                fs::read_dir(&project.0).unwrap().count(),
                0,
                "{arguments:?}"
            );
        }
    }
}
