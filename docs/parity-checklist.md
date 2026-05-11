# MarkRush Port Parity Checklist

This checklist tracks the behavior that the Rust cross-platform build must preserve from the working WPF implementation in `reference/wpf`.

## Workspace And Files

- Open a folder with a folder picker.
- Accept a folder or Markdown file from command-line launch arguments.
- Browse Markdown files from a left-side folder tree.
- Lazily expand folder tree nodes to avoid loading large workspaces at startup.
- Refresh the folder tree with a button and `F5`.
- Show the active root path, current file name, and full file path.
- Copy the current file path.
- Show selected files/folders in the platform file browser.
- Reopen the last folder and file on startup.

## Reading And Editing

- Load one active Markdown file at a time.
- Render Markdown in read mode.
- Switch between rendered read mode and raw Markdown edit mode.
- Start in read-only mode.
- Enable editing explicitly with an Edit toggle.
- Auto-enable editing on paste or cut.
- Save with a button and `Ctrl+S`.
- Prompt before losing unsaved changes.
- Preserve editor caret/scroll state and rendered scroll state per file.
- Toggle line wrapping.

## Markdown Support

- Headings.
- Paragraphs.
- Blockquotes.
- Ordered and unordered lists.
- Code blocks.
- Inline code.
- Bold and italic emphasis.
- Horizontal rules.
- Tables.
- Links.
- Heading anchors.

## Navigation

- Follow local Markdown file links.
- Follow local folder links.
- Follow heading anchors in the current file.
- Follow heading anchors after opening another local file.
- Open external URLs in the system browser.
- Resolve relative local links from the current file directory.

## Search

- Focus search with `Ctrl+F`.
- Search in rendered and edit modes.
- Next result with `Enter` or `F3`.
- Previous result with `Shift+Enter` or `Shift+F3`.
- Show match counts/status.
- Highlight the current rendered search match.

## Appearance And Settings

- Venice-style themes: EVA Dark, Pearl White, Abyss Teal, Ember Glow, Mosswood, Rose Noir.
- High Contrast theme.
- Persist selected theme.
- Persist line wrapping.
- Persist window size.
- Keep controls readable across themes.
- Rounded theme dropdown and compact tool controls.

## Reliability

- Write crash logs under the platform app-data/config directory.
- Avoid old private/local paths.
- Avoid loading ignored build outputs into the source tree.
- Keep large Markdown files with thousands of lines responsive.
