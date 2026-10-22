//! Source package loader shared by checked examples, captures and benchmarks.
use std::path::{Path, PathBuf};
use ubi_lang::Compiler;

pub fn compiler(entry: &str) -> Result<Compiler, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("ubi-ui");
    let mut compiler = Compiler::new();
    let mut paths = Vec::new();
    collect(&root.join("packages/ubi-ui"), &mut paths)?;
    paths.sort();
    for path in paths {
        let id = path
            .strip_prefix(&root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        compiler
            .add_source(id, std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("{e:?}"))?;
    }
    let entry_path = root.join(entry).canonicalize().map_err(|e| e.to_string())?;
    if !entry_path.starts_with(root.canonicalize().map_err(|e| e.to_string())?) {
        return Err("example escapes Ubi UI root".into());
    }
    compiler
        .add_source(entry, std::fs::read(entry_path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("{e:?}"))?;
    Ok(compiler)
}
fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        if ty.is_symlink() {
            return Err("source package contains a symlink".into());
        }
        if ty.is_dir() {
            collect(&entry.path(), out)?;
        } else if ty.is_file() && entry.path().extension().is_some_and(|x| x == "ubi") {
            out.push(entry.path());
        }
    }
    Ok(())
}
