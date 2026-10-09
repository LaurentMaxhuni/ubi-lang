//! Public independent SPEC section 22 portable-package and durable CLI conformance.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "ubi-v1-package-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ubi"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
    fn init(&self) {
        let out = self.run(&["init", "app"]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    fn package(&self, target: &str) {
        let out = Command::new(env!("CARGO_BIN_EXE_ubi"))
            .current_dir(self.0.join("app"))
            .args(["package", "--target", target])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    fn cli(&self, args: &[&str]) -> Output {
        Command::new("node")
            .arg(self.0.join("app/.ubi-dist/cli/run.mjs"))
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
fn init_preflights_existing_files_without_clobber_or_partial_creation() {
    let p = Project::new();
    fs::create_dir_all(p.0.join("app")).unwrap();
    fs::write(p.0.join("app/ubi.json"), "user config").unwrap();
    fs::write(p.0.join("app/keep.txt"), "unrelated").unwrap();
    let out = p.run(&["init", "app"]);
    assert!(!out.status.success());
    assert_eq!(
        fs::read_to_string(p.0.join("app/ubi.json")).unwrap(),
        "user config"
    );
    assert_eq!(fs::read_dir(p.0.join("app")).unwrap().count(), 2);
}

#[test]
fn all_four_packages_have_modules_assets_and_safe_host_configurations() {
    let p = Project::new();
    p.init();
    for target in ["web", "cli", "desktop", "mobile"] {
        p.package(target);
        let out = p.0.join("app/.ubi-dist").join(target);
        let assets = if target == "mobile" {
            out.join("www")
        } else {
            out.clone()
        };
        assert!(assets.join("modules/src/main.mjs").is_file());
        if target == "cli" {
            assert!(out.join("run.mjs").is_file());
            continue;
        }
        for asset in ["index.html", "app.js", "style.css", "storage.js"] {
            assert!(assets.join(asset).is_file(), "missing {target}/{asset}");
        }
        let app = fs::read_to_string(assets.join("app.js")).unwrap();
        assert!(!app.contains("/__ubi/"));
        assert!(app.contains("./modules/"));
        if target == "desktop" {
            let host = fs::read_to_string(out.join("electron.cjs")).unwrap();
            assert!(
                host.contains("nodeIntegration: false") || host.contains("nodeIntegration:false")
            );
            assert!(
                host.contains("contextIsolation: true") || host.contains("contextIsolation:true")
            );
            assert!(out.join("package.json").is_file());
            assert!(out.join("package-lock.json").is_file());
        }
        if target == "mobile" {
            let config: serde_json::Value =
                serde_json::from_slice(&fs::read(out.join("capacitor.config.json")).unwrap())
                    .unwrap();
            assert!(config.get("server").and_then(|s| s.get("url")).is_none());
            assert!(out.join("native.mjs").is_file());
            assert!(out.join("package-lock.json").is_file());
        }
    }
}

#[test]
fn packaged_cli_creates_validates_completes_filters_deletes_and_reloads_tasks() {
    let p = Project::new();
    p.init();
    p.package("cli");
    for args in [
        &["add", "write tests"][..],
        &["add", "ship app"][..],
        &["complete", "1"][..],
    ] {
        let out = p.cli(args);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let active = p.cli(&["list", "active"]);
    assert!(active.status.success());
    let active = String::from_utf8_lossy(&active.stdout);
    assert!(active.contains("ship app"));
    assert!(!active.contains("write tests"));
    let completed = p.cli(&["list", "completed"]);
    assert!(completed.status.success());
    assert!(String::from_utf8_lossy(&completed.stdout).contains("write tests"));
    let before = fs::read(p.0.join("app/.ubi-dist/cli/tasks.json")).unwrap();
    assert!(!p.cli(&["add", "   "]).status.success());
    assert_eq!(
        fs::read(p.0.join("app/.ubi-dist/cli/tasks.json")).unwrap(),
        before
    );
    assert!(p.cli(&["delete", "1"]).status.success());
    let all = p.cli(&["list"]);
    assert!(all.status.success());
    assert!(!String::from_utf8_lossy(&all.stdout).contains("write tests"));
    assert!(String::from_utf8_lossy(&all.stdout).contains("ship app"));
}

#[test]
fn corrupt_or_invalid_persistence_is_preserved_and_reported() {
    let p = Project::new();
    p.init();
    p.package("cli");
    let store = p.0.join("app/.ubi-dist/cli/tasks.json");
    for text in ["{","null","[{}]","[{\"id\":1,\"title\":\"\",\"completed\":false}]","[{\"id\":1,\"title\":\"ok\",\"completed\":false},{\"id\":1,\"title\":\"duplicate\",\"completed\":true}]"] {
        fs::write(&store,text).unwrap();let result=p.cli(&["add","keep data"]);assert!(!result.status.success(),"corruption accepted: {text}");assert_eq!(fs::read_to_string(&store).unwrap(),text);assert!(!result.stderr.is_empty());
    }
}

#[test]
fn failed_persistence_leaves_existing_store_untouched() {
    let p = Project::new();
    p.init();
    p.package("cli");
    let store = p.0.join("app/.ubi-dist/cli/tasks.json");
    fs::create_dir(&store).unwrap();
    fs::write(store.join("keep"), "preserve").unwrap();
    let result = p.cli(&["add", "cannot persist"]);
    assert!(!result.status.success());
    assert_eq!(fs::read_to_string(store.join("keep")).unwrap(), "preserve");
    assert!(store.is_dir());
}

#[test]
fn web_failed_writes_and_corrupt_reload_preserve_visible_and_persisted_tasks() {
    let p = Project::new();
    p.init();
    p.package("web");
    let web = p.0.join("app/.ubi-dist/web");
    // Exercise shipped host logic with controllable platform storage and minimal DOM.
    // Real browser accessibility/layout/native launch checks remain separate.
    fs::write(web.join("package.json"), "{\"type\":\"module\"}").unwrap();
    fs::write(web.join("conformance.mjs"),r#"
import assert from 'node:assert/strict';
class Element extends EventTarget {
  constructor(){super();this.children=[];this.hidden=false;this.disabled=false;this.value='';this.textContent='';this.classList={toggle(){}};}
  append(...items){this.children.push(...items);} replaceChildren(...items){this.children=items;}
  querySelector(){return submit;} setAttribute(){} focus(){}
}
const submit=new Element();const nodes=new Map(['#add-task','#title','#tasks','#error','#filter','#empty','#status','#reload'].map(key=>[key,new Element()]));nodes.get('#filter').value='all';
globalThis.document=new EventTarget();document.querySelector=key=>nodes.get(key);document.createElement=()=>new Element();document.hidden=false;globalThis.window=new EventTarget();
let saved=JSON.stringify([{id:1,title:'keep task',completed:false}]);let fail=false;
globalThis.localStorage={getItem:()=>saved,setItem:(key,value)=>{if(fail)throw Error('disk full');saved=value;}};
await import('./app.js');
const titles=()=>nodes.get('#tasks').children.map(row=>row.children[0].textContent);
const settle=async()=>{await new Promise(r=>setImmediate(r));await new Promise(r=>setImmediate(r));};
assert.deepEqual(titles(),['keep task']);const before=saved;
fail=true;nodes.get('#title').value='new task';nodes.get('#add-task').dispatchEvent(new Event('submit',{cancelable:true}));await settle();
assert.equal(saved,before);assert.deepEqual(titles(),['keep task']);assert.equal(nodes.get('#error').hidden,false);assert.equal(nodes.get('#title').value,'new task');
saved='{';nodes.get('#reload').dispatchEvent(new Event('click'));await settle();assert.equal(saved,'{');assert.deepEqual(titles(),['keep task']);assert.equal(nodes.get('#error').hidden,false);assert.equal(submit.disabled,true);
fail=false;saved=JSON.stringify([{id:1,title:'resumed task',completed:true}]);window.dispatchEvent(new Event('pageshow'));await settle();assert.deepEqual(titles(),['resumed task']);assert.equal(nodes.get('#error').hidden,true);assert.equal(submit.disabled,false);
"#).unwrap();
    let output = Command::new("node")
        .arg(web.join("conformance.mjs"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn packaging_preserves_unrelated_output_files_and_rejects_unknown_target() {
    let p = Project::new();
    p.init();
    let dist = p.0.join("app/.ubi-dist/web");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("keep.txt"), "user data").unwrap();
    p.package("web");
    assert_eq!(
        fs::read_to_string(dist.join("keep.txt")).unwrap(),
        "user data"
    );
    let out = p.run(&["package", "--target", "unsupported"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn public_assets_are_copied_with_relative_paths_and_output_override() {
    let p = Project::new();
    p.init();
    let root = p.0.join("app");
    fs::create_dir_all(root.join("web/assets")).unwrap();
    fs::write(root.join("web/assets/space name.txt"), "asset bytes").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_ubi"))
        .current_dir(&root)
        .args(["package", "--target", "web", "--out-dir", "../custom"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        fs::read_to_string(p.0.join("custom/assets/space name.txt")).unwrap(),
        "asset bytes"
    );
    assert!(p.0.join("custom/modules/src/main.mjs").is_file());
}

#[test]
fn public_asset_symlink_escape_cannot_be_packaged() {
    let p = Project::new();
    p.init();
    fs::write(p.0.join("outside.txt"), "private").unwrap();
    let link = p.0.join("app/web/escape.txt");
    #[cfg(windows)]
    let result = std::os::windows::fs::symlink_file(p.0.join("outside.txt"), &link);
    #[cfg(unix)]
    let result = std::os::unix::fs::symlink(p.0.join("outside.txt"), &link);
    if let Err(error) = result {
        eprintln!("asset symlink check unavailable: {error}");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_ubi"))
        .current_dir(p.0.join("app"))
        .args(["package", "--target", "web"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!p.0.join("app/.ubi-dist/web/escape.txt").exists());
    assert_eq!(
        fs::read_to_string(p.0.join("outside.txt")).unwrap(),
        "private"
    );
}
