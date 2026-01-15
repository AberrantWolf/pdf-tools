# Project Context for Claude

## Architecture

- **CLI app** (`pdf-tools-cli`) and **GUI app** (`pdf-tools-gui`) use shared library crates for core logic
- **Async architecture** with channels for responsive UIs
- All work must be async with commands/updates via communication channels
- **Cross-platform**: desktop (macOS/Linux/Windows) + web (WASM)
  - Web: GUI only
  - Desktop: both CLI and GUI
- All functionality must work on all platforms

## Purpose

Collection of PDF processing and generation tools - features wanted but not found in usable FOSS form.

## Crate Structure

Located in `crates/`:

- `pdf-tools-cli` - CLI interface (binary: `pdft`) - working
- `pdf-tools-gui` - Desktop GUI (egui/eframe) + WASM web support
- `pdf-async-runtime` - Async runtime/communication layer (`PdfCommand`/`PdfUpdate` channels)
- `pdf-impose` - PDF imposition library with signature binding, perfect binding, printer's marks
- `pdf-flashcards` - PDF flashcard generation from CSV (working)
- Additional crates as needed for shared functionality

## Features

### Flashcard Generation (Working)

- Generate printable flashcard PDFs from CSV files
- Configurable paper types (Letter, Legal, A4, A5, Custom)
- Customizable card dimensions, rows, columns
- Multiple measurement systems (inches, millimeters, points)

### PDF Imposition (Working)

- Binding types: signature, perfect binding, side stitch, spiral, case binding
- Page arrangements: folio (4pgs), quarto (8pgs), octavo (16pgs), custom
- Output formats: double-sided, two-sided, single-sided sequence
- Scaling modes: fit, fill, none, stretch
- Printer's marks: fold lines, cut lines, crop marks, registration marks, sewing marks, spine marks
- Sheet and leaf margins configuration
- Flyleaves (front/back blank pages)
- Configuration loading/saving (JSON)
- Statistics calculation (source pages, output sheets, signatures, blank pages added)
- CLI: `pdft impose` with full options support
- GUI: fully built with live preview

### PDF Viewer (Desktop Only)

- Basic page navigation using PDFium library
- LRU page cache (50 pages) for fast navigation
- Prefetching of adjacent pages for smoother navigation
- Command deduplication to avoid redundant renders
- Optional feature (`pdf-viewer`) - can build without it
- PDFium auto-downloaded and vendored to `vendor/pdfium/`
- TODOs: zoom controls, jump-to-page input, thumbnail sidebar

## CLI Usage

```bash
# Flashcards
pdft flashcards -i cards.csv -o output.pdf --rows 2 --columns 3

# Imposition
pdft impose -i input.pdf -o output.pdf \
  --binding signature \
  --arrangement folio \
  --paper letter \
  --orientation landscape \
  --format double-sided \
  --scaling fit \
  --fold-lines \
  --crop-marks \
  --stats-only  # Preview stats without generating
```

## Performance Optimizations

- **Source document caching**: Impose preview doesn't reload PDFs on every option change
- **Page prefetching**: Viewer preloads adjacent pages (N-1, N+1, N+2) after rendering current page
- **Command deduplication**: Both preview generation and page render commands are deduplicated - rapid changes only process the latest request
- **LRU cache**: 50 rendered pages cached to avoid re-rendering

## Development Guidelines

1. **Be concise, correct, direct** - no politeness/pandering
2. **Don't repeat yourself**
3. **Modular design** - with any major change, consider:
   - Should source files be split?
   - Should a new crate be created for reusability?
4. **Shared logic goes in library crates** - not in CLI/GUI apps
5. **Responsive UIs** - use async + channels, never block
6. **Never use dead/unmaintained crates** - verify crates are actively maintained
7. **Always favor the latest version of a crate**
