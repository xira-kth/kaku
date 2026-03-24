use std::fmt::Write as _;
use std::mem;

use anstyle::{Color, Effects, Style};
use kaku_core::{Block, CodeFence, Document, HeadingLevel, Inline, ListItem, Table};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{Theme, ThemeName};

#[derive(Debug, Clone)]
pub struct LayoutOptions {
    pub width: usize,
    pub theme: ThemeName,
    pub syntax_highlighting: bool,
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub lines: Vec<LayoutLine>,
    pub toc: Vec<TocEntry>,
}

#[derive(Debug, Clone)]
pub struct LayoutLine {
    pub spans: Vec<StyledSpan>,
    pub plain_text: String,
    pub link_indices: Vec<usize>,
}

impl LayoutLine {
    pub fn to_ansi_string(&self) -> String {
        let mut out = format!("{}", Style::new().render_reset());
        for span in &self.spans {
            let _ = write!(out, "{}", span.style.render());
            out.push_str(&span.text);
            let _ = write!(out, "{}", Style::new().render_reset());
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct StyledSpan {
    pub text: String,
    pub style: Style,
    pub link_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct TocEntry {
    pub title: String,
    pub level: HeadingLevel,
    pub line_index: usize,
}

pub fn layout_document(document: &Document, options: &LayoutOptions) -> Layout {
    let width = options.width.max(8);
    let theme = Theme::resolve(options.theme);
    let mut renderer = Renderer {
        width,
        theme,
        syntax_highlighting: options.syntax_highlighting,
        lines: Vec::new(),
        toc: Vec::new(),
    };

    for block in &document.blocks {
        renderer.render_block(block, 0);
    }

    Layout {
        lines: renderer.lines,
        toc: renderer.toc,
    }
}

struct Renderer {
    width: usize,
    theme: Theme,
    syntax_highlighting: bool,
    lines: Vec<LayoutLine>,
    toc: Vec<TocEntry>,
}

impl Renderer {
    fn render_block(&mut self, block: &Block, indent: usize) {
        match block {
            Block::Paragraph(inlines) => {
                self.push_wrapped(inlines_to_spans(inlines, self.theme), indent, "", "")
            }
            Block::Heading { level, text, .. } => self.render_heading(*level, text, indent),
            Block::Quote(blocks) => self.render_quote(blocks, indent),
            Block::List {
                ordered,
                start,
                items,
            } => self.render_list(*ordered, *start, items, indent),
            Block::CodeBlock(fence) => self.render_code_block(fence, indent),
            Block::Table(table) => self.render_table(table, indent),
            Block::Rule => self.render_rule(indent),
            Block::Html(html) => {
                self.push_wrapped(
                    vec![styled(
                        format!("[html omitted] {html}"),
                        muted(self.theme),
                        None,
                    )],
                    indent,
                    "",
                    "",
                );
            }
            Block::FootnoteDefinition(footnote) => {
                self.push_wrapped(
                    vec![styled(
                        format!("[^{}]", footnote.label),
                        emphasis(self.theme),
                        None,
                    )],
                    indent,
                    "",
                    "",
                );
                for block in &footnote.blocks {
                    self.render_block(block, indent + 2);
                }
            }
        }
    }

    fn render_heading(&mut self, level: HeadingLevel, text: &[Inline], indent: usize) {
        let prefix = format!("{} ", "#".repeat(level.as_usize()));
        let mut spans = vec![styled(
            prefix.clone(),
            heading_style(self.theme, level).effects(Effects::BOLD),
            None,
        )];
        spans.extend(
            inlines_to_spans(text, self.theme)
                .into_iter()
                .map(|mut span| {
                    span.style = heading_style(self.theme, level);
                    span
                }),
        );

        let line_index = self.lines.len();
        self.push_wrapped(spans, indent, "", &" ".repeat(prefix.len()));
        self.toc.push(TocEntry {
            title: plain_text_from_inlines(text),
            level,
            line_index,
        });
        self.lines.push(LayoutLine {
            spans: Vec::new(),
            plain_text: String::new(),
            link_indices: Vec::new(),
        });
    }

    fn render_quote(&mut self, blocks: &[Block], indent: usize) {
        for block in blocks {
            let before = self.lines.len();
            self.render_block(block, indent + 2);
            for line in &mut self.lines[before..] {
                if line.spans.is_empty() && line.plain_text.is_empty() {
                    continue;
                }
                line.spans.insert(
                    0,
                    styled(
                        format!("{}> ", " ".repeat(indent)),
                        Style::new().fg_color(Some(Color::Ansi(self.theme.quote))),
                        None,
                    ),
                );
                line.plain_text = format!("{}> {}", " ".repeat(indent), line.plain_text);
            }
        }
        self.lines.push(LayoutLine {
            spans: Vec::new(),
            plain_text: String::new(),
            link_indices: Vec::new(),
        });
    }

    fn render_list(&mut self, ordered: bool, start: usize, items: &[ListItem], indent: usize) {
        for (offset, item) in items.iter().enumerate() {
            let marker = if let Some(checked) = item.task {
                if checked { "[x] " } else { "[ ] " }.to_string()
            } else if ordered {
                format!("{}. ", start + offset)
            } else {
                "• ".to_string()
            };

            if let Some((first, rest)) = item.blocks.split_first() {
                self.render_list_item_block(first, indent, &marker);
                for block in rest {
                    self.render_block(block, indent + marker_width(&marker));
                }
            } else {
                self.push_wrapped(
                    Vec::new(),
                    indent,
                    &marker,
                    &" ".repeat(marker_width(&marker)),
                );
            }
        }
        self.lines.push(LayoutLine {
            spans: Vec::new(),
            plain_text: String::new(),
            link_indices: Vec::new(),
        });
    }

    fn render_list_item_block(&mut self, block: &Block, indent: usize, marker: &str) {
        match block {
            Block::Paragraph(inlines) => self.push_wrapped(
                inlines_to_spans(inlines, self.theme),
                indent,
                marker,
                &" ".repeat(marker_width(marker)),
            ),
            other => {
                self.push_wrapped(
                    Vec::new(),
                    indent,
                    marker,
                    &" ".repeat(marker_width(marker)),
                );
                self.render_block(other, indent + marker_width(marker));
            }
        }
    }

    fn render_code_block(&mut self, fence: &CodeFence, indent: usize) {
        let body = render_code_lines(fence, self.theme, self.syntax_highlighting);
        self.push_wrapped(
            vec![styled(
                format!("```{}", fence.language.as_deref().unwrap_or_default()),
                Style::new().fg_color(Some(Color::Ansi(self.theme.code))),
                None,
            )],
            indent,
            "",
            "",
        );

        for line in body {
            self.push_prebuilt_line(indent + 2, line);
        }

        self.push_wrapped(
            vec![styled(
                "```".to_string(),
                Style::new().fg_color(Some(Color::Ansi(self.theme.code))),
                None,
            )],
            indent,
            "",
            "",
        );
        self.lines.push(LayoutLine {
            spans: Vec::new(),
            plain_text: String::new(),
            link_indices: Vec::new(),
        });
    }

    fn render_table(&mut self, table: &Table, indent: usize) {
        let rows = std::iter::once(&table.headers)
            .chain(table.rows.iter())
            .map(|row| {
                row.iter()
                    .map(|cell| plain_text_from_inlines(cell))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let mut widths = column_widths(&rows, self.width.saturating_sub(indent).max(8));
        if widths.is_empty() {
            widths.push(1);
        }

        if !table.headers.is_empty() {
            let header = render_table_row(
                table
                    .headers
                    .iter()
                    .map(|cell| plain_text_from_inlines(cell))
                    .collect(),
                &widths,
                true,
                self.theme,
            );
            self.push_wrapped(header, indent, "", "");

            let separator = widths
                .iter()
                .map(|width| "-".repeat(*width + 2))
                .collect::<Vec<_>>()
                .join("+");
            self.push_wrapped(
                vec![styled(
                    separator,
                    Style::new().fg_color(Some(Color::Ansi(self.theme.rule))),
                    None,
                )],
                indent,
                "",
                "",
            );
        }

        for row in &table.rows {
            let rendered = render_table_row(
                row.iter()
                    .map(|cell| plain_text_from_inlines(cell))
                    .collect(),
                &widths,
                false,
                self.theme,
            );
            self.push_wrapped(rendered, indent, "", "");
        }

        self.lines.push(LayoutLine {
            spans: Vec::new(),
            plain_text: String::new(),
            link_indices: Vec::new(),
        });
    }

    fn render_rule(&mut self, indent: usize) {
        let width = self.width.saturating_sub(indent).max(4);
        self.push_wrapped(
            vec![styled(
                "─".repeat(width.min(40)),
                Style::new().fg_color(Some(Color::Ansi(self.theme.rule))),
                None,
            )],
            indent,
            "",
            "",
        );
        self.lines.push(LayoutLine {
            spans: Vec::new(),
            plain_text: String::new(),
            link_indices: Vec::new(),
        });
    }

    fn push_wrapped(
        &mut self,
        spans: Vec<StyledSpan>,
        indent: usize,
        first_prefix: &str,
        continuation_prefix: &str,
    ) {
        let rendered = wrap_spans(
            spans,
            indent,
            first_prefix,
            continuation_prefix,
            self.width.max(8),
        );
        self.lines.extend(rendered);
    }

    fn push_prebuilt_line(&mut self, indent: usize, spans: Vec<StyledSpan>) {
        let indent_text = " ".repeat(indent);
        let mut out_spans = Vec::new();
        let mut plain = indent_text.clone();
        let mut links = Vec::new();

        if !indent_text.is_empty() {
            out_spans.push(styled(indent_text, Style::new(), None));
        }

        for span in spans {
            if let Some(link) = span.link_index {
                links.push(link);
            }
            plain.push_str(&span.text);
            out_spans.push(span);
        }

        self.lines.push(LayoutLine {
            spans: out_spans,
            plain_text: plain,
            link_indices: links,
        });
    }
}

fn render_code_lines(fence: &CodeFence, theme: Theme, enabled: bool) -> Vec<Vec<StyledSpan>> {
    #[cfg(feature = "syntax")]
    {
        if enabled && !theme.monochrome {
            return crate::syntax::highlight_code(fence, theme);
        }
    }

    let style = Style::new().fg_color(Some(Color::Ansi(theme.code)));
    fence
        .code
        .lines()
        .map(|line| vec![styled(line.to_string(), style, None)])
        .collect()
}

fn render_table_row(
    cells: Vec<String>,
    widths: &[usize],
    header: bool,
    theme: Theme,
) -> Vec<StyledSpan> {
    let mut spans = Vec::new();

    for (index, width) in widths.iter().enumerate() {
        let cell = cells.get(index).cloned().unwrap_or_default();
        let truncated = truncate_to_width(&cell, *width);
        let style = if header {
            Style::new()
                .fg_color(Some(Color::Ansi(theme.heading)))
                .effects(Effects::BOLD)
        } else {
            Style::new()
        };
        spans.push(styled("│ ".to_string(), muted(theme), None));
        spans.push(styled(
            format!("{truncated:<width$}", width = *width),
            style,
            None,
        ));
        spans.push(styled(" ".to_string(), style, None));
    }
    spans.push(styled("│".to_string(), muted(theme), None));

    spans
}

fn wrap_spans(
    spans: Vec<StyledSpan>,
    indent: usize,
    first_prefix: &str,
    continuation_prefix: &str,
    width: usize,
) -> Vec<LayoutLine> {
    let indent_text = " ".repeat(indent);
    let first_prefix_full = format!("{indent_text}{first_prefix}");
    let continuation_prefix_full = format!("{indent_text}{continuation_prefix}");
    let first_width = UnicodeWidthStr::width(first_prefix_full.as_str());
    let continuation_width = UnicodeWidthStr::width(continuation_prefix_full.as_str());
    let content_width = width.max(first_width + 1);
    let continuation_content_width = width.max(continuation_width + 1);

    let mut lines = Vec::new();
    let mut current_spans = vec![styled(first_prefix_full.clone(), Style::new(), None)];
    let mut current_plain = first_prefix_full;
    let mut current_width = first_width;
    let mut current_links = Vec::new();
    let mut current_limit = content_width;
    let next_prefix = continuation_prefix_full;

    for span in spans {
        for part in split_preserving_newlines(&span.text) {
            if part == "\n" {
                lines.push(finish_line(
                    &mut current_spans,
                    &mut current_plain,
                    &mut current_links,
                ));
                current_spans = vec![styled(next_prefix.clone(), Style::new(), None)];
                current_plain = next_prefix.clone();
                current_width = continuation_width;
                current_limit = continuation_content_width;
                continue;
            }

            for grapheme in part.graphemes(true) {
                let grapheme_width = UnicodeWidthStr::width(grapheme);
                if current_width + grapheme_width > current_limit && current_width > 0 {
                    lines.push(finish_line(
                        &mut current_spans,
                        &mut current_plain,
                        &mut current_links,
                    ));
                    current_spans = vec![styled(next_prefix.clone(), Style::new(), None)];
                    current_plain = next_prefix.clone();
                    current_width = continuation_width;
                    current_limit = continuation_content_width;
                }

                current_width += grapheme_width;
                current_plain.push_str(grapheme);
                if let Some(link) = span.link_index {
                    current_links.push(link);
                }

                if let Some(last) = current_spans.last_mut() {
                    if last.style == span.style && last.link_index == span.link_index {
                        last.text.push_str(grapheme);
                    } else {
                        current_spans.push(StyledSpan {
                            text: grapheme.to_string(),
                            style: span.style,
                            link_index: span.link_index,
                        });
                    }
                }
            }
        }
    }

    if current_spans.len() > 1 || !current_plain.trim().is_empty() {
        lines.push(finish_line(
            &mut current_spans,
            &mut current_plain,
            &mut current_links,
        ));
    }

    if lines.is_empty() {
        lines.push(LayoutLine {
            spans: vec![styled(
                format!("{indent_text}{first_prefix}"),
                Style::new(),
                None,
            )],
            plain_text: format!("{indent_text}{first_prefix}"),
            link_indices: Vec::new(),
        });
    }

    lines
}

fn finish_line(
    spans: &mut Vec<StyledSpan>,
    plain_text: &mut String,
    link_indices: &mut Vec<usize>,
) -> LayoutLine {
    LayoutLine {
        spans: mem::take(spans),
        plain_text: mem::take(plain_text),
        link_indices: dedup_indices(mem::take(link_indices)),
    }
}

fn dedup_indices(mut indices: Vec<usize>) -> Vec<usize> {
    indices.sort_unstable();
    indices.dedup();
    indices
}

fn split_preserving_newlines(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;

    for (index, ch) in input.char_indices() {
        if ch == '\n' {
            if start < index {
                parts.push(&input[start..index]);
            }
            parts.push(&input[index..index + ch.len_utf8()]);
            start = index + ch.len_utf8();
        }
    }

    if start < input.len() {
        parts.push(&input[start..]);
    }

    if parts.is_empty() {
        parts.push("");
    }

    parts
}

fn inlines_to_spans(inlines: &[Inline], theme: Theme) -> Vec<StyledSpan> {
    let mut spans = Vec::new();
    append_inlines(&mut spans, inlines, Style::new(), theme);
    spans
}

fn append_inlines(
    spans: &mut Vec<StyledSpan>,
    inlines: &[Inline],
    base_style: Style,
    theme: Theme,
) {
    for inline in inlines {
        match inline {
            Inline::Text(text) => spans.push(styled(text.clone(), base_style, None)),
            Inline::Code(text) => spans.push(styled(
                text.clone(),
                Style::new().fg_color(Some(Color::Ansi(theme.code))),
                None,
            )),
            Inline::SoftBreak => spans.push(styled(" ".to_string(), base_style, None)),
            Inline::HardBreak => spans.push(styled("\n".to_string(), base_style, None)),
            Inline::Emphasis(children) => {
                append_inlines(spans, children, base_style.effects(Effects::ITALIC), theme)
            }
            Inline::Strong(children) => {
                append_inlines(spans, children, base_style.effects(Effects::BOLD), theme)
            }
            Inline::Strikethrough(children) => append_inlines(
                spans,
                children,
                base_style.effects(Effects::STRIKETHROUGH),
                theme,
            ),
            Inline::Link { text, index, .. } => {
                let style = Style::new()
                    .fg_color(Some(Color::Ansi(theme.link)))
                    .effects(Effects::UNDERLINE);
                append_link(spans, text, style, *index);
            }
        }
    }
}

fn append_link(spans: &mut Vec<StyledSpan>, inlines: &[Inline], style: Style, link_index: usize) {
    for inline in inlines {
        match inline {
            Inline::Text(text) | Inline::Code(text) => {
                spans.push(styled(text.clone(), style, Some(link_index)))
            }
            Inline::SoftBreak => spans.push(styled(" ".to_string(), style, Some(link_index))),
            Inline::HardBreak => spans.push(styled("\n".to_string(), style, Some(link_index))),
            Inline::Emphasis(children)
            | Inline::Strong(children)
            | Inline::Strikethrough(children) => append_link(spans, children, style, link_index),
            Inline::Link { text, .. } => append_link(spans, text, style, link_index),
        }
    }
}

fn plain_text_from_inlines(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(text) | Inline::Code(text) => out.push_str(text),
            Inline::SoftBreak | Inline::HardBreak => out.push(' '),
            Inline::Emphasis(children)
            | Inline::Strong(children)
            | Inline::Strikethrough(children) => out.push_str(&plain_text_from_inlines(children)),
            Inline::Link { text, .. } => out.push_str(&plain_text_from_inlines(text)),
        }
    }
    out
}

fn heading_style(theme: Theme, level: HeadingLevel) -> Style {
    let color = match level {
        HeadingLevel::H1 | HeadingLevel::H2 => theme.heading,
        HeadingLevel::H3 | HeadingLevel::H4 => theme.accent,
        HeadingLevel::H5 | HeadingLevel::H6 => theme.link,
    };

    Style::new().fg_color(Some(Color::Ansi(color)))
}

fn emphasis(theme: Theme) -> Style {
    Style::new()
        .fg_color(Some(Color::Ansi(theme.accent)))
        .effects(Effects::BOLD)
}

fn muted(theme: Theme) -> Style {
    Style::new().fg_color(Some(Color::Ansi(theme.muted)))
}

pub(crate) fn styled(text: String, style: Style, link_index: Option<usize>) -> StyledSpan {
    StyledSpan {
        text,
        style,
        link_index,
    }
}

fn marker_width(marker: &str) -> usize {
    UnicodeWidthStr::width(marker)
}

fn truncate_to_width(input: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;

    for grapheme in input.graphemes(true) {
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if used + grapheme_width > width {
            break;
        }
        used += grapheme_width;
        out.push_str(grapheme);
    }

    out
}

fn column_widths(rows: &[Vec<String>], max_width: usize) -> Vec<usize> {
    let cols = rows.iter().map(|row| row.len()).max().unwrap_or(0);
    if cols == 0 {
        return Vec::new();
    }

    let mut widths = vec![3; cols];
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(UnicodeWidthStr::width(cell.as_str()));
        }
    }

    let separator_cost = cols * 3 + 1;
    let available = max_width.saturating_sub(separator_cost).max(cols);
    let total = widths.iter().sum::<usize>();

    if total <= available {
        return widths;
    }

    let cap = available / cols;
    widths
        .into_iter()
        .map(|width| width.min(cap.max(1)))
        .collect()
}

#[cfg(test)]
mod tests {
    use kaku_core::parse_document;

    use super::{LayoutOptions, ThemeName, layout_document};

    #[test]
    fn wraps_unicode_without_panicking() {
        let doc = parse_document("# 제목\n\n한글 emoji 😀 mixed text");
        let layout = layout_document(
            &doc,
            &LayoutOptions {
                width: 12,
                theme: ThemeName::Dark,
                syntax_highlighting: false,
            },
        );

        assert!(
            layout
                .lines
                .iter()
                .any(|line| line.plain_text.contains("제목"))
        );
        assert!(layout.lines.len() > 2);
    }

    #[test]
    fn builds_toc_entries() {
        let doc = parse_document("# A\n## B\n");
        let layout = layout_document(
            &doc,
            &LayoutOptions {
                width: 80,
                theme: ThemeName::Dark,
                syntax_highlighting: false,
            },
        );

        assert_eq!(layout.toc.len(), 2);
        assert_eq!(layout.toc[1].title, "B");
    }
}
