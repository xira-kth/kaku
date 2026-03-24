use crate::document::{
    Block, CodeFence, Document, Footnote, Heading, HeadingLevel, Inline, Link, ListItem, Table,
};
use pulldown_cmark::{
    CodeBlockKind, Event, HeadingLevel as MdHeadingLevel, Options, Parser, Tag, TagEnd,
};

pub fn parse_document(source: &str) -> Document {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_GFM;

    let events = Parser::new_ext(source, options).collect::<Vec<_>>();
    let mut parser = MarkdownParser::new(events);

    Document {
        blocks: parser.parse_blocks_until(EndCondition::Document),
        headings: parser.headings,
        links: parser.links,
        footnotes: parser.footnotes,
    }
}

struct MarkdownParser<'a> {
    events: Vec<Event<'a>>,
    cursor: usize,
    headings: Vec<Heading>,
    links: Vec<Link>,
    footnotes: Vec<Footnote>,
}

impl<'a> MarkdownParser<'a> {
    fn new(events: Vec<Event<'a>>) -> Self {
        Self {
            events,
            cursor: 0,
            headings: Vec::new(),
            links: Vec::new(),
            footnotes: Vec::new(),
        }
    }

    fn parse_blocks_until(&mut self, end: EndCondition) -> Vec<Block> {
        let mut blocks = Vec::new();

        while let Some(event) = self.peek() {
            if end.matches(event) {
                self.next();
                break;
            }

            match self.next() {
                Some(Event::Start(tag)) => {
                    blocks.push(self.parse_block(tag));
                }
                Some(Event::Rule) => blocks.push(Block::Rule),
                Some(Event::Html(html)) | Some(Event::InlineHtml(html)) => {
                    blocks.push(Block::Html(html.into_string()));
                }
                Some(Event::Text(text)) => {
                    blocks.push(Block::Paragraph(vec![Inline::Text(text.into_string())]));
                }
                Some(Event::Code(text)) => {
                    blocks.push(Block::Paragraph(vec![Inline::Code(text.into_string())]));
                }
                Some(Event::InlineMath(text)) | Some(Event::DisplayMath(text)) => {
                    blocks.push(Block::Paragraph(vec![Inline::Code(text.into_string())]));
                }
                Some(Event::SoftBreak) | Some(Event::HardBreak) => {}
                Some(Event::End(_)) => break,
                Some(Event::FootnoteReference(label)) => {
                    blocks.push(Block::Paragraph(vec![Inline::Text(format!("[^{label}]"))]));
                }
                Some(Event::TaskListMarker(_)) => {}
                None => break,
            }
        }

        blocks
    }

    fn parse_block(&mut self, tag: Tag<'a>) -> Block {
        match tag {
            Tag::Paragraph => Block::Paragraph(self.parse_inlines_until(EndCondition::Paragraph)),
            Tag::Heading { level, .. } => self.parse_heading(level),
            Tag::BlockQuote(_) => Block::Quote(self.parse_blocks_until(EndCondition::BlockQuote)),
            Tag::List(start) => self.parse_list(start),
            Tag::Item => {
                let item = self.parse_list_item();
                Block::List {
                    ordered: false,
                    start: 1,
                    items: vec![item],
                }
            }
            Tag::CodeBlock(kind) => self.parse_code_block(kind),
            Tag::Table(_) => self.parse_table(),
            Tag::FootnoteDefinition(label) => self.parse_footnote_definition(label.into_string()),
            Tag::HtmlBlock => {
                let chunks = self.collect_text_until(EndCondition::HtmlBlock);
                Block::Html(chunks.join(""))
            }
            _ => Block::Paragraph(self.parse_inlines_for_tag(tag)),
        }
    }

    fn parse_heading(&mut self, level: MdHeadingLevel) -> Block {
        let text = self.parse_inlines_until(EndCondition::Heading(level));
        let title = plain_text(&text);
        let id = slugify(&title);

        self.headings.push(Heading {
            level: convert_heading(level),
            id: id.clone(),
            title: title.clone(),
        });

        Block::Heading {
            level: convert_heading(level),
            text,
            id,
        }
    }

