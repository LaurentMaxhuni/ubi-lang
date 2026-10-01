# Animated Ubi landing page

Run from the repository root:

```powershell
cargo run -- dev --root examples/landing-page --port 3017
```

Open http://127.0.0.1:3017/. No frontend dependencies or external assets required.

The HTML/CSS/JavaScript host provides the interface and animations. Compiled
`src/main.ubi` functions control the live orbit's radius, duration, and label,
plus the compilation target descriptions. The source preview is an excerpt.
The energy input is clamped to 1–100 inside Ubi, including calls from other hosts.

Includes responsive layouts, keyboard controls, reduced-motion support, a motion
pause button, clipboard installation command, and expandable getting-started
instructions. Desktop/mobile targets describe future packaging, not current
native UI support.

Check and compile independently:

```powershell
cargo run -- check --root examples/landing-page
cargo run -- build --root examples/landing-page
```
