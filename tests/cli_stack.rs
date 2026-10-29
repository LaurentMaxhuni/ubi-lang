//! Process-level parser-budget regression, independent of edit preparation.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("ubi-cli-stack-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cli_parser_reports_nesting_budget_instead_of_overflowing_process_stack() {
    let project = TempDir::new();
    for (depth, expected_status) in [(4, 0), (40, 1)] {
        let source = format!(
            "export fn main() -> int {{ {}1{} }}",
            "if (true) { ".repeat(depth),
            " } else { 1 }".repeat(depth)
        );
        fs::write(project.0.join("main.ubi"), &source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_ubi"))
            .current_dir(&project.0)
            .args(["check", "main.ubi", "--json"])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(expected_status),
            "depth {depth}: {output:?}"
        );
        assert!(output.stderr.is_empty(), "depth {depth}: {output:?}");
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["schemaVersion"], 1);
        assert_eq!(report["sources"][0]["id"], "main.ubi");
        let diagnostics = report["diagnostics"].as_array().unwrap();
        if expected_status == 0 {
            assert!(diagnostics.is_empty());
        } else {
            assert!(diagnostics
                .iter()
                .any(|diagnostic| diagnostic["code"] == "UBI0090"));
        }
        assert_eq!(
            fs::read(project.0.join("main.ubi")).unwrap(),
            source.as_bytes()
        );
        assert!(!project.0.join(".ubi-build").exists());
    }
}