    fn parse_list(&mut self, start: Option<u64>) -> Block {
        let mut items = Vec::new();

        while !self.at_end(EndCondition::List) {
            match self.next() {
                Some(Event::Start(Tag::Item)) => items.push(self.parse_list_item()),
                Some(_) => {}
                None => break,
            }
        }

        self.consume_end(EndCondition::List);

        Block::List {
            ordered: start.is_some(),
            start: start.unwrap_or(1) as usize,
            items,
        }
    }

    fn parse_list_item(&mut self) -> ListItem {
        let mut task = None;
        let mut blocks = Vec::new();

        if let Some(Event::TaskListMarker(checked)) = self.peek() {
            task = Some(*checked);
            self.next();
        }

        while !self.at_end(EndCondition::Item) {
            match self.next() {
                Some(Event::Start(tag)) => blocks.push(self.parse_block(tag)),
                Some(Event::Text(text)) => {
                    blocks.push(Block::Paragraph(vec![Inline::Text(text.into_string())]));
                }
                Some(Event::Rule) => blocks.push(Block::Rule),
                Some(_) => {}
                None => break,
            }
        }

        self.consume_end(EndCondition::Item);

        ListItem { task, blocks }
    }

    fn parse_code_block(&mut self, kind: CodeBlockKind<'a>) -> Block {
        let language = match kind {
            CodeBlockKind::Fenced(name) if !name.is_empty() => Some(name.into_string()),
            _ => None,
        };

        let code = self.collect_text_until(EndCondition::CodeBlock).join("");
        Block::CodeBlock(CodeFence { language, code })
    }

    fn parse_table(&mut self) -> Block {
        let mut headers = Vec::new();
        let mut rows = Vec::new();

        while !self.at_end(EndCondition::Table) {
            match self.next() {
                Some(Event::Start(Tag::TableHead)) => {
                    headers = self.parse_table_head();
                }
                Some(Event::Start(Tag::TableRow)) => {
                    rows.push(self.parse_table_row());
                }
                Some(_) => {}
                None => break,
            }
        }

        self.consume_end(EndCondition::Table);
        Block::Table(Table { headers, rows })
    }

    fn parse_table_head(&mut self) -> Vec<Vec<Inline>> {
        let mut cells = Vec::new();

        while !self.at_end(EndCondition::TableHead) {
            match self.next() {
                Some(Event::Start(Tag::TableCell)) => {
                    cells.push(self.parse_inlines_until(EndCondition::TableCell));
                }
                Some(_) => {}
                None => break,
            }
        }

        self.consume_end(EndCondition::TableHead);
        cells
    }

    fn parse_table_row(&mut self) -> Vec<Vec<Inline>> {
        let mut cells = Vec::new();

        while !self.at_end(EndCondition::TableRow) {
            match self.next() {
                Some(Event::Start(Tag::TableCell)) => {
                    cells.push(self.parse_inlines_until(EndCondition::TableCell));
                }
                Some(_) => {}
                None => break,
            }
        }

        self.consume_end(EndCondition::TableRow);
        cells
    }

    fn parse_footnote_definition(&mut self, label: String) -> Block {
        let blocks = self.parse_blocks_until(EndCondition::FootnoteDefinition);
        let footnote = Footnote {
            label: label.clone(),
            blocks: blocks.clone(),
        };
        self.footnotes.push(footnote.clone());
        Block::FootnoteDefinition(footnote)
    }

    fn parse_inlines_for_tag(&mut self, tag: Tag<'a>) -> Vec<Inline> {
        let end = EndCondition::from_inline_tag(&tag);
        self.parse_inlines_until(end)
    }

