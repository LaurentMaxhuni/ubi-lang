# Linux verification, 2026-10-09

Existing Ubuntu 24.04.4 WSL2 x86_64 and WSLg ran the CLI and desktop application.
These checks verify this host; hosted CI, macOS and iOS remain unexecuted.

## Isolated prerequisites

Verification tools and outputs live under ignored `.ubi-build/linux-verification`.
Node 22.22.1 and 24.21.0 were downloaded from the official Node distribution and checked against
its SHA-256 manifest. Rustup installed Rust/Cargo1.99.0 with project-local
`CARGO_HOME`/`RUSTUP_HOME` and no shell-profile edits. Ubuntu package downloads
were extracted locally: GCC13, its libraries, NSS/NSPR and ALSA. Existing system
GTK/GBM and WSLg supplied display/runtime prerequisites. No system package
installation, global PATH change, sandbox disabling or security-policy change
was needed.

`CARGO_TARGET_DIR` and `TMPDIR` also pointed into this verification directory.
The local compiler wrapper selects the extracted GCC13 and its support files;
the local runtime library directory is supplied through `LD_LIBRARY_PATH`.
On a normal Linux build host with Rust, Node and Electron prerequisites already
installed, the repository commands below need no wrapper.

## Executed evidence

| Check | Result |
| --- | --- |
| Node CLI | Created, completed and listed a task from a task-owned durable store |
| Compiler/conformance | Independent `cargo test --all-targets --locked -- --nocapture`: 178 passed with Node 22; a second full run with Node 24.21.0 also passes all 178 |
| Symlink prerequisites | Independent file and directory symlink creation/read passed on the actual temporary filesystem |
| Symlink containment | CLI imports/config/entry/output, dev assets and package assets checked without skipped fixtures |
| Formatting/Clippy | `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` pass on Rust1.99 |
| Desktop source | Locked npm install and `npm run smoke` exit0, `UBI_DESKTOP_SMOKE_OK` |
| Desktop package | Electron44.7.0 / Packager20.3.0 create `out/UbiTasks-linux-x64/UbiTasks` |
| Actual executable | `node scripts/verify-desktop.mjs <output>` exit0, `UBI_DESKTOP_SMOKE_OK`; persistence and renderer isolation pass |

Independent verification found an intermittent existing dev-server failure:
Linux resets a socket closed with unread oversized input, sometimes after only
part of a fragmented rejection header was delivered. The server now refuses
headers over8KiB without writing a response. Existing memory/time limits remain;
the independent regression permits silent close. After the fix,100 oversized
requests produced zero response bytes, and the dev and full suites passed.

Evidence logs:

- `.ubi-build/linux-verification/independent-tests.log`
- `.ubi-build/linux-verification/node24-tests.log`
- `.ubi-build/linux-verification/clippy-verified.log`
- `.ubi-build/linux-verification/desktop-smoke.log`
- `.ubi-build/linux-verification/desktop-package.log`
- `.ubi-build/linux-verification/desktop-packaged-smoke.log`

## Reproduction on a prepared Linux host

```sh
cargo test --all-targets --locked
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo run --locked -- package --root examples/v1 --target cli
verification_store="$(mktemp -d)/tasks.json"
node examples/v1/.ubi-dist/cli/run.mjs --store "$verification_store" add VerifyLinux
node examples/v1/.ubi-dist/cli/run.mjs --store "$verification_store" complete 1
cargo run --locked -- package --root examples/v1 --target desktop
cd examples/v1/.ubi-dist/desktop
npm ci
npm run smoke
npm run package
cd ../../../..
node scripts/verify-desktop.mjs
```

Use a fresh task-owned store for each run. Headless CI uses `xvfb-run -a` around
the desktop launch commands. Standard Ubuntu24 desktop installations may require
an app-specific Chromium sandbox prerequisite under AppArmor; WSLg success does
not validate that configuration. CI temporarily uses Ubuntu22.04 with sandbox
enabled; migrate before that runner retires on2027-04-17.
