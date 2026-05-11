# MarkRush

MarkRush is a fast local Markdown viewer and editor focused on large files and large folders.

The active implementation is a cross-platform Rust desktop app built with egui/eframe. The original working Windows WPF implementation is preserved in `reference/wpf` as a behavior reference.

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

## Build

Install Rust, then run:

```powershell
cargo run
```

On this machine, Rust may be installed under:

```powershell
$env:USERPROFILE\.cargo\bin\cargo.exe run
```

## Port Status

See [docs/parity-checklist.md](docs/parity-checklist.md).

## License

MIT

Bundled fonts in `assets/fonts` are distributed under the SIL Open Font License 1.1, with license text included next to each font file.