    fn parse_inlines_until(&mut self, end: EndCondition) -> Vec<Inline> {
        let mut inlines = Vec::new();

        while let Some(event) = self.peek() {
            if end.matches(event) {
                self.next();
                break;
            }

            match self.next() {
                Some(Event::Text(text)) => inlines.push(Inline::Text(text.into_string())),
                Some(Event::Code(code)) => inlines.push(Inline::Code(code.into_string())),
                Some(Event::InlineMath(text)) | Some(Event::DisplayMath(text)) => {
                    inlines.push(Inline::Code(text.into_string()))
                }
                Some(Event::SoftBreak) => inlines.push(Inline::SoftBreak),
                Some(Event::HardBreak) => inlines.push(Inline::HardBreak),
                Some(Event::Html(html)) | Some(Event::InlineHtml(html)) => {
                    inlines.push(Inline::Text(html.into_string()))
                }
                Some(Event::FootnoteReference(label)) => {
                    inlines.push(Inline::Text(format!("[^{label}]")));
                }
                Some(Event::Start(Tag::Emphasis)) => {
                    inlines.push(Inline::Emphasis(
                        self.parse_inlines_until(EndCondition::Emphasis),
                    ));
                }
                Some(Event::Start(Tag::Strong)) => {
                    inlines.push(Inline::Strong(
                        self.parse_inlines_until(EndCondition::Strong),
                    ));
                }
                Some(Event::Start(Tag::Strikethrough)) => inlines.push(Inline::Strikethrough(
                    self.parse_inlines_until(EndCondition::Strikethrough),
                )),
                Some(Event::Start(Tag::Link {
                    dest_url, title, ..
                })) => {
                    let text = self.parse_inlines_until(EndCondition::Link);
                    let index = self.links.len();
                    let label = plain_text(&text);

                    self.links.push(Link {
                        index,
                        destination: dest_url.to_string(),
                        title: title.to_string(),
                        label: label.clone(),
                    });

                    inlines.push(Inline::Link {
                        text,
                        destination: dest_url.to_string(),
                        title: title.to_string(),
                        index,
                    });
                }
                Some(Event::Start(Tag::Image {
                    dest_url, title, ..
                })) => {
                    let alt_text = self.parse_inlines_until(EndCondition::Image);
                    let alt = plain_text(&alt_text);
                    let label = if alt.is_empty() {
                        format!("[image] {}", dest_url)
                    } else {
                        format!("[image: {alt}] {}", dest_url)
                    };

                    let index = self.links.len();
                    self.links.push(Link {
                        index,
                        destination: dest_url.to_string(),
                        title: title.to_string(),
                        label: alt,
                    });
                    inlines.push(Inline::Text(label));
                }
                Some(Event::Start(tag)) => {
                    inlines.extend(self.parse_inlines_for_tag(tag));
                }
                Some(Event::TaskListMarker(_)) => {}
                Some(Event::Rule) => inlines.push(Inline::Text("---".to_string())),
                Some(Event::End(_)) => break,
                None => break,
            }
        }

        inlines
    }

    fn collect_text_until(&mut self, end: EndCondition) -> Vec<String> {
        let mut chunks = Vec::new();

        while let Some(event) = self.peek() {
            if end.matches(event) {
                self.next();
                break;
            }

            match self.next() {
                Some(Event::Text(text)) | Some(Event::Code(text)) => {
                    chunks.push(text.into_string())
                }
                Some(Event::SoftBreak) | Some(Event::HardBreak) => chunks.push("\n".to_string()),
                Some(Event::Html(text)) | Some(Event::InlineHtml(text)) => {
                    chunks.push(text.into_string())
                }
                Some(Event::Start(tag)) => {
                    chunks.push(plain_text(&self.parse_inlines_for_tag(tag)));
                }
                Some(_) => {}
                None => break,
            }
        }

        chunks
    }

    fn at_end(&self, end: EndCondition) -> bool {
        self.peek().is_some_and(|event| end.matches(event))
    }

    fn consume_end(&mut self, end: EndCondition) {
        if self.at_end(end) {
            self.next();
        }
    }

    fn peek(&self) -> Option<&Event<'a>> {
        self.events.get(self.cursor)
    }

    fn next(&mut self) -> Option<Event<'a>> {
        let event = self.events.get(self.cursor).cloned();
        if event.is_some() {
            self.cursor += 1;
        }
        event
    }
}

#[derive(Debug, Clone, Copy)]
enum EndCondition {
    Document,
    Paragraph,
    Heading(MdHeadingLevel),
    BlockQuote,
    List,
    Item,
    CodeBlock,
    Table,
    TableHead,
    TableRow,
    TableCell,
    FootnoteDefinition,
    Emphasis,
    Strong,
    Strikethrough,
    Link,
    Image,
    HtmlBlock,
    Other,
}

