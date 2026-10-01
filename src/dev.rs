use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

struct State {
    revision: u64,
    modules: BTreeMap<String, String>,
    errors: Vec<String>,
}

impl State {
    fn rebuild(&mut self, root: &Path, entry: &str, stderr: &mut impl Write) -> Result<(), String> {
        self.revision += 1;
        self.modules.clear();
        self.errors.clear();
        match crate::cli::load_compiler(root, entry) {
            Ok(compiler) => {
                let result = compiler.build();
                for diagnostic in result.diagnostics {
                    let message = format!(
                        "{}:{}-{}: {:?}[{}]: {}",
                        diagnostic.primary.source_id,
                        diagnostic.primary.start,
                        diagnostic.primary.end,
                        diagnostic.severity,
                        diagnostic.code,
                        diagnostic.message
                    );
                    let _ = writeln!(stderr, "{message}");
                    if diagnostic.severity == ubi_lang::Severity::Error {
                        self.errors.push(message);
                    }
                }
                if self.errors.is_empty() {
                    self.modules = result.javascript.unwrap_or_default();
                }
            }
            Err(error) => {
                let _ = writeln!(stderr, "ubi: {error}");
                self.errors.push(error.clone());
                return Err(error);
            }
        }
        Ok(())
    }
}

pub(crate) fn serve(
    root: &Path,
    entry: &str,
    port: u16,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<u8, String> {
    let web = fs::canonicalize(root.join("web"))
        .map_err(|error| format!("cannot open web directory: {error}; create web/index.html"))?;
    if !web.starts_with(root) || asset(&web, "index.html").is_none() {
        return Err("web/index.html must be a public file within the project root".to_owned());
    }
    let mut watched = snapshot(root)?;
    let mut state = State {
        revision: 0,
        modules: BTreeMap::new(),
        errors: Vec::new(),
    };
    state.rebuild(root, entry, stderr)?;
    if !state.errors.is_empty() {
        return Ok(1);
    }
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|error| format!("cannot start dev server: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    writeln!(stdout, "http://127.0.0.1:{port}/")
        .and_then(|()| stdout.flush())
        .map_err(|error| error.to_string())?;
    let mut checked = Instant::now();
    let mut watch_failed = false;
    loop {
        if checked.elapsed() >= Duration::from_millis(500) {
            match snapshot(root) {
                Ok(next) if next != watched || watch_failed => {
                    watched = next;
                    watch_failed = false;
                    let _ = state.rebuild(root, entry, stderr);
                }
                Ok(_) => {}
                Err(error) => {
                    watch_failed = true;
                    if state.errors != [error.clone()] {
                        state.revision += 1;
                        state.modules.clear();
                        state.errors = vec![error.clone()];
                        let _ = writeln!(stderr, "ubi: {error}");
                    }
                }
            }
            checked = Instant::now();
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = respond(&mut stream, &web, port, &state);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20))
            }
            Err(error) => return Err(format!("dev server connection error: {error}")),
        }
    }
}

type Snapshot = BTreeMap<PathBuf, (u64, Option<SystemTime>)>;

fn snapshot(root: &Path) -> Result<Snapshot, String> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    let mut count = 0;
    while let Some(directory) = pending.pop() {
        for item in
            fs::read_dir(directory).map_err(|error| format!("cannot watch project: {error}"))?
        {
            let item = item.map_err(|error| error.to_string())?;
            let name = item.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || matches!(name.as_ref(), "target" | "node_modules") {
                continue;
            }
            count += 1;
            if count > 4096 {
                return Err("dev watch exceeds 4096-entry limit".to_owned());
            }
            let ty = item.file_type().map_err(|error| error.to_string())?;
            if ty.is_symlink() {
                continue;
            }
            let path = item.path();
            if ty.is_dir() {
                pending.push(path);
            } else if ty.is_file()
                && (path.extension().is_some_and(|ext| ext == "ubi")
                    || path.starts_with(root.join("web")))
            {
                let metadata = item.metadata().map_err(|error| error.to_string())?;
                files.insert(path, (metadata.len(), metadata.modified().ok()));
            }
        }
    }
    Ok(files)
}

fn decode_path(raw: &str) -> Option<String> {
    let mut bytes = Vec::new();
    let raw = raw.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'%' {
            let hex = std::str::from_utf8(raw.get(index + 1..index + 3)?).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            bytes.push(raw[index]);
            index += 1;
        }
    }
    let path = String::from_utf8(bytes).ok()?;
    if path.contains('\\')
        || path.contains(':')
        || path.chars().any(char::is_control)
        || path.split('/').any(|part| part.starts_with('.'))
    {
        return None;
    }
    Some(path)
}

