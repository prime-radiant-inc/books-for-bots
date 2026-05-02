//! Intermediate representation. The DOM walker emits `Block`/`Inline` trees;
//! the renderer consumes them. No behavior — pure data.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading { level: u8, text: Inline },
    Paragraph(Inline),
    BlockQuote(Vec<Block>),
    List { ordered: bool, items: Vec<Vec<Block>> },
    Table { header: Vec<Inline>, rows: Vec<Vec<Inline>> },
    CodeBlock { lang: Option<String>, code: String },
    Image { src: String, alt: String, title: Option<String> },
    HorizontalRule,
    /// Hoisted footnote definition. Rendered at end of chapter, not inline.
    FootnoteDef { id: String, content: Vec<Block> },
    /// Anchor tag injected by the assembler for non-heading IDs.
    /// Rendered as `<a id="..."></a>` on its own line.
    Anchor { id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Emphasis(Vec<Inline>),
    Strong(Vec<Inline>),
    Code(String),
    Link { href: String, children: Vec<Inline> },
    Image { src: String, alt: String, title: Option<String> },
    FootnoteRef(String),
    LineBreak,
    Concat(Vec<Inline>),
}

impl Inline {
    pub fn empty() -> Self {
        Inline::Concat(Vec::new())
    }

    /// True if every nested text leaf is empty/whitespace and there are no images,
    /// links, line breaks, footnote refs, or code spans.
    pub fn is_empty(&self) -> bool {
        match self {
            Inline::Text(s) => s.trim().is_empty(),
            Inline::Concat(xs) | Inline::Emphasis(xs) | Inline::Strong(xs) => {
                xs.iter().all(|i| i.is_empty())
            }
            Inline::Link { children, .. } => children.iter().all(|i| i.is_empty()),
            Inline::Code(s) => s.is_empty(),
            Inline::Image { .. } | Inline::FootnoteRef(_) | Inline::LineBreak => false,
        }
    }
}
