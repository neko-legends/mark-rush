# MarkRush

MarkRush is a fast local Markdown viewer and editor. The goal is fast file loading and smooth scrolling for Markdown files with thousands of lines, especially in large local folders where slower editors can lag.

The active implementation is a cross-platform Rust desktop app built with egui/eframe. The original working Windows WPF implementation is preserved in `reference/wpf` only as a behavior reference.

## Goals

- Open local Markdown folders quickly.
- Read and edit files with thousands of lines without heavy editor lag.
- Follow local Markdown links and heading anchors.
- Keep the UI compact, readable, and themeable.
- Run on Windows, macOS, and Linux from one Rust codebase.

## Current Features

- Open a folder with a native folder picker.
- Browse Markdown files from a cached, lazily expanded folder tree.
- Open a folder or file from command-line arguments.
- Reopen the last folder/file on startup.
- Render large Markdown files as virtualized rows for responsive scrolling.
- Switch between read mode and raw Markdown edit mode.
- Save with the Save button or `Ctrl+S`.
- Prompt before discarding unsaved changes.
- Search with `Ctrl+F`, `Enter`, `Shift+Enter`, `F3`, and `Shift+F3`.
- Follow local file links, local folder links, heading anchors, and external URLs.
- Adjust rendered/editing font family and document font size from the top toolbar.
- Persist theme, wrap mode, document font, window size, and rendered scroll position.
- Includes the Venice-style theme set plus High Contrast.
- Bundles redistributable document fonts: Noto Sans, Noto Serif, Lora, Poppins, and Roboto Mono.
- Writes crash logs to the platform app config directory.

## Build For Agents

Work from the repository root:

```powershell
cd D:\forPublic\MarkRush
```

Rust is required. If `cargo` is on `PATH`, use:

```powershell
cargo fmt
cargo check
cargo build --release
```

On this Windows machine, Cargo may need to be called by full path:

```powershell
$Cargo = "$env:USERPROFILE\.cargo\bin\cargo.exe"
& $Cargo fmt
& $Cargo check
& $Cargo build --release
```

The portable Windows executable is produced at:

```text
target/release/markrush.exe
```

For local testing:

```powershell
.\target\release\markrush.exe
```

Notes for agents:

- Prefer `cargo check` for quick validation and `cargo build --release` before handing off a Windows portable build.
- The Windows release build uses `build.rs` and `assets/windows/MarkRush.rc` to embed the app icon into the `.exe`.
- The Windows release binary is configured as a GUI app, so double-clicking `target/release/markrush.exe` should not open a blank console.
- If resource embedding fails on Windows, install the Visual Studio Build Tools or Windows SDK so Cargo can find the Windows resource compiler.
- Do not treat `reference/wpf` as active source. It is kept only to compare behavior with the original Windows version.

## Port Status

See [docs/parity-checklist.md](docs/parity-checklist.md).

## License

MIT

Bundled fonts in `assets/fonts` are distributed under the SIL Open Font License 1.1, with license text included next to each font file.
