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
- `pdf-tools-cli` - CLI interface
- `pdf-tools-gui` - GUI interface (egui/eframe)
- `pdf-impose` - PDF imposition library (shared)
- `pdf-flashcards` - PDF flashcard generation library (shared)
- Additional crates as needed for shared functionality

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
