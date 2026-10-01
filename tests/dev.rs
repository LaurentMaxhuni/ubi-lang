//! Dev-server expectations derived independently from SPEC.md section 10.
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "ubi-dev-tests-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        let project = Self(path);
        project.write("ubi.json", r#"{"schemaVersion":1,"name":"dev test","entry":"src/main.ubi","targets":["web","cli"]}"#);
        project.write(
            "src/main.ubi",
            "import { number } from \"./lib.ubi\"; export fn main() -> int { number() }",
        );
        project.write("src/lib.ubi", "export fn number() -> int { 41 }");
        project.write("web/index.html", "<!doctype html><html><body><h1>Dev fixture</h1><script type=\"module\">import { main } from '/__ubi/modules/src/main.mjs';</script></body></html>");
        project
    }

    fn write(&self, name: &str, text: &str) {
        let file = self.0.join(name);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Server {
    child: Child,
    address: SocketAddr,
}

impl Server {
    fn start(project: &Project, options: &[&str], no_node: bool) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ubi"));
        command
            .current_dir(&project.0)
            .args(["dev", "--port", "0"])
            .args(options)
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if no_node {
            command.env("PATH", "");
        }
        let mut server = Self {
            child: command.spawn().unwrap(),
            address: "127.0.0.1:0".parse().unwrap(),
        };
        let stdout = server.child.stdout.take().unwrap();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut announced = false;
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Some(url) = line
                    .split_whitespace()
                    .find(|word| word.starts_with("http://"))
                {
                    if !announced {
                        let _ = sender.send(url.to_owned());
                        announced = true;
                    }
                }
            }
        });
        let url = receiver
            .recv_timeout(Duration::from_secs(8))
            .expect("dev must print startup URL within eight seconds");
        server.address = url
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .parse()
            .unwrap();
        assert_eq!(server.address.ip().to_string(), "127.0.0.1");
        assert_ne!(server.address.port(), 0);
        server
    }

    fn request(&self, method: &str, path: &str) -> Response {
        self.raw(&format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            self.address
        ))
        .expect("server must return HTTP response")
    }

    fn raw(&self, request: &str) -> Option<Response> {
        let mut stream = TcpStream::connect_timeout(&self.address, Duration::from_secs(2)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        if stream.write_all(request.as_bytes()).is_err() {
            return None;
        }
        let mut bytes = Vec::new();
        match stream.read_to_end(&mut bytes) {
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
                ) => {}
            Err(error) => panic!("request did not finish within timeout: {error}"),
        }
        if bytes.is_empty() {
            return None;
        }
        let split = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("HTTP response header terminator");
        let headers = String::from_utf8(bytes[..split].to_vec()).unwrap();
        let status = headers
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        Some(Response {
            status,
            headers,
            body: bytes[split + 4..].to_vec(),
        })
    }

    fn changed_status(&self, previous: &[u8]) -> Response {
        let until = Instant::now() + Duration::from_secs(6);
        loop {
            let response = self.request("GET", "/__ubi/status");
            assert_eq!(response.status, 200);
            if response.body != previous {
                return response;
            }
            assert!(
                Instant::now() < until,
                "watcher did not publish change within six seconds"
            );
            thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Response {
    status: u16,
    headers: String,
    body: Vec<u8>,
}

impl Response {
    fn header(&self, name: &str) -> &str {
        self.headers
            .lines()
            .find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case(name).then(|| value.trim())
            })
            .unwrap_or_else(|| panic!("missing {name}: {}", self.headers))
    }
    fn text(&self) -> &str {
        std::str::from_utf8(&self.body).unwrap()
    }
}

