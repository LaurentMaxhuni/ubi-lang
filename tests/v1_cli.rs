//! Public independent SPEC section 22 CLI conformance.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "ubi-v1-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn write(&self, name: &str, text: &str) {
        let p = self.0.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ubi"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn version_reports_crate_version_and_run_awaits_async_results_and_faults() {
    let p = Project::new();
    let version = p.run(&["--version"]);
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains(env!("CARGO_PKG_VERSION")));
    p.write("main.ubi","async fn value() -> int { 42 } export async fn main() -> int { await value() } export async fn fault() -> int { let n = await value(); n / 0 }");
    let good = p.run(&["run", "main.ubi"]);
    assert!(
        good.status.success(),
        "{}",
        String::from_utf8_lossy(&good.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&good.stdout), "42\n");
    let bad = p.run(&["run", "main.ubi", "--function", "fault"]);
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("UBI-R0002"));
    assert!(bad.stdout.is_empty());
}

#[test]
fn fmt_checks_without_writes_then_formats_reachable_modules_idempotently() {
    let p = Project::new();
    let main = "import { value } from \"./lib.ubi\";export fn main()->int{value()}";
    let lib = "// kept\nexport fn value()->int{42}";
    p.write("main.ubi", main);
    p.write("lib.ubi", lib);
    p.write("unrelated.ubi", "not source");
    let check = p.run(&["fmt", "main.ubi", "--check"]);
    assert_eq!(check.status.code(), Some(1));
    assert_eq!(fs::read_to_string(p.0.join("main.ubi")).unwrap(), main);
    assert_eq!(fs::read_to_string(p.0.join("lib.ubi")).unwrap(), lib);
    let formatted = p.run(&["fmt", "main.ubi"]);
    assert!(
        formatted.status.success(),
        "{}",
        String::from_utf8_lossy(&formatted.stderr)
    );
    let canonical_main = fs::read(p.0.join("main.ubi")).unwrap();
    let canonical_lib = fs::read(p.0.join("lib.ubi")).unwrap();
    assert_ne!(canonical_main, main.as_bytes());
    assert_ne!(canonical_lib, lib.as_bytes());
    assert_eq!(
        fs::read_to_string(p.0.join("unrelated.ubi")).unwrap(),
        "not source"
    );
    assert!(p.run(&["fmt", "main.ubi", "--check"]).status.success());
    assert!(p.run(&["fmt", "main.ubi"]).status.success());
    assert_eq!(fs::read(p.0.join("main.ubi")).unwrap(), canonical_main);
    assert_eq!(fs::read(p.0.join("lib.ubi")).unwrap(), canonical_lib);
}

#[test]
fn fmt_invalid_reachable_syntax_or_missing_import_writes_no_source_changes() {
    for bad in [
        "export fn value( {",
        "import { absent } from \"./missing.ubi\";export fn value()->int{1}",
    ] {
        let p = Project::new();
        let main = "import { value } from \"./lib.ubi\";export fn main()->int{value()}";
        p.write("main.ubi", main);
        p.write("lib.ubi", bad);
        let output = p.run(&["fmt", "main.ubi"]);
        assert!(!output.status.success());
        assert_eq!(fs::read_to_string(p.0.join("main.ubi")).unwrap(), main);
        assert_eq!(fs::read_to_string(p.0.join("lib.ubi")).unwrap(), bad);
    }
}

#[test]
fn fmt_invalid_utf8_in_later_module_preserves_earlier_source() {
    let p = Project::new();
    let source = "import { value } from \"./lib.ubi\";export fn main()->int{value()}";
    p.write("main.ubi", source);
    fs::write(p.0.join("lib.ubi"), [0xff]).unwrap();
    assert!(!p.run(&["fmt", "main.ubi"]).status.success());
    assert_eq!(fs::read_to_string(p.0.join("main.ubi")).unwrap(), source);
    assert_eq!(fs::read(p.0.join("lib.ubi")).unwrap(), [0xff]);
}

#[test]
fn fmt_readonly_later_module_preserves_all_original_sources() {
    let p = Project::new();
    let main = "import { value } from \"./lib.ubi\";export fn main()->int{value()}";
    let lib = "export fn value()->int{1}";
    p.write("main.ubi", main);
    p.write("lib.ubi", lib);
    let path = p.0.join("lib.ubi");
    let original = fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let result = p.run(&["fmt", "main.ubi"]);
    fs::set_permissions(&path, original).unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(fs::read_to_string(p.0.join("main.ubi")).unwrap(), main);
    assert_eq!(fs::read_to_string(path).unwrap(), lib);
}

#[test]
fn package_compiler_errors_are_json_and_leave_existing_output_untouched() {
    let p = Project::new();
    p.write("main.ubi", "export fn main() -> int { missing }");
    p.write("dist/user.txt", "preserve");
    let output = p.run(&[
        "package",
        "main.ubi",
        "--target",
        "web",
        "--out-dir",
        "dist",
        "--json",
    ]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value.to_string().contains("UBI0010"));
    assert_eq!(
        fs::read_to_string(p.0.join("dist/user.txt")).unwrap(),
        "preserve"
    );
    assert_eq!(fs::read_dir(p.0.join("dist")).unwrap().count(), 1);
}
