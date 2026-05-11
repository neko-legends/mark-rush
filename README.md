# MarkRush

MarkRush is a fast local Markdown viewer and editor for Windows.

The goal is simple: open a folder full of Markdown files, move through them quickly, follow local links, search, read, and make edits without the lag that shows up in heavier editors on large notes.

MarkRush is currently a Windows WPF/.NET app. A future cross-platform port may use Rust or another native toolkit, but this repository preserves the working Windows version first.

## Features

- Open a folder and browse Markdown files from a left-side tree.
- Load one Markdown file at a time for responsive navigation.
- Follow Markdown links to local Markdown files, folders, anchors, and external URLs.
- Render Markdown in a reading view.
- Switch to raw Markdown source with the Edit toggle.
- Start in read-only mode, with explicit editing when needed.
- Paste and cut automatically switch into edit mode.
- Save with the Save button or `Ctrl+S`.
- Search with `Ctrl+F`, `Enter`, `Shift+Enter`, `F3`, and `Shift+F3`.
- Preserve scroll/caret view state while moving between files and modes.
- Refresh the folder tree with the Refresh button or `F5`.
- Toggle line wrapping.
- Open a folder or file passed on the command line.
- Reopen the last folder/file on startup.
- Copy file paths and show files/folders in File Explorer.
- Includes multiple themes based on the Venice-style palette.

## Performance Goal

MarkRush is intended for large local Markdown files and folders. The design avoids loading an entire workspace into an editor surface, keeps navigation local, and focuses on one active document so files with thousands of lines remain practical to read and edit.

## Requirements

- Windows 10 or Windows 11
- .NET 9 SDK to build from source

## Build

```powershell
dotnet build .\MarkRush.csproj
```

## Run From Source

```powershell
dotnet run --project .\MarkRush.csproj
```

Or use:

```cmd
Run-MarkRush.cmd
```

## Publish

```powershell
dotnet publish .\MarkRush.csproj -c Release
```

The published executable is written under:

```text
bin\Release\net9.0-windows\win-x64\publish\MarkRush.exe
```

## License

MIT
