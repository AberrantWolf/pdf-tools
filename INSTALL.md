# Installation Guide

## Prerequisites

The PDF viewer functionality requires **PDFium**, which is automatically downloaded during build.

## Building the Project

PDFium is automatically downloaded and installed to `vendor/pdfium/` during the build process:

```bash
# Build all binaries (PDFium downloaded automatically)
cargo build --release

# Run the GUI
cargo run --release --bin pdf-tools-gui

# Run the CLI
cargo run --release --bin pdf-tools-cli -- --help
```

The build script will:

1. Detect your platform and architecture
2. Download the appropriate PDFium binaries from [pdfium-binaries](https://github.com/bblanchon/pdfium-binaries)
3. Extract them to `vendor/pdfium/` in the repository
4. Configure linking automatically

## Supported Platforms

- **macOS**: Intel (x64) and Apple Silicon (arm64)
- **Linux**: x64 and arm64
- **Windows**: x86, x64, and arm64

## Running the Built Application

### macOS

```bash
# Set library path to use the vendored PDFium
export DYLD_LIBRARY_PATH="$(pwd)/vendor/pdfium/lib:$DYLD_LIBRARY_PATH"
./target/release/pdf-tools-gui
```

### Linux

```bash
# Set library path to use the vendored PDFium
export LD_LIBRARY_PATH="$(pwd)/vendor/pdfium/lib:$LD_LIBRARY_PATH"
./target/release/pdf-tools-gui
```

### Windows

```cmd
REM Ensure the vendor\pdfium\lib directory is accessible
set PATH=%CD%\vendor\pdfium\lib;%PATH%
target\release\pdf-tools-gui.exe
```

## Alternative: Building Without PDF Viewer

If you encounter issues or don't need the PDF viewer:

```bash
# Build GUI without PDF viewer feature
cargo build --release --bin pdf-tools-gui --no-default-features

# CLI and other features still work
cargo build --release --bin pdf-tools-cli
```

The flashcards and impose features will still work without the PDF viewer.

## Distribution

### For End Users (Binary Distribution)

When distributing the compiled application, include the PDFium library:

**macOS:**

```bash
mkdir -p dist/pdf-tools.app/Contents/MacOS
mkdir -p dist/pdf-tools.app/Contents/Frameworks

cp target/release/pdf-tools-gui dist/pdf-tools.app/Contents/MacOS/
cp vendor/pdfium/lib/libpdfium.dylib dist/pdf-tools.app/Contents/Frameworks/

# Update library path in binary
install_name_tool -change \
  libpdfium.dylib \
  @executable_path/../Frameworks/libpdfium.dylib \
  dist/pdf-tools.app/Contents/MacOS/pdf-tools-gui
```

**Linux:**

```bash
mkdir -p dist/lib
cp target/release/pdf-tools-gui dist/
cp vendor/pdfium/lib/libpdfium.so dist/lib/

# Create launcher script
cat > dist/pdf-tools.sh << 'EOF'
#!/bin/bash
DIR="$(dirname "$(readlink -f "$0")")"
export LD_LIBRARY_PATH="$DIR/lib:$LD_LIBRARY_PATH"
exec "$DIR/pdf-tools-gui" "$@"
EOF
chmod +x dist/pdf-tools.sh
```

**Windows:**

```cmd
mkdir dist
copy target\release\pdf-tools-gui.exe dist\
copy vendor\pdfium\lib\pdfium.dll dist\
```

## Troubleshooting

### Build Fails to Download PDFium

If the download fails:

1. Check your internet connection
2. Try building again (the script will retry)
3. Manually download from [pdfium-binaries releases](https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium%2F7350)
4. Extract to `vendor/pdfium/` in the repository root

### Runtime "Failed to Load PDFium" Error

Ensure the library path is set correctly:

**macOS:**

```bash
export DYLD_LIBRARY_PATH="$(pwd)/vendor/pdfium/lib:$DYLD_LIBRARY_PATH"
```

**Linux:**

```bash
export LD_LIBRARY_PATH="$(pwd)/vendor/pdfium/lib:$LD_LIBRARY_PATH"
```

### Clean Build

To force re-download of PDFium:

```bash
rm -rf vendor/pdfium
cargo clean -p pdf-tools-gui
cargo build --release
```

## Getting Help

If you encounter issues:

1. Check the [pdfium-binaries releases](https://github.com/bblanchon/pdfium-binaries/releases) for your platform
2. Verify your architecture: `uname -m` (macOS/Linux) or System Properties (Windows)
3. Open an issue with:
   - Your OS and architecture
   - Complete error message
   - Output of `cargo build -vv`
