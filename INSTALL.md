# Installation Guide

## From a release

Download from the [Releases page](https://github.com/AberrantWolf/print-junk/releases). The GUI and CLI are separate downloads.

### GUI (Print Junk)

| Platform | Download | Install |
|----------|----------|---------|
| macOS (Apple Silicon) | `Print-Junk-vX.Y.Z-macos-arm64.zip` | Unzip, drag **Print Junk.app** to Applications |
| macOS (Intel) | `Print-Junk-vX.Y.Z-macos-x64.zip` | Unzip, drag **Print Junk.app** to Applications |
| Linux (x64) | `Print-Junk-vX.Y.Z-linux-x64.AppImage` | `chmod +x` the file, then double-click or run it |
| Windows (x64) | `Print-Junk-vX.Y.Z-windows-x64.zip` | Unzip, run `print-junk-gui.exe` |

### CLI (pdft)

| Platform | Download |
|----------|----------|
| macOS (Apple Silicon) | `pdft-vX.Y.Z-macos-arm64.tar.gz` |
| macOS (Intel) | `pdft-vX.Y.Z-macos-x64.tar.gz` |
| Linux (x64) | `pdft-vX.Y.Z-linux-x64.tar.gz` |
| Windows (x64) | `pdft-vX.Y.Z-windows-x64.zip` |

Extract and place `pdft` somewhere on your PATH.

### Platform notes

#### macOS Gatekeeper

Since the app isn't signed yet, macOS may block it. To allow it:

- Right-click the app and select "Open", or
- Run `xattr -cr "Print Junk.app"` in Terminal

#### Linux dependencies

The AppImage bundles its dependencies, but if you run the binary directly you may need:

```bash
# Debian/Ubuntu
sudo apt install libxcb-render0 libxcb-shape0 libxcb-xfixes0 libxkbcommon0 libgl1 libgtk-3-0

# Fedora
sudo dnf install libxcb libxkbcommon mesa-libGL gtk3

# Arch
sudo pacman -S libxcb libxkbcommon mesa gtk3
```

#### Windows

You may need the [Visual C++ Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist) if it isn't already installed.

## From source

### Supported platforms

| Platform | Architectures |
|----------|---------------|
| macOS | Intel (x64), Apple Silicon (arm64) |
| Linux | x64, arm64 |
| Windows | x64, x86, arm64 |

### Building

```bash
cargo build --release
```

PDF rendering is pure Rust (the shared `junk-libs-platen` crate) — no binary
downloads, no bundled libraries, no manual setup.

To build without the PDF viewer:

```bash
cargo build --release --no-default-features
```

### Linux build dependencies

In addition to the runtime libraries above, you need development headers:

```bash
# Debian/Ubuntu
sudo apt install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxkbcommon-dev libgl1-mesa-dev libgtk-3-dev libatk1.0-dev
```

## Troubleshooting

### Getting help

Open an issue with your OS, architecture (`uname -m`), and the full error message.
