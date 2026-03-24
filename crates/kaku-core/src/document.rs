/// Parsed Markdown document used by the renderer and pager.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Document {
    pub blocks: Vec<Block>,
    pub headings: Vec<Heading>,
    pub links: Vec<Link>,
    pub footnotes: Vec<Footnote>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Paragraph(Vec<Inline>),
    Heading {
        level: HeadingLevel,
        text: Vec<Inline>,
        id: String,
    },
    Quote(Vec<Block>),
    List {
        ordered: bool,
        start: usize,
        items: Vec<ListItem>,
    },
    CodeBlock(CodeFence),
    Table(Table),
    Rule,
    Html(String),
    FootnoteDefinition(Footnote),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeFence {
    pub language: Option<String>,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub headers: Vec<Vec<Inline>>,
    pub rows: Vec<Vec<Vec<Inline>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Code(String),
    SoftBreak,
    HardBreak,
    Emphasis(Vec<Inline>),
    Strong(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Link {
        text: Vec<Inline>,
        destination: String,
        title: String,
        index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: HeadingLevel,
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub index: usize,
    pub destination: String,
    pub title: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Footnote {
    pub label: String,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HeadingLevel {
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
}

impl HeadingLevel {
    pub fn as_usize(self) -> usize {
        match self {
            Self::H1 => 1,
            Self::H2 => 2,
            Self::H3 => 3,
            Self::H4 => 4,
            Self::H5 => 5,
            Self::H6 => 6,
        }
    }
}
