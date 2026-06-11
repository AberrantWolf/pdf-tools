# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Purpose

Collection of PDF processing and generation tools — features wanted but not found in usable FOSS form. Targets real-world print production and bookbinding workflows.

## Commands

```bash
# Build everything (release; PDF rendering is pure Rust via junk-libs-platen — no downloads)
cargo build --release

# Build without the PDF viewer
cargo build --no-default-features

# Run the apps
cargo run --release --bin print-junk-gui
cargo run --release --bin pdft -- --help

# Tests
cargo test                              # whole workspace
cargo test -p pdf-impose                # one crate
cargo test -p pdf-impose --test impose_tests   # one integration test file
cargo test -p pdf-impose spread         # tests matching a substring

# Lint — workspace lint config is strict; correctness/suspicious are deny-level
cargo clippy --all-targets

# Imposition example binaries (visual/format sanity checks)
cargo run -p pdf-impose --example test_all_formats

# Typesetting sample (writes a typeset PDF for visual checks)
cargo run -p pdf-typeset --example sample -- /tmp/sample.pdf
```

Note: `platen_render_test` (in `print-junk-gui`) requires the `pdf-viewer` feature; it is skipped under `--no-default-features`.

## Architecture

Cargo workspace. Two binaries share logic through library crates — **no PDF logic lives in the apps**; it goes in the libraries so both CLI and GUI (and WASM) reuse it.

```
crates/
  print-junk-cli      CLI (binary: pdft) — clap-based, calls libraries directly
  print-junk-gui      Desktop GUI (egui/eframe) + WASM web build
  pdf-async-runtime   PdfCommand / PdfUpdate channel message types (the contract between UI and worker)
  pdf-impose          Imposition library (the bulk of domain logic)
  pdf-flashcards      Flashcard PDF generation from CSV
  pdf-typeset         Text (Plaintext/Markdown/HTML) → typeset PDF via Typst (desktop-only)
  pdf-units           Shared paper sizes, orientation, margins, mm/pt conversions (I/O-free)
```

### GUI async model (important)

The GUI never blocks on PDF work. It follows a command/update channel pattern:

- The UI thread sends `PdfCommand` variants over an mpsc channel.
- `worker.rs` runs an async `worker_task` that pattern-matches each command and dispatches to `handlers/{flashcards,impose,viewer}.rs`.
- Handlers send `PdfUpdate` variants back (progress, results, errors), which the UI drains each frame.
- The command/update enums live in `pdf-async-runtime` so both sides depend on one shared contract.
- On desktop the worker runs on a tokio multi-thread runtime; on WASM it uses `wasm-bindgen-futures` (tokio `sync` only). Keep handler logic runtime-agnostic.

GUI source split: `app.rs` (eframe app state/update loop) → `views/` (egui rendering) and `handlers/` (worker-side command processing). The `viewer.rs`/`ViewerState` is gated behind the `pdf-viewer` feature. The Typesetting mode and project save/restore (`project.rs`) are desktop-only, gated with `cfg(not(target_arch = "wasm32"))` (Typst doesn't build for WASM); on the web build the Typesetting tab shows an "unavailable" message.

Project save/restore: `app.rs` auto-persists every mode's settings to eframe storage (restored on launch) and supports explicit `.pjproj` files via the `☰` menu. Only settings and file *paths* are persisted — never file contents — so loaded files are re-read from their paths on restore.

### pdf-impose structure

Domain logic is layered: `layout/` computes page placement (arrangement, page order, creep, slots, spreads) independent of PDF I/O; `impose/` performs the actual PDF assembly (signature/simple/cascade strategies, page sources, sheet building); `marks.rs` draws printer's marks; `stats.rs` computes statistics; `preview.rs` renders previews; `options.rs`/`types.rs` define the public config surface. Layout is unit-tested in `src/layout/tests/`; cross-cutting behavior is tested in `tests/`.

### Platform matrix

- Desktop (macOS/Linux/Windows): both CLI and GUI; GUI includes the platen-backed (hayro) viewer and the Typst-backed typesetting mode.
- Web (WASM): GUI only, no viewer and no typesetting.
- Desktop-only capabilities: the PDF viewer (`pdf-viewer` feature) and the Typesetting mode + project save/restore (`cfg(not(target_arch = "wasm32"))`, since Typst is a large native dependency). All other functionality must work on every platform.

## Terminology (print/bookbinding domain)

- **Sheet**: a physical sheet of paper. **Printer Page**: one side of a sheet as printed.
- **Page**: one side of printed content. **Book Page**: one side of a leaf — multiple book pages fit on one printer page in signature binding.
- **Leaf**: front and back of a single piece of paper. **Recto**: front of a leaf. **Verso**: back of a leaf.
- **Signature**: a collated set of folded pages. **Book**: signatures bound together.
- **Spine**: the bound back of a book; also the fold line of a signature. **Spread**: verso + recto aligned with the spine in the middle.

## Development guidelines

1. Be concise, correct, direct.
2. Shared logic goes in library crates, never in the CLI/GUI apps.
3. Responsive UIs only — async + channels, never block the UI thread.
4. Modular design: with any major change, ask whether a file should be split or logic extracted into a (reusable) crate.
5. **There is no stable API.** Always favor better design over preserving existing signatures. When a new system replaces an old one, delete the old one — don't leave both.
6. Never use dead/unmaintained crates; prefer the latest version.
7. **Never silence clippy with `#[allow(...)]`** — fix the underlying code. Workspace-level allows in root `Cargo.toml` are a temporary debt to be eliminated (see the TODO there); do not add new ones.

## Planning requirements

Every implementation plan must address three sections:

1. **DRY** — identify duplication to reduce and existing abstractions to reuse; consolidate patterns already present elsewhere.
2. **Best practices** — improve adherence to codebase conventions and Rust idioms; flag and propose fixes for anti-patterns the change touches.
3. **Usefulness for printers and bookbinders** — evaluate from the perspective of real print production; ensure terminology, defaults, and behavior match professional workflows.

## Notes

- PDF rendering is provided by the shared `junk-libs-platen` crate (git dep, gated behind the `pdf-viewer` feature) — a pure-Rust engine (hayro), so there is nothing to download, bundle, or bind at runtime, and pages can render in parallel.
- Releases are tag-driven: bump version in root `Cargo.toml` `[workspace.package]`, then push a `vX.Y.Z` tag — the GitHub release workflow builds all platforms. See `INSTALL.md` for runtime/build dependencies.
- `Imposition Details.md` contains in-depth reference on imposition math and arrangements.
