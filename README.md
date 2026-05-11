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
- On first launch, start at one-quarter of the current screen area, then remember user resizing.
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

For a portable release package on Windows, use:

```powershell
.\Build-Windows.ps1
```

That creates a ZIP under:

```text
dist/
```

For local testing:

```powershell
.\target\release\markrush.exe
```

Notes for agents:

- Prefer `cargo check` for quick validation and `cargo build --release` before handing off a Windows portable build.
- Prefer `.\Build-Windows.ps1` when preparing a Windows package, because it forces a fresh date/hash build stamp and writes a SHA256 file.
- The Windows release build uses `build.rs` to embed the app icon and generated version metadata into the `.exe`.
- The Windows release binary is configured as a GUI app, so double-clicking `target/release/markrush.exe` should not open a blank console.
- If resource embedding fails on Windows, install the Visual Studio Build Tools or Windows SDK so Cargo can find the Windows resource compiler.
- Do not treat `reference/wpf` as active source. It is kept only to compare behavior with the original Windows version.

## Build Version

Release builds are stamped at compile time with a date plus git hash, similar to Venice Media Local. The status bar shows a friendly label such as:

```text
ver 2026-05-11 - gabcdef12
```

The full generated build version is shaped like:

```text
2026.5.11071530+gabcdef12
```

Meaning:

- `2026` = UTC build year
- `5` = UTC build month
- `11071530` = UTC day + time, here day 11 at 07:15:30
- `gabcdef12` = git commit hash
- `.dirty` is appended when the build was made with uncommitted tracked changes

For release binaries, commit first, then run `cargo build --release` so the executable contains the release commit hash rather than the previous commit or a dirty marker.

## Port Status

See [docs/parity-checklist.md](docs/parity-checklist.md).

## License

MIT

Bundled fonts in `assets/fonts` are distributed under the SIL Open Font License 1.1, with license text included next to each font file.
