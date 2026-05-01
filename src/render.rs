use crate::block::{Block, Inline};

#[derive(Debug, Clone)]
pub struct ChapterOffset {
    /// Body-relative byte offset (0-indexed) where the chapter heading begins.
    pub byte: u64,
    /// Body-relative line number (1-indexed) where the chapter heading begins.
    pub line: u64,
}

pub struct RenderResult {
    pub body: String,
    /// One entry per chapter, in chapter order.
    pub chapter_offsets: Vec<ChapterOffset>,
}

pub struct ChapterToRender<'a> {
    pub number: usize,
    pub title: &'a str,
    pub blocks: &'a [Block],
    /// Footnote definitions to emit at chapter end, in reference order.
    pub footnotes: &'a [Block],
}

pub fn render(chapters: &[ChapterToRender<'_>]) -> RenderResult {
    let mut r = Renderer::new();
    for ch in chapters {
        r.start_chapter(ch.number, ch.title);
        for b in ch.blocks {
            r.render_block(b);
        }
        if !ch.footnotes.is_empty() {
            r.render_block(&Block::HorizontalRule);
            for f in ch.footnotes {
                r.render_block(f);
            }
        }
    }
    RenderResult { body: r.buf, chapter_offsets: r.offsets }
}

struct Renderer {
    buf: String,
    line: u64,
    offsets: Vec<ChapterOffset>,
}

impl Renderer {
    fn new() -> Self { Self { buf: String::new(), line: 1, offsets: Vec::new() } }

    fn current_byte(&self) -> u64 { self.buf.len() as u64 }

    fn ensure_blank_line(&mut self) {
        if self.buf.is_empty() { return; }
        if self.buf.ends_with("\n\n") { return; }
        if self.buf.ends_with('\n') { self.write_raw("\n"); return; }
        self.write_raw("\n\n");
    }

    fn write_raw(&mut self, s: &str) {
        self.buf.push_str(s);
        self.line += s.bytes().filter(|b| *b == b'\n').count() as u64;
    }

    fn start_chapter(&mut self, number: usize, title: &str) {
        self.ensure_blank_line();
        // Anchor first, on its own line.
        self.write_raw(&format!("<a id=\"chapter-{number}\"></a>\n"));
        // Record offset at the start of the heading line.
        self.offsets.push(ChapterOffset { byte: self.current_byte(), line: self.line });
        self.write_raw(&format!("## {title}\n\n"));
    }

    fn render_block(&mut self, b: &Block) {
        match b {
            Block::Paragraph(i) => {
                self.ensure_blank_line();
                self.render_inline(i);
                self.write_raw("\n\n");
            }
            Block::Heading { level, text } => {
                self.ensure_blank_line();
                // Per spec: in-chapter <h1> shifts to ##, <h2> to ###, etc.
                let shifted = (*level + 1).min(6);
                let hashes = "#".repeat(shifted as usize);
                self.write_raw(&format!("{hashes} "));
                self.render_inline(text);
                self.write_raw("\n\n");
            }
            Block::HorizontalRule => {
                self.ensure_blank_line();
                self.write_raw("---\n\n");
            }
            Block::BlockQuote(children) => {
                self.ensure_blank_line();
                let mut sub = Renderer::new();
                for c in children { sub.render_block(c); }
                for line in sub.buf.trim_end().split_inclusive('\n') {
                    self.write_raw("> ");
                    self.write_raw(line);
                }
                self.write_raw("\n\n");
            }
            Block::Anchor { id } => {
                self.ensure_blank_line();
                self.write_raw(&format!("<a id=\"{id}\"></a>\n"));
            }
            // Tasks 17-20 will add: List, Table, CodeBlock, Image, FootnoteDef, inline pieces.
            _ => { /* placeholder for later tasks */ }
        }
    }

    fn render_inline(&mut self, i: &Inline) {
        match i {
            Inline::Text(s) => self.write_raw(s),
            Inline::Concat(xs) => for x in xs { self.render_inline(x); },
            // Task 17 fills in: Emphasis, Strong, Code, Link, Image, FootnoteRef, LineBreak.
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{Block, Inline};

    fn render_one(blocks: Vec<Block>) -> String {
        let chs = vec![ChapterToRender {
            number: 1,
            title: "T",
            blocks: &blocks,
            footnotes: &[],
        }];
        render(&chs).body
    }

    #[test]
    fn chapter_anchor_and_heading() {
        let s = render_one(vec![]);
        assert!(s.starts_with("<a id=\"chapter-1\"></a>\n## T\n\n"));
    }

    #[test]
    fn paragraph() {
        let s = render_one(vec![Block::Paragraph(Inline::Text("hi".into()))]);
        assert!(s.contains("\nhi\n\n"));
    }

    #[test]
    fn heading_levels_shift() {
        let s = render_one(vec![
            Block::Heading { level: 1, text: Inline::Text("A".into()) },
            Block::Heading { level: 5, text: Inline::Text("B".into()) },
        ]);
        assert!(s.contains("## A\n"));
        assert!(s.contains("###### B\n"));
    }

    #[test]
    fn hr() {
        let s = render_one(vec![Block::HorizontalRule]);
        assert!(s.contains("---\n"));
    }

    #[test]
    fn blockquote() {
        let s = render_one(vec![Block::BlockQuote(vec![
            Block::Paragraph(Inline::Text("said".into())),
        ])]);
        assert!(s.contains("> said\n"));
    }
}
