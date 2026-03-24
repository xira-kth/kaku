# kaku

`kaku` is a fast, minimal Markdown reader for terminals.

It is built as a small Rust workspace with three clean layers:

- `kaku-core` parses Markdown into a document model.
- `kaku-render` turns the document model into ANSI layout lines.
- `kaku` provides the CLI, pager loop, search, and watch mode.

## Philosophy

- Read one document well
- Stay local and terminal-native
- Use a centered reading column on wide terminals
- Prefer calm output over decorative styling
- Keep the command surface small

## Features

- Full-screen pager for local files and stdin
- Plain stdout output with `-p`
- Unicode-aware wrapping for CJK and emoji-heavy text
- Headings, lists, tables, blockquotes, task lists, links, and code blocks
- Search, TOC panel, file watching, and stdin support
- Optional syntax highlighting for source builds with `--features syntax`

## Usage

```bash
kaku README.md
kaku -
cat README.md | kaku
kaku -p README.md
kaku -w README.md
kaku -t README.md
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
- `?`: show a short key hint
- `q`: quit

## Packaging

The workspace ships with `cargo-dist` metadata for GitHub Releases, Homebrew, and npm.
See [docs/RELEASING.md](/Users/voidique/folders/kaku/docs/RELEASING.md) for the current release flow.
