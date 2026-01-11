# PDF Tools

Collection of PDF processing and generation tools built with Rust.

## Features

- **PDF Viewer** - Basic PDF viewing with page navigation
- **Flashcard Generator** - Create PDF flashcards from CSV
- **PDF Imposition** - 2-up, 4-up, booklet layouts (in progress)

## Quick Start

```bash
# Build (PDFium downloaded automatically during build)
cargo build --release

# Run GUI
cargo run --release --bin pdf-tools-gui

# Run CLI
cargo run --release --bin pdf-tools-cli -- --help
```

PDFium is automatically downloaded and installed to `vendor/pdfium/` during the build process. See **[INSTALL.md](INSTALL.md)** for platform-specific runtime instructions and troubleshooting.

## Documentation

- **[INSTALL.md](INSTALL.md)** - Installation instructions, troubleshooting, and distribution
- **[CLAUDE.md](CLAUDE.md)** - Project architecture and development guidelines

## Architecture

- **CLI** (`pdf-tools-cli`) - Command-line interface
- **GUI** (`pdf-tools-gui`) - Desktop app built with egui
- **Libraries** - Shared functionality in separate crates
  - `pdf-flashcards` - Flashcard generation
  - `pdf-impose` - PDF imposition
  - `pdf-async-runtime` - Async command/update infrastructure

## Building Without PDF Viewer

If you encounter issues with PDFium:

```bash
cargo build --release --no-default-features
```

The flashcards and impose features will still work.

## License

MIT
