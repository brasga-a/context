use crate::{Diagnostic, Span};

/// The parsed representation of a complete source document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub frontmatter: Option<FrontmatterBlock>,
    pub children: Vec<Block>,
    pub definitions: Vec<LinkDefinition>,
    pub diagnostics: Vec<Diagnostic>,
}

/// A link reference definition collected from a paragraph's leading lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkDefinition {
    pub span: Span,
    pub label: Span,
    pub dest: Span,
    pub title: Option<Span>,
}

/// A frontmatter region recognized at the beginning of the document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrontmatterBlock {
    pub span: Span,
    pub terminated: bool,
}

/// A block-level document node.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Block {
    Paragraph {
        span: Span,
        content: Vec<Inline>,
    },
    Heading {
        span: Span,
        level: u8,
        kind: HeadingKind,
        content: Vec<Inline>,
    },
    ThematicBreak {
        span: Span,
    },
    CodeBlock {
        span: Span,
        kind: CodeBlockKind,
        literal: Vec<Span>,
    },
    BlockQuote {
        span: Span,
        children: Vec<Block>,
    },
    List {
        span: Span,
        kind: ListKind,
        tight: bool,
        items: Vec<ListItem>,
    },
    Table {
        span: Span,
        alignments: Vec<TableAlignment>,
        head: TableRow,
        rows: Vec<TableRow>,
    },
    FootnoteDefinition {
        span: Span,
        label: Span,
        children: Vec<Block>,
    },
}

/// An inline node within paragraph or heading content.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Inline {
    Text {
        span: Span,
    },
    Escaped {
        span: Span,
    },
    CharacterReference {
        span: Span,
        value: String,
    },
    CodeSpan {
        span: Span,
        literal: Vec<Span>,
    },
    Emphasis {
        span: Span,
        children: Vec<Inline>,
    },
    Strong {
        span: Span,
        children: Vec<Inline>,
    },
    Link {
        span: Span,
        target: LinkTarget,
        children: Vec<Inline>,
    },
    Image {
        span: Span,
        target: LinkTarget,
        children: Vec<Inline>,
    },
    Autolink {
        span: Span,
        uri: Span,
        email: bool,
    },
    WikiLink {
        span: Span,
        target: Span,
        label: Option<Span>,
    },
    Strikethrough {
        span: Span,
        children: Vec<Inline>,
    },
    Highlight {
        span: Span,
        children: Vec<Inline>,
    },
    Math {
        span: Span,
        display: bool,
        literal: Vec<Span>,
    },
    FootnoteReference {
        span: Span,
        label: Span,
    },
    SoftBreak {
        span: Span,
    },
    HardBreak {
        span: Span,
    },
}

/// Source-backed destination metadata shared by links and images.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkTarget {
    pub dest: Option<Span>,
    pub title: Option<Span>,
    pub label: Option<Span>,
}

/// The syntax used to form a heading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadingKind {
    Atx,
    Setext,
}

/// The syntax used to form a code block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeBlockKind {
    Fenced { info: Option<Span> },
    Indented,
}

/// The marker family used by a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKind {
    Bullet { marker: u8 },
    Ordered { start: u32, delimiter: u8 },
}

/// Alignment selected by a table delimiter cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableAlignment {
    None,
    Left,
    Center,
    Right,
}

/// One source row in a table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableRow {
    pub span: Span,
    pub cells: Vec<TableCell>,
}

/// One trimmed table cell and its parsed inline content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableCell {
    pub span: Span,
    pub content: Vec<Inline>,
}

/// One item in a list container.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem {
    pub span: Span,
    pub task: Option<bool>,
    pub children: Vec<Block>,
}

#[derive(Debug)]
pub(crate) struct RawDocument {
    pub(crate) frontmatter: Option<FrontmatterBlock>,
    pub(crate) children: Vec<RawBlock>,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
pub(crate) enum RawBlock {
    Paragraph {
        span: Span,
        fragments: Vec<Span>,
    },
    Heading {
        span: Span,
        level: u8,
        kind: HeadingKind,
        fragments: Vec<Span>,
    },
    ThematicBreak {
        span: Span,
    },
    CodeBlock {
        span: Span,
        kind: CodeBlockKind,
        literal: Vec<Span>,
    },
    BlockQuote {
        span: Span,
        children: Vec<RawBlock>,
    },
    List {
        span: Span,
        kind: ListKind,
        tight: bool,
        items: Vec<RawListItem>,
    },
    Table {
        span: Span,
        alignments: Vec<TableAlignment>,
        head: RawTableRow,
        rows: Vec<RawTableRow>,
    },
    FootnoteDefinition {
        span: Span,
        label: Span,
        children: Vec<RawBlock>,
    },
}

#[derive(Debug)]
pub(crate) struct RawListItem {
    pub(crate) span: Span,
    pub(crate) task: Option<bool>,
    pub(crate) children: Vec<RawBlock>,
}

#[derive(Debug)]
pub(crate) struct RawTableRow {
    pub(crate) span: Span,
    pub(crate) cells: Vec<Span>,
}
