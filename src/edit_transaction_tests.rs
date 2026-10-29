//! Independently authored contracts for edit pre-install guards and cleanup.

use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("ubi-edit-guard-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }

    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const MAIN: &str = "import { value } from \"./dep.ubi\"; export fn main() -> int { value() }";
const DEP: &str = "export fn value() -> int { 1 }";

fn snapshot(root: &Path) -> (Vec<ubi_lang::SourceRevision>, Vec<(String, PathBuf)>) {
    let compiler = load_compiler(root, "main.ubi").unwrap();
    let revisions = compiler.source_revisions();
    let paths = revisions
        .iter()
        .map(|source| {
            (
                source.id.clone(),
                fs::canonicalize(root.join(&source.id)).unwrap(),
            )
        })
        .collect();
    (revisions, paths)
}

fn proposed_change(root: &Path) -> [FormatChange; 1] {
    [FormatChange {
        path: root.join("main.ubi"),
        original: MAIN.as_bytes().to_vec(),
        formatted: MAIN.replace("{ value() }", "{ 42 }"),
        staged: None,
        backup: None,
    }]
}

#[test]
fn edit_guard_rejects_changed_dependency_after_staging_and_cleans_temporary_files() {
    let project = TempDir::new();
    project.write("main.ubi", MAIN);
    project.write("dep.ubi", DEP);
    let (revisions, paths) = snapshot(&project.0);
    let mut changes = proposed_change(&project.0);
    let external = "export fn value() -> int { 9 }";
    let result = replace_sources(&mut changes, || {
        assert!(project.names().iter().any(|name| name.ends_with(".stage")));
        project.write("dep.ubi", external);
        recheck_edit_sources(&project.0, "main.ubi", &revisions, &paths)
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read(project.0.join("main.ubi")).unwrap(),
        MAIN.as_bytes()
    );
    assert_eq!(
        fs::read(project.0.join("dep.ubi")).unwrap(),
        external.as_bytes()
    );
    assert_eq!(project.names(), ["dep.ubi", "main.ubi"]);
    assert!(changes[0].staged.is_none());
    assert!(changes[0].backup.is_none());
}

#[test]
fn edit_guard_preserves_external_target_edit_instead_of_overwriting_it() {
    let project = TempDir::new();
    project.write("main.ubi", MAIN);
    project.write("dep.ubi", DEP);
    let (revisions, paths) = snapshot(&project.0);
    let mut changes = proposed_change(&project.0);
    let external = MAIN.replace("{ value() }", "{ 73 }");
    let result = replace_sources(&mut changes, || {
        project.write("main.ubi", &external);
        recheck_edit_sources(&project.0, "main.ubi", &revisions, &paths)
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read(project.0.join("main.ubi")).unwrap(),
        external.as_bytes()
    );
    assert_eq!(fs::read(project.0.join("dep.ubi")).unwrap(), DEP.as_bytes());
    assert_eq!(project.names(), ["dep.ubi", "main.ubi"]);
}

#[test]
fn edit_guard_rejects_missing_dependency_and_preserves_external_deletion() {
    let project = TempDir::new();
    project.write("main.ubi", MAIN);
    project.write("dep.ubi", DEP);
    let (revisions, paths) = snapshot(&project.0);
    let mut changes = proposed_change(&project.0);
    let result = replace_sources(&mut changes, || {
        fs::remove_file(project.0.join("dep.ubi")).unwrap();
        recheck_edit_sources(&project.0, "main.ubi", &revisions, &paths)
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read(project.0.join("main.ubi")).unwrap(),
        MAIN.as_bytes()
    );
    assert_eq!(project.names(), ["main.ubi"]);
}

#[test]
fn edit_guard_failure_before_backup_preserves_every_original_file() {
    let project = TempDir::new();
    project.write("main.ubi", MAIN);
    project.write("dep.ubi", DEP);
    let mut changes = proposed_change(&project.0);
    let result = replace_sources(&mut changes, || {
        Err("guard deliberately rejected replacement".into())
    });
    assert!(result
        .unwrap_err()
        .contains("guard deliberately rejected replacement"));
    assert_eq!(
        fs::read(project.0.join("main.ubi")).unwrap(),
        MAIN.as_bytes()
    );
    assert_eq!(fs::read(project.0.join("dep.ubi")).unwrap(), DEP.as_bytes());
    assert_eq!(project.names(), ["dep.ubi", "main.ubi"]);
}

#[test]
fn edit_guard_rejects_symlink_path_drift_even_when_source_bytes_are_identical() {
    let project = TempDir::new();
    project.write("main.ubi", MAIN);
    project.write("a.ubi", DEP);
    project.write("b.ubi", DEP);
    let link = project.0.join("dep.ubi");
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(project.0.join("a.ubi"), &link);
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_file(project.0.join("a.ubi"), &link);
    if let Err(error) = linked {
        #[cfg(windows)]
        if error.raw_os_error() == Some(1314) {
            eprintln!("symlink guard unavailable: Windows privilege 1314");
            return;
        }
        panic!("cannot create symlink fixture: {error}");
    }
    let (revisions, paths) = snapshot(&project.0);
    let mut changes = proposed_change(&project.0);
    let result = replace_sources(&mut changes, || {
        fs::remove_file(&link).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(project.0.join("b.ubi"), &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(project.0.join("b.ubi"), &link).unwrap();
        recheck_edit_sources(&project.0, "main.ubi", &revisions, &paths)
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read(project.0.join("main.ubi")).unwrap(),
        MAIN.as_bytes()
    );
    assert_eq!(fs::canonicalize(&link).unwrap(), project.0.join("b.ubi"));
    assert_eq!(project.names(), ["a.ubi", "b.ubi", "dep.ubi", "main.ubi"]);
}
