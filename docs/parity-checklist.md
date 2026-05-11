# MarkRush Port Parity Checklist

This checklist tracks the behavior that the Rust cross-platform build must preserve from the working WPF implementation in `reference/wpf`.

Legend: `[x]` implemented in the Rust app, `[~]` partially implemented, `[ ]` still pending.

## Workspace And Files

- [x] Open a folder with a folder picker.
- [x] Accept a folder or Markdown file from command-line launch arguments.
- [x] Browse Markdown files from a left-side folder tree.
- [x] Lazily expand folder tree nodes to avoid loading large workspaces at startup.
- [x] Refresh the folder tree with a button and `F5`.
- [x] Show the active root path, current file name, and full file path.
- [x] Copy the current file path.
- [~] Show selected files/folders in the platform file browser. Current Rust behavior opens the containing folder for the active file.
- [x] Reopen the last folder and file on startup.

## Reading And Editing

- [x] Load one active Markdown file at a time.
- [x] Render Markdown in read mode.
- [x] Switch between rendered read mode and raw Markdown edit mode.
- [x] Start in read-only mode.
- [x] Enable editing explicitly with an Edit toggle.
- [x] Auto-enable editing on paste or cut shortcuts.
- [x] Save with a button and `Ctrl+S`.
- [x] Prompt before losing unsaved changes.
- [~] Preserve editor caret/scroll state and rendered scroll state per file. Rendered scroll is persisted; editor caret restoration is pending.
- [x] Toggle line wrapping.

## Markdown Support

- [x] Headings.
- [x] Paragraphs.
- [x] Blockquotes.
- [x] Ordered and unordered lists.
- [x] Code blocks.
- [~] Inline code. Raw markers are visible in the current fast renderer.
- [~] Bold and italic emphasis. Raw markers are visible in the current fast renderer.
- [x] Horizontal rules.
- [~] Tables. Rows render quickly, but full table layout is pending.
- [x] Links.
- [x] Heading anchors.

## Navigation

- [x] Follow local Markdown file links.
- [x] Follow local folder links.
- [x] Follow heading anchors in the current file.
- [x] Follow heading anchors after opening another local file.
- [x] Open external URLs in the system browser.
- [x] Resolve relative local links from the current file directory.

## Search

- [x] Focus search with `Ctrl+F`.
- [x] Search in rendered and edit modes.
- [x] Next result with `Enter` or `F3`.
- [x] Previous result with `Shift+Enter` or `Shift+F3`.
- [x] Show match counts/status.
- [x] Highlight the current rendered search match.

## Appearance And Settings

- [x] Venice-style themes: EVA Dark, Pearl White, Abyss Teal, Ember Glow, Mosswood, Rose Noir.
- [x] High Contrast theme.
- [x] Persist selected theme.
- [x] Persist line wrapping.
- [x] Persist window size.
- [x] Keep controls readable across themes.
- [~] Rounded theme dropdown and compact tool controls. egui native controls are themed but not fully custom-drawn.

## Reliability

- [x] Write crash logs under the platform app-data/config directory.
- [x] Avoid old private/local paths.
- [x] Avoid loading ignored build outputs into the source tree.
- [x] Keep large Markdown files with thousands of lines responsive through virtualized rendered rows.