fn asset(web: &Path, relative: &str) -> Option<(Vec<u8>, &'static str)> {
    if relative
        .split('/')
        .any(|part| part.is_empty() || part.starts_with('.'))
    {
        return None;
    }
    let path = fs::canonicalize(web.join(relative)).ok()?;
    if !path.starts_with(web)
        || !path.is_file()
        || path
            .strip_prefix(web)
            .ok()?
            .components()
            .any(|part| part.as_os_str().to_string_lossy().starts_with('.'))
    {
        return None;
    }
    let mime = match path.extension()?.to_str()? {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        _ => return None,
    };
    if fs::metadata(&path).ok()?.len() > 8 * 1024 * 1024 {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 8 * 1024 * 1024 {
        return None;
    }
    Some((bytes, mime))
}

fn respond(stream: &mut TcpStream, web: &Path, port: u16, state: &State) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(200)))?;
    stream.set_write_timeout(Some(Duration::from_millis(200)))?;
    let started = Instant::now();
    let mut request = Vec::new();
    let mut buffer = [0; 1024];
    while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
        let remaining = Duration::from_millis(500).saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return response(stream, 408, "text/plain", b"Request timeout", false);
        }
        stream.set_read_timeout(Some(remaining.min(Duration::from_millis(200))))?;
        let count = match stream.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(count) => count,
            Err(_) => return response(stream, 408, "text/plain", b"Request timeout", false),
        };
        request.extend_from_slice(&buffer[..count]);
        if request.len() > 8192 {
            return response(
                stream,
                431,
                "text/plain",
                b"Request headers too large",
                false,
            );
        }
        if started.elapsed() > Duration::from_millis(500) {
            return response(stream, 408, "text/plain", b"Request timeout", false);
        }
    }
    let Ok(request) = std::str::from_utf8(&request) else {
        return response(stream, 400, "text/plain", b"Invalid request", false);
    };
    let mut lines = request.split("\r\n");
    let parts: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
    if parts.len() != 3 || !matches!(parts[2], "HTTP/1.1" | "HTTP/1.0") {
        return response(stream, 400, "text/plain", b"Invalid request", false);
    }
    let head = parts[0] == "HEAD";
    if !matches!(parts[0], "GET" | "HEAD") {
        return response(stream, 405, "text/plain", b"GET or HEAD required", head);
    }
    let hosts: Vec<_> = lines
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.eq_ignore_ascii_case("host"))
        .map(|(_, value)| value.trim())
        .collect();
    if hosts.len() != 1
        || (hosts[0] != format!("127.0.0.1:{port}") && hosts[0] != format!("localhost:{port}"))
    {
        return response(stream, 403, "text/plain", b"Loopback Host required", head);
    }
    let raw = parts[1].split('?').next().unwrap_or("");
    let Some(path) =
        decode_path(raw).filter(|path| path.starts_with('/') && !path.starts_with("//"))
    else {
        return response(stream, 400, "text/plain", b"Invalid path", head);
    };
    if path == "/__ubi/status" {
        let data =
            serde_json::json!({"revision": state.revision, "errors": state.errors}).to_string();
        return response(stream, 200, "application/json", data.as_bytes(), head);
    }
    if path == "/__ubi/dev.js" {
        return response(stream, 200, "text/javascript", RELOAD.as_bytes(), head);
    }
    if let Some(module) = path.strip_prefix("/__ubi/modules/") {
        if !state.errors.is_empty() {
            return response(
                stream,
                503,
                "text/plain",
                b"Fix compilation errors before loading modules",
                head,
            );
        }
        return match state.modules.get(module) {
            Some(source) => response(
                stream,
                200,
                "text/javascript; charset=utf-8",
                source.as_bytes(),
                head,
            ),
            None => response(stream, 404, "text/plain", b"Module not found", head),
        };
    }
    let relative = if path == "/" {
        "index.html"
    } else {
        &path[1..]
    };
    match asset(web, relative) {
        Some((mut bytes, mime)) => {
            if relative == "index.html" {
                bytes.extend_from_slice(
                    b"\n<script type=\"module\" src=\"/__ubi/dev.js\"></script>",
                );
            }
            response(stream, 200, mime, &bytes, head)
        }
        None => response(stream, 404, "text/plain", b"Public asset not found", head),
    }
}

fn response(
    stream: &mut TcpStream,
    status: u16,
    mime: &str,
    bytes: &[u8],
    head: bool,
) -> std::io::Result<()> {
    write!(stream, "HTTP/1.1 {status} Response\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n", bytes.len())?;
    if !head {
        stream.write_all(bytes)?;
    }
    Ok(())
}

const RELOAD: &str = r#"
let revision;
const overlay = document.createElement('pre');
overlay.setAttribute('role', 'alert');
overlay.style.cssText = 'position:fixed;inset:16px;z-index:9999;overflow:auto;background:#201b19;color:#fff;padding:24px;border:3px solid #ffb49a;white-space:pre-wrap;font:14px/1.6 monospace';
async function poll() {
  try {
    const state = await fetch('/__ubi/status', {cache:'no-store'}).then(response => response.json());
    if (state.errors.length) {
      overlay.textContent = 'Ubi compilation failed\n\n' + state.errors.join('\n\n');
      if (!overlay.isConnected) document.body.append(overlay);
    } else {
      overlay.remove();
      if (revision !== undefined && state.revision !== revision) location.reload();
    }
    revision = state.revision;
  } catch { /* A stopped server does not erase the current page. */ }
  setTimeout(poll, 700);
}
poll();
"#;
