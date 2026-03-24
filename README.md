# kaku

`kaku` is a fast Markdown viewer for terminals.

It is built as a small Rust workspace with three clean layers:

- `kaku-core` parses Markdown into a document model.
- `kaku-render` turns the document model into ANSI layout lines.
- `kaku` provides the CLI, pager loop, search, and watch mode.

## Features

- Full-screen pager and one-shot print mode
- Unicode-aware wrapping for CJK and emoji-heavy text
- Headings, lists, tables, blockquotes, task lists, links, and code blocks
- Optional syntax highlighting behind a Cargo feature
- Search, TOC panel, file watching, and stdin support

## Usage

```bash
kaku README.md
cat README.md | kaku --stdin
kaku --print README.md
kaku --watch README.md
kaku --toc README.md
```

## Pager Keys

- `j` / `Down`: scroll down
- `k` / `Up`: scroll up
- `PgDn` / `Space`: page down
- `PgUp`: page up
- `g` / `G`: top / bottom
- `/`: search
- `n` / `N`: next / previous match
- `t`: toggle TOC panel
- `Enter`: jump to the selected TOC entry
- `o`: open the first link near the current viewport
- `r`: reload
- `q`: quit

## Packaging

The workspace ships with `cargo-dist` metadata for GitHub Releases, Homebrew, and npm.
Update the repository, tap, and scope values in the root `Cargo.toml` if your release
infrastructure uses different owners.

