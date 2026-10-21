//! Independent package acceptance for the source-owned Ubi UI foundation.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

struct OutputDir(PathBuf);
impl OutputDir {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "ubi-ui-package-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn package(&self, target: &str) -> PathBuf {
        let output = self.0.join(target);
        let result = Command::new(env!("CARGO_BIN_EXE_ubi"))
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args([
                "package",
                "--root",
                "examples/ui",
                "--target",
                target,
                "--out-dir",
            ])
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        output
    }
}
impl Drop for OutputDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn read(path: &Path, file: &str) -> String {
    fs::read_to_string(path.join(file)).unwrap()
}

#[test]
fn ui_packages_share_components_renderer_and_safe_offline_hosts() {
    let output = OutputDir::new();
    let mut reference = None;
    for target in ["web", "desktop", "mobile"] {
        let package = output.package(target);
        let assets = if target == "mobile" {
            package.join("www")
        } else {
            package.clone()
        };
        for asset in [
            "index.html",
            "style.css",
            "app.js",
            "codec.mjs",
            "validate.mjs",
            "renderer.mjs",
            "storage.js",
        ] {
            assert!(assets.join(asset).is_file(), "{target}: missing {asset}");
        }
        let module = read(&assets, "modules/src/main.mjs");
        if let Some(ref prior) = reference {
            assert_eq!(&module, prior);
        } else {
            reference = Some(module);
        }
        for name in ["core", "components"] {
            assert!(assets.join(format!("modules/src/ui/{name}.mjs")).is_file());
        }
        let app = read(&assets, "app.js");
        assert!(app.contains("./modules/src/main.mjs"));
        assert!(!app.contains("/__ubi/"));
        let html = read(&assets, "index.html");
        assert!(html.contains("script-src 'self'"));
        assert!(!html.contains("https://"));
        if target == "desktop" {
            let host = read(&package, "electron.cjs");
            for text in [
                "sandbox: true",
                "contextIsolation: true",
                "nodeIntegration: false",
                "webSecurity: true",
                "'renderer.mjs'",
                "'codec.mjs'",
                "'validate.mjs'",
                "action: 'deny'",
            ] {
                assert!(host.contains(text), "desktop missing {text}");
            }
            assert!(host.contains("!name.startsWith('modules/')"));
        }
        if target == "mobile" {
            let config: serde_json::Value =
                serde_json::from_str(&read(&package, "capacitor.config.json")).unwrap();
            assert_eq!(config["appId"], "org.ubilang.ui");
            assert_eq!(config["webDir"], "www");
        }
    }
}

#[test]
fn packaged_ui_runs_public_exports_through_the_real_renderer_contract() {
    let output = OutputDir::new();
    let package = output.package("web");
    fs::write(package.join("check.mjs"), r#"
import assert from 'node:assert/strict';
import * as app from './modules/src/main.mjs';
import {encode} from './codec.mjs';
import {validateView} from './validate.mjs';
const initial = app.initial();
assert.equal(initial.count, 0);
const result = app.update(encode(initial), encode({tag:'Activate',values:[{tag:'Increment',values:[]}]}));
assert.equal(result.tag, 'Ok'); assert.equal(result.values[0].count, 1); assert.equal(initial.count,0);
for (const section of ['buttons','composition']) for (const theme of ['Light','Dark','System']) {
  const state = {section,theme:{tag:theme,values:[]},count:3};
  assert.equal(app.validate(encode(state)).tag,'Ok');
  const view = validateView(app.view(encode(state)));
  assert.equal(view.root,'explorer'); assert.ok(view.nodes.length>10);
  assert.equal(view.nodes[0].props.Theme,theme);
}
assert.throws(()=>app.update(initial, {tag:'Activate',values:[]}), TypeError);
console.log('UBI_UI_PACKAGE_OK');
"#).unwrap();
    let result = Command::new("node")
        .arg(package.join("check.mjs"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("UBI_UI_PACKAGE_OK"));
}