fn bounded_command(root: &Path, arguments: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ubi"))
        .current_dir(root)
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let until = Instant::now() + Duration::from_secs(8);
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= until {
            let _ = child.kill();
            let _ = child.wait();
            panic!("command failed to exit within eight seconds: {arguments:?}")
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn dev_starts_on_loopback_without_node_and_serves_assets_and_current_modules() {
    let project = Project::new();
    project.write("web/site.css", "body { color: green; }");
    project.write("web/site.js", "export const message = 'ready';");
    project.write(
        "web/icon.svg",
        "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
    );
    let server = Server::start(&project, &[], true);
    for (path, mime, content) in [
        ("/", "text/html", "Dev fixture"),
        ("/site.css", "text/css", "color: green"),
        ("/site.js", "javascript", "message"),
        ("/icon.svg", "image/svg+xml", "<svg"),
        ("/__ubi/modules/src/main.mjs", "javascript", "lib.mjs"),
        ("/__ubi/modules/src/lib.mjs", "javascript", "41"),
    ] {
        let get = server.request("GET", path);
        assert_eq!(get.status, 200, "{path}");
        assert!(
            get.header("Content-Type").contains(mime),
            "{path}: {}",
            get.headers
        );
        assert!(
            get.header("Cache-Control").contains("no-"),
            "{path}: {}",
            get.headers
        );
        assert!(get.text().contains(content), "{path}: {}", get.text());
        let head = server.request("HEAD", path);
        assert_eq!(head.status, 200);
        assert!(head.body.is_empty());
        assert_eq!(head.header("Content-Type"), get.header("Content-Type"));
        assert_eq!(head.header("Content-Length"), get.header("Content-Length"));
    }
    let index = server.request("GET", "/");
    let polls_status = index.text().contains("/__ubi/status")
        || index.text().split("src=").skip(1).any(|attribute| {
            let attribute = attribute.trim_start();
            let source = match attribute.as_bytes().first() {
                Some(b'\"' | b'\'') => attribute[1..]
                    .split(attribute.as_bytes()[0] as char)
                    .next()
                    .unwrap(),
                _ => attribute.split([' ', '>']).next().unwrap(),
            };
            if !source.starts_with('/') || source.starts_with("//") {
                return false;
            }
            let script = server.request("GET", source);
            script.status == 200 && script.text().contains("/__ubi/status")
        });
    assert!(
        polls_status,
        "index must load developer script that polls status"
    );
    let status = server.request("GET", "/__ubi/status");
    assert_eq!(status.status, 200);
    assert!(status.header("Content-Type").contains("application/json"));
    assert!(status.text().trim().starts_with('{'));
}

#[test]
fn dev_watches_source_and_web_asset_edits_creations_and_deletions() {
    let project = Project::new();
    let server = Server::start(&project, &["src/main.ubi"], false);
    let mut status = server.request("GET", "/__ubi/status");
    project.write("src/lib.ubi", "export fn number() -> int { 12345 }");
    status = server.changed_status(&status.body);
    assert!(server
        .request("GET", "/__ubi/modules/src/lib.mjs")
        .text()
        .contains("12345"));
    project.write("new.ubi", "fn private() -> int { 1 }");
    status = server.changed_status(&status.body);
    fs::remove_file(project.0.join("new.ubi")).unwrap();
    status = server.changed_status(&status.body);
    project.write("web/new.css", "body { color: blue; }");
    status = server.changed_status(&status.body);
    assert_eq!(server.request("GET", "/new.css").status, 200);
    project.write("web/new.css", "body { color: purple; }");
    status = server.changed_status(&status.body);
    assert!(server.request("GET", "/new.css").text().contains("purple"));
    fs::remove_file(project.0.join("web/new.css")).unwrap();
    server.changed_status(&status.body);
    assert_eq!(server.request("GET", "/new.css").status, 404);
}

#[test]
fn dev_failed_rebuild_blocks_modules_and_recovers_without_stopping_server() {
    let project = Project::new();
    let mut server = Server::start(&project, &[], false);
    let initial = server.request("GET", "/__ubi/status");
    project.write("src/lib.ubi", "export fn number() -> int { missing }");
    let failed = server.changed_status(&initial.body);
    assert!(failed.text().contains("UBI0010"), "{}", failed.text());
    assert_eq!(
        server.request("GET", "/__ubi/modules/src/main.mjs").status,
        503
    );
    assert_eq!(
        server.request("HEAD", "/__ubi/modules/src/lib.mjs").status,
        503
    );
    assert_eq!(server.request("GET", "/").status, 200);
    assert!(server.child.try_wait().unwrap().is_none());
    project.write("src/lib.ubi", "export fn number() -> int { 54321 }");
    let recovered = server.changed_status(&failed.body);
    assert!(!recovered.text().contains("UBI0010"));
    let module = server.request("GET", "/__ubi/modules/src/lib.mjs");
    assert_eq!(module.status, 200);
    assert!(module.text().contains("54321"));
}

#[test]
fn dev_refuses_private_paths_traversal_nonloopback_hosts_and_unsupported_methods() {
    let project = Project::new();
    project.write("web/.env", "PRIVATE_SENTINEL");
    project.write("web/.hidden/file.txt", "PRIVATE_SENTINEL");
    project.write("web/leak.ubi", "PRIVATE_SENTINEL");
    project.write("web/ubi.json", "PRIVATE_SENTINEL");
    project.write(".ubi-build/extra.mjs", "PRIVATE_SENTINEL");
    fs::write(
        project.0.join("web/large.bin"),
        vec![b'x'; 8 * 1024 * 1024 + 1],
    )
    .unwrap();
    let server = Server::start(&project, &[], false);
    for path in [
        "/../ubi.json",
        "/%2e%2e/ubi.json",
        "/%2E%2E%2Fubi.json",
        "/..%5cubi.json",
        "/%2e%2e%5csrc%5cmain.ubi",
        "/.env",
        "/.hidden/file.txt",
        "/%2ehidden/file.txt",
        "/src/main.ubi",
        "/leak.ubi",
        "/ubi.json",
        "/.ubi-build/extra.mjs",
        "/__ubi/modules/../extra.mjs",
        "/__ubi/modules/%2e%2e/extra.mjs",
        "/__ubi/modules/extra.mjs",
        "/__ubi/modules/src/main.ubi",
    ] {
        let response = server.request("GET", path);
        assert!(
            (400..500).contains(&response.status),
            "{path}: {}",
            response.status
        );
        assert!(!response.text().contains("PRIVATE_SENTINEL"));
    }
    for host in [
        "evil.example",
        "192.0.2.1",
        "127.0.0.1.evil.example",
        "localhost.evil.example",
    ] {
        let response = server
            .raw(&format!(
                "GET / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
            ))
            .unwrap();
        assert!(
            (400..500).contains(&response.status),
            "accepted Host: {host}"
        );
    }
    for method in ["POST", "PUT", "DELETE", "CONNECT"] {
        assert!((400..500).contains(&server.request(method, "/").status));
    }
    let oversized = format!(
        "GET / HTTP/1.1\r\nHost: {}\r\nX-Large: {}\r\n\r\n",
        server.address,
        "x".repeat(128 * 1024)
    );
    if let Some(response) = server.raw(&oversized) {
        assert!((400..500).contains(&response.status));
    }
    let started = Instant::now();
    let incomplete = format!("GET / HTTP/1.1\r\nHost: {}\r\n", server.address);
    if let Some(response) = server.raw(&incomplete) {
        assert!((400..500).contains(&response.status));
    }
    assert!(
        started.elapsed() < Duration::from_millis(1500),
        "incomplete header exceeded bounded read budget with scheduling allowance"
    );
    let large = server.request("GET", "/large.bin");
    assert!(
        large.status >= 400,
        "served public asset beyond 8 MiB limit"
    );
    assert!(large.body.len() < 8 * 1024 * 1024);
    assert_eq!(server.request("GET", "/").status, 200);
}

#[test]
fn dev_does_not_serve_assets_symlinked_outside_web_root() {
    let project = Project::new();
    let outside = Project::new();
    outside.write("secret.txt", "PRIVATE_SENTINEL");
    project.write("private.txt", "PRIVATE_SENTINEL");
    let link = project.0.join("web/outside.txt");
    #[cfg(windows)]
    let result = std::os::windows::fs::symlink_file(outside.0.join("secret.txt"), &link);
    #[cfg(unix)]
    let result = std::os::unix::fs::symlink(outside.0.join("secret.txt"), &link);
    if let Err(error) = result {
        eprintln!("SKIP dev symlink containment: {error}");
        return;
    }
    let inner_link = project.0.join("web/inside-project.txt");
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(project.0.join("private.txt"), &inner_link).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(project.0.join("private.txt"), &inner_link).unwrap();
    let server = Server::start(&project, &[], false);
    let response = server.request("GET", "/outside.txt");
    assert!((400..500).contains(&response.status));
    assert!(!response.text().contains("PRIVATE_SENTINEL"));
    let response = server.request("GET", "/inside-project.txt");
    assert!((400..500).contains(&response.status));
    assert!(!response.text().contains("PRIVATE_SENTINEL"));
}

#[test]
fn dev_setup_and_option_failures_are_bounded_and_distinguish_compile_errors() {
    let project = Project::new();
    for options in [
        vec!["--target", "cli"],
        vec!["--port", "65536"],
        vec!["--port", "-1"],
        vec!["--port", "bad"],
        vec!["--port"],
        vec!["--json"],
        vec!["--out-dir", "out"],
        vec!["--function", "main"],
        vec!["--args", "[]"],
    ] {
        let mut arguments = vec!["dev"];
        arguments.extend(options);
        let output = bounded_command(&project.0, &arguments);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stderr.is_empty());
    }
    fs::remove_file(project.0.join("web/index.html")).unwrap();
    assert_eq!(
        bounded_command(&project.0, &["dev", "--port", "0"])
            .status
            .code(),
        Some(2)
    );
    project.write("web/index.html", "<html>ready</html>");
    project.write(
        "ubi.json",
        r#"{"schemaVersion":1,"name":"dev test","entry":"src/main.ubi","targets":["cli"]}"#,
    );
    assert_eq!(
        bounded_command(&project.0, &["dev", "--port", "0"])
            .status
            .code(),
        Some(2)
    );
    project.write("src/main.ubi", "export fn main() -> int { missing }");
    let failed = bounded_command(&project.0, &["dev", "src/main.ubi", "--port", "0"]);
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stderr).contains("UBI0010"));
    assert!(!String::from_utf8_lossy(&failed.stdout).contains("http://"));
}
