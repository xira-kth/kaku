//! Markdown parsing and document modeling for `kaku`.

mod document;
mod parse;

pub use document::{
    Block, CodeFence, Document, Footnote, Heading, HeadingLevel, Inline, Link, ListItem, Table,
};
pub use parse::parse_document;
