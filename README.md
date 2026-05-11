# MarkRush

MarkRush is a fast local Markdown viewer and editor focused on large files and large folders.

The current rebuild target is a cross-platform Rust desktop app. The original working Windows WPF implementation is preserved in `reference/wpf` until the Rust build reaches feature parity.

## Goals

- Open local Markdown folders quickly.
- Read and edit files with thousands of lines without heavy editor lag.
- Follow local Markdown links and heading anchors.
- Keep the UI compact, readable, and themeable.
- Run on Windows, macOS, and Linux from one Rust codebase.

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