impl EndCondition {
    fn from_inline_tag(tag: &Tag<'_>) -> Self {
        match tag {
            Tag::Emphasis => Self::Emphasis,
            Tag::Strong => Self::Strong,
            Tag::Strikethrough => Self::Strikethrough,
            Tag::Link { .. } => Self::Link,
            Tag::Image { .. } => Self::Image,
            Tag::Paragraph => Self::Paragraph,
            Tag::Heading { level, .. } => Self::Heading(*level),
            Tag::HtmlBlock => Self::HtmlBlock,
            _ => Self::Other,
        }
    }

    fn matches(self, event: &Event<'_>) -> bool {
        match (self, event) {
            (Self::Document, _) => false,
            (Self::Paragraph, Event::End(TagEnd::Paragraph)) => true,
            (Self::Heading(level), Event::End(TagEnd::Heading(end))) => level == *end,
            (Self::BlockQuote, Event::End(TagEnd::BlockQuote(_))) => true,
            (Self::List, Event::End(TagEnd::List(_))) => true,
            (Self::Item, Event::End(TagEnd::Item)) => true,
            (Self::CodeBlock, Event::End(TagEnd::CodeBlock)) => true,
            (Self::Table, Event::End(TagEnd::Table)) => true,
            (Self::TableHead, Event::End(TagEnd::TableHead)) => true,
            (Self::TableRow, Event::End(TagEnd::TableRow)) => true,
            (Self::TableCell, Event::End(TagEnd::TableCell)) => true,
            (Self::FootnoteDefinition, Event::End(TagEnd::FootnoteDefinition)) => true,
            (Self::Emphasis, Event::End(TagEnd::Emphasis)) => true,
            (Self::Strong, Event::End(TagEnd::Strong)) => true,
            (Self::Strikethrough, Event::End(TagEnd::Strikethrough)) => true,
            (Self::Link, Event::End(TagEnd::Link)) => true,
            (Self::Image, Event::End(TagEnd::Image)) => true,
            (Self::HtmlBlock, Event::End(TagEnd::HtmlBlock)) => true,
            (Self::Other, Event::End(_)) => true,
            _ => false,
        }
    }
}

fn convert_heading(level: MdHeadingLevel) -> HeadingLevel {
    match level {
        MdHeadingLevel::H1 => HeadingLevel::H1,
        MdHeadingLevel::H2 => HeadingLevel::H2,
        MdHeadingLevel::H3 => HeadingLevel::H3,
        MdHeadingLevel::H4 => HeadingLevel::H4,
        MdHeadingLevel::H5 => HeadingLevel::H5,
        MdHeadingLevel::H6 => HeadingLevel::H6,
    }
}

fn plain_text(inlines: &[Inline]) -> String {
    let mut out = String::new();

    for inline in inlines {
        match inline {
            Inline::Text(text) | Inline::Code(text) => out.push_str(text),
            Inline::SoftBreak | Inline::HardBreak => out.push(' '),
            Inline::Emphasis(children)
            | Inline::Strong(children)
            | Inline::Strikethrough(children) => out.push_str(&plain_text(children)),
            Inline::Link { text, .. } => out.push_str(&plain_text(text)),
        }
    }

    out
}

fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in input.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if (ch.is_whitespace() || ch == '-' || ch == '_')
            && !last_was_dash
            && !slug.is_empty()
        {
            slug.push('-');
            last_was_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::parse_document;
    use crate::{Block, Inline};

    #[test]
    fn parses_headings_links_lists_and_tables() {
        let doc = parse_document(
            r#"# Title

- [x] done
- [ ] pending

| A | B |
|---|---|
| 1 | 2 |

[docs](https://example.com)
"#,
        );

        assert_eq!(doc.headings.len(), 1);
        assert_eq!(doc.links.len(), 1);

        match &doc.blocks[1] {
            Block::List { items, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].task, Some(true));
                assert_eq!(items[1].task, Some(false));
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn turns_images_into_placeholders() {
        let doc = parse_document("![logo](https://example.com/logo.png)");

        match &doc.blocks[0] {
            Block::Paragraph(inlines) => {
                assert!(
                    matches!(&inlines[0], Inline::Text(text) if text.contains("[image: logo]"))
                );
            }
            other => panic!("expected paragraph, got {other:?}"),
        }
    }
}
