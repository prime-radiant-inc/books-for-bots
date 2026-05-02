use crate::block::{Block, Inline};

fn backtick_fence_for(s: &str) -> String {
    let mut max_run = 0usize;
    let mut cur = 0usize;
    for c in s.chars() {
        if c == '`' { cur += 1; max_run = max_run.max(cur); } else { cur = 0; }
    }
    "`".repeat(max_run + 1)
}

/// Collapse every run of ASCII whitespace down to a single space.
/// Used inside heading lines, where adjacent text nodes around a <br/>
/// in the source can produce stray double-spaces.
fn collapse_runs_of_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out
}

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
    pub book_title: &'a str,
    pub blocks: &'a [Block],
    /// Footnote definitions to emit at chapter end, in reference order.
    pub footnotes: &'a [Block],
}

pub fn render(chapters: &[ChapterToRender<'_>]) -> RenderResult {
    let mut r = Renderer::new();
    for ch in chapters {
        r.start_chapter(ch.number, ch.title);
        let drop_set: std::collections::BTreeSet<usize> =
            find_auxiliary_heading_indices(ch.blocks, ch.title, ch.book_title)
                .into_iter().collect();
        for (i, b) in ch.blocks.iter().enumerate() {
            if drop_set.contains(&i) { continue; }
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

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Normalize for comparison: whitespace-collapse then strip hyphens/en-dashes/em-dashes
/// so "Man-Month" and "Man Month" and "Man–Month" all compare equal.
fn normalize_for_match(s: &str) -> String {
    normalize_ws(&s.replace(['-', '\u{2013}', '\u{2014}'], " "))
}

fn heading_matches(h_norm: &str, target_norm: &str) -> bool {
    if h_norm == target_norm { return true; }
    // Also try with dashes stripped so "Man-Month" matches "Man Month".
    let h_dash = normalize_for_match(h_norm);
    let t_dash = normalize_for_match(target_norm);
    if h_dash == t_dash { return true; }
    if h_norm.len() < 4 || target_norm.len() < 4 { return false; }
    if target_norm.contains(h_norm) || h_norm.contains(target_norm) { return true; }
    // Fuzzy contains with dashes stripped.
    if h_dash.len() >= 4 && t_dash.len() >= 4 {
        if t_dash.contains(&h_dash) || h_dash.contains(&t_dash) { return true; }
    }
    false
}

/// Return indices of leading auxiliary heading blocks to drop.
/// "Auxiliary" means: heading at level <= 2 in the first ~8 blocks whose
/// text matches (whitespace-normalized) either the chapter title or the
/// book title, with a fuzzy contains-match fallback. Stops scanning at
/// the first content-bearing block.
fn find_auxiliary_heading_indices(
    blocks: &[Block],
    chapter_title: &str,
    book_title: &str,
) -> Vec<usize> {
    let mut to_drop = Vec::new();
    let limit = blocks.len().min(8);
    let chap_norm = normalize_ws(chapter_title);
    let book_norm = normalize_ws(book_title);
    for (i, b) in blocks[..limit].iter().enumerate() {
        match b {
            Block::Heading { level, text } if *level <= 2 => {
                let h_norm = normalize_ws(&inline_to_text(text));
                let matches_chap = heading_matches(&h_norm, &chap_norm);
                let matches_book = !book_norm.is_empty() && heading_matches(&h_norm, &book_norm);
                if matches_chap || matches_book {
                    to_drop.push(i);
                    continue;
                }
                // First non-matching heading — stop scanning.
                return to_drop;
            }
            // Auxiliary: keep scanning past these (they stay in the output unless dropped).
            Block::Paragraph(inl) if inl.is_empty() => continue,
            Block::Image { .. } => continue,
            Block::Anchor { .. } => continue,
            // Content block reached — stop.
            _ => return to_drop,
        }
    }
    to_drop
}

fn inline_to_text(i: &Inline) -> String {
    match i {
        Inline::Text(s) => s.clone(),
        Inline::Concat(xs) | Inline::Emphasis(xs) | Inline::Strong(xs) => {
            xs.iter().map(inline_to_text).collect()
        }
        Inline::Link { children, .. } => children.iter().map(inline_to_text).collect(),
        Inline::Code(s) => s.clone(),
        Inline::Image { alt, .. } => alt.clone(),
        Inline::FootnoteRef(_) => String::new(),
        Inline::LineBreak => " ".to_string(),
    }
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
        let _ = number; // chapter number is used by upstream namespacing, not by render
        self.ensure_blank_line();
        // Record offset at the heading line.
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
                // Render the heading's inline content to a scratch buffer, then
                // collapse runs of whitespace (XHTML indentation between inline
                // elements often produces stray double-spaces around <br/>).
                let mut sub = Renderer::new();
                sub.render_inline_for_heading(text);
                let collapsed = collapse_runs_of_whitespace(&sub.buf);
                self.write_raw(&format!("{hashes} "));
                self.write_raw(collapsed.trim());
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
            Block::Anchor { .. } => {
                // No-op: explicit IDs aren't markdown-native. Within-doc links
                // resolve via heading auto-slugs in conformant renderers.
            }
            Block::Image { src, alt, title } => {
                self.ensure_blank_line();
                match title {
                    Some(t) => self.write_raw(&format!(r#"![{alt}]({src} "{t}")"#)),
                    None => self.write_raw(&format!("![{alt}]({src})")),
                }
                self.write_raw("\n\n");
            }
            Block::List { ordered, items } => {
                self.ensure_blank_line();
                for (idx, item) in items.iter().enumerate() {
                    let marker = if *ordered {
                        format!("{}. ", idx + 1)
                    } else {
                        "- ".to_string()
                    };
                    let indent = " ".repeat(marker.len());
                    let mut sub = Renderer::new();
                    for b in item { sub.render_block(b); }
                    let trimmed = sub.buf.trim();
                    let mut first_line = true;
                    for line in trimmed.split_inclusive('\n') {
                        if line.trim().is_empty() {
                            continue;
                        }
                        if first_line {
                            self.write_raw(&marker);
                            first_line = false;
                        } else {
                            self.write_raw(&indent);
                        }
                        self.write_raw(line);
                        if !line.ends_with('\n') {
                            self.write_raw("\n");
                        }
                    }
                }
                self.write_raw("\n");
            }
            Block::Table { header, rows } => {
                self.ensure_blank_line();
                self.write_raw("| ");
                for (i, h) in header.iter().enumerate() {
                    if i > 0 { self.write_raw(" | "); }
                    self.render_cell(h);
                }
                self.write_raw(" |\n| ");
                for i in 0..header.len() {
                    if i > 0 { self.write_raw(" | "); }
                    self.write_raw("---");
                }
                self.write_raw(" |\n");
                for row in rows {
                    self.write_raw("| ");
                    for (i, c) in row.iter().enumerate() {
                        if i > 0 { self.write_raw(" | "); }
                        self.render_cell(c);
                    }
                    self.write_raw(" |\n");
                }
                self.write_raw("\n");
            }
            Block::CodeBlock { lang, code } => {
                self.ensure_blank_line();
                // Find the longest run of backticks anywhere in the code.
                let mut max_run = 0usize;
                let mut cur = 0usize;
                for c in code.chars() {
                    if c == '`' {
                        cur += 1;
                        if cur > max_run { max_run = cur; }
                    } else {
                        cur = 0;
                    }
                }
                let fence_len = max_run.max(2) + 1;
                let fence = "`".repeat(fence_len);
                self.write_raw(&fence);
                if let Some(l) = lang { self.write_raw(l); }
                self.write_raw("\n");
                self.write_raw(code);
                if !code.ends_with('\n') { self.write_raw("\n"); }
                self.write_raw(&fence);
                self.write_raw("\n\n");
            }
            Block::FootnoteDef { id, content } => {
                self.ensure_blank_line();
                let mut sub = Renderer::new();
                for c in content { sub.render_block(c); }
                let body = sub.buf.trim().to_string();
                self.write_raw(&format!("[^{id}]: "));
                // Continuation lines indented by 4 spaces per CommonMark footnotes.
                let mut first = true;
                for line in body.split_inclusive('\n') {
                    if first { first = false; } else { self.write_raw("    "); }
                    self.write_raw(line);
                }
                self.write_raw("\n\n");
            }
            // All Block variants are explicitly handled above; no catch-all needed.
        }
    }

    fn render_cell(&mut self, i: &Inline) {
        let mut tmp = Renderer::new();
        tmp.render_inline(i);
        let escaped = tmp.buf.replace('\n', "<br>").replace('|', "\\|");
        self.write_raw(&escaped);
    }

    fn render_inline(&mut self, i: &Inline) {
        match i {
            Inline::Text(s) => self.write_raw(s),
            Inline::Concat(xs) => for x in xs { self.render_inline(x); },
            Inline::Emphasis(xs) => {
                self.write_raw("*");
                for x in xs { self.render_inline(x); }
                self.write_raw("*");
            }
            Inline::Strong(xs) => {
                self.write_raw("**");
                for x in xs { self.render_inline(x); }
                self.write_raw("**");
            }
            Inline::Code(s) => {
                let fence = backtick_fence_for(s);
                self.write_raw(&fence);
                if s.starts_with('`') { self.write_raw(" "); }
                self.write_raw(s);
                if s.ends_with('`') { self.write_raw(" "); }
                self.write_raw(&fence);
            }
            Inline::Link { href, children } => {
                self.write_raw("[");
                for c in children { self.render_inline(c); }
                self.write_raw(&format!("]({href})"));
            }
            Inline::Image { src, alt, title } => {
                match title {
                    Some(t) => self.write_raw(&format!(r#"![{alt}]({src} "{t}")"#)),
                    None => self.write_raw(&format!("![{alt}]({src})")),
                }
            }
            Inline::FootnoteRef(id) => self.write_raw(&format!("[^{id}]")),
            Inline::LineBreak => self.write_raw("  \n"),
        }
    }

    fn render_inline_for_heading(&mut self, i: &Inline) {
        match i {
            Inline::Text(s) => self.write_raw(s),
            Inline::Concat(xs) => for x in xs { self.render_inline_for_heading(x); },
            Inline::Emphasis(xs) => {
                self.write_raw("*");
                for x in xs { self.render_inline_for_heading(x); }
                self.write_raw("*");
            }
            Inline::Strong(xs) => {
                self.write_raw("**");
                for x in xs { self.render_inline_for_heading(x); }
                self.write_raw("**");
            }
            Inline::Code(s) => {
                let fence = backtick_fence_for(s);
                self.write_raw(&fence);
                if s.starts_with('`') { self.write_raw(" "); }
                self.write_raw(s);
                if s.ends_with('`') { self.write_raw(" "); }
                self.write_raw(&fence);
            }
            Inline::Link { href, children } => {
                self.write_raw("[");
                for c in children { self.render_inline_for_heading(c); }
                self.write_raw(&format!("]({href})"));
            }
            Inline::Image { src, alt, title } => {
                match title {
                    Some(t) => self.write_raw(&format!(r#"![{alt}]({src} "{t}")"#)),
                    None => self.write_raw(&format!("![{alt}]({src})")),
                }
            }
            Inline::FootnoteRef(id) => self.write_raw(&format!("[^{id}]")),
            // In headings, LineBreaks become spaces. Surrounding stray spaces
            // are collapsed by collapse_runs_of_whitespace at the heading-block
            // emission site.
            Inline::LineBreak => self.write_raw(" "),
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
            book_title: "",
            blocks: &blocks,
            footnotes: &[],
        }];
        render(&chs).body
    }

    #[test]
    fn chapter_heading_no_anchor() {
        let s = render_one(vec![]);
        assert!(s.starts_with("## T\n\n"), "got: {s}");
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

    #[test]
    fn emphasis_and_strong() {
        let s = render_one(vec![Block::Paragraph(Inline::Concat(vec![
            Inline::Emphasis(vec![Inline::Text("a".into())]),
            Inline::Text(" and ".into()),
            Inline::Strong(vec![Inline::Text("b".into())]),
        ]))]);
        assert!(s.contains("*a* and **b**"));
    }

    #[test]
    fn inline_code_with_backtick() {
        let s = render_one(vec![Block::Paragraph(Inline::Code("x`y".into()))]);
        assert!(s.contains("``x`y``"));
    }

    #[test]
    fn link() {
        let s = render_one(vec![Block::Paragraph(Inline::Link {
            href: "h".into(), children: vec![Inline::Text("t".into())],
        })]);
        assert!(s.contains("[t](h)"));
    }

    #[test]
    fn line_break() {
        let s = render_one(vec![Block::Paragraph(Inline::Concat(vec![
            Inline::Text("a".into()), Inline::LineBreak, Inline::Text("b".into()),
        ]))]);
        assert!(s.contains("a  \nb"));
    }

    #[test]
    fn footnote_ref() {
        let s = render_one(vec![Block::Paragraph(Inline::FootnoteRef("c1-fn1".into()))]);
        assert!(s.contains("[^c1-fn1]"));
    }

    #[test]
    fn block_image() {
        let s = render_one(vec![Block::Image {
            src: "images/x.jpg".into(), alt: "cat".into(), title: None,
        }]);
        assert!(s.contains("![cat](images/x.jpg)"));
    }

    #[test]
    fn block_image_with_title() {
        let s = render_one(vec![Block::Image {
            src: "x.jpg".into(), alt: "a".into(), title: Some("t".into()),
        }]);
        assert!(s.contains(r#"![a](x.jpg "t")"#));
    }

    #[test]
    fn unordered_list() {
        let s = render_one(vec![Block::List {
            ordered: false,
            items: vec![
                vec![Block::Paragraph(Inline::Text("a".into()))],
                vec![Block::Paragraph(Inline::Text("b".into()))],
            ],
        }]);
        assert!(s.contains("- a\n- b\n"), "got: {s}");
    }

    #[test]
    fn ordered_list() {
        let s = render_one(vec![Block::List {
            ordered: true,
            items: vec![
                vec![Block::Paragraph(Inline::Text("x".into()))],
                vec![Block::Paragraph(Inline::Text("y".into()))],
            ],
        }]);
        assert!(s.contains("1. x\n2. y\n"));
    }

    #[test]
    fn nested_list_indents_two_spaces() {
        let s = render_one(vec![Block::List {
            ordered: false,
            items: vec![vec![
                Block::Paragraph(Inline::Text("outer".into())),
                Block::List {
                    ordered: false,
                    items: vec![vec![Block::Paragraph(Inline::Text("inner".into()))]],
                },
            ]],
        }]);
        assert!(s.contains("- outer\n  - inner\n"), "got: {s}");
    }

    #[test]
    fn pipe_table() {
        let s = render_one(vec![Block::Table {
            header: vec![Inline::Text("A".into()), Inline::Text("B".into())],
            rows: vec![
                vec![Inline::Text("1".into()), Inline::Text("2".into())],
                vec![Inline::Text("3 | x".into()), Inline::Text("4".into())],
            ],
        }]);
        assert!(s.contains("| A | B |\n| --- | --- |\n| 1 | 2 |\n| 3 \\| x | 4 |\n"), "got: {s}");
    }

    #[test]
    fn code_block_no_language() {
        let s = render_one(vec![Block::CodeBlock { lang: None, code: "x = 1".into() }]);
        assert!(s.contains("```\nx = 1\n```\n"));
    }

    #[test]
    fn code_block_with_language() {
        let s = render_one(vec![Block::CodeBlock { lang: Some("rs".into()), code: "fn main(){}".into() }]);
        assert!(s.contains("```rs\nfn main(){}\n```\n"));
    }

    #[test]
    fn code_block_with_internal_triple_backticks() {
        let s = render_one(vec![Block::CodeBlock {
            lang: None,
            code: "echo \"```\"".into(),
        }]);
        // Should use a 4-backtick fence.
        assert!(s.contains("````\necho \"```\"\n````\n"), "got: {s}");
    }

    #[test]
    fn footnote_def() {
        let s = render_one(vec![Block::FootnoteDef {
            id: "c1-fn1".into(),
            content: vec![Block::Paragraph(Inline::Text("Note.".into()))],
        }]);
        assert!(s.contains("[^c1-fn1]: Note.\n"), "got: {s}");
    }

    #[test]
    fn first_heading_matching_title_is_skipped() {
        let blocks = [
            Block::Heading { level: 1, text: Inline::Text("Cover".into()) },
            Block::Paragraph(Inline::Text("body".into())),
        ];
        let chs = vec![ChapterToRender {
            number: 1,
            title: "Cover",
            book_title: "",
            blocks: &blocks,
            footnotes: &[],
        }];
        let body = render(&chs).body;
        // Should contain "## Cover" exactly once, then "body".
        assert_eq!(body.matches("## Cover").count(), 1, "got: {body}");
        assert!(body.contains("body"));
    }

    #[test]
    fn first_heading_not_matching_title_is_kept() {
        let blocks = [
            Block::Heading { level: 1, text: Inline::Text("Different Heading".into()) },
        ];
        let chs = vec![ChapterToRender {
            number: 1,
            title: "Cover",
            book_title: "",
            blocks: &blocks,
            footnotes: &[],
        }];
        let body = render(&chs).body;
        // Both headings appear: "## Cover" (auto) and "## Different Heading" (h1 shifted).
        assert!(body.contains("## Cover"), "got: {body}");
        assert!(body.contains("## Different Heading"), "got: {body}");
    }

    #[test]
    fn skip_heading_preceded_by_empty_paragraph() {
        let blocks = [
            Block::Paragraph(Inline::empty()),  // auxiliary
            Block::Heading { level: 1, text: Inline::Text("Cover".into()) },
            Block::Paragraph(Inline::Text("body".into())),
        ];
        let chs = vec![ChapterToRender {
            number: 1,
            title: "Cover",
            book_title: "",
            blocks: &blocks,
            footnotes: &[],
        }];
        let body = render(&chs).body;
        assert_eq!(body.matches("## Cover").count(), 1, "got: {body}");
        assert!(body.contains("body"));
    }

    #[test]
    fn skip_heading_preceded_by_image() {
        let blocks = [
            Block::Image { src: "x.jpg".into(), alt: "".into(), title: None },
            Block::Heading { level: 1, text: Inline::Text("Intro".into()) },
        ];
        let chs = vec![ChapterToRender {
            number: 1,
            title: "Intro",
            book_title: "",
            blocks: &blocks,
            footnotes: &[],
        }];
        let body = render(&chs).body;
        assert_eq!(body.matches("## Intro").count(), 1, "got: {body}");
        // Image should still appear.
        assert!(body.contains("![](x.jpg)"), "got: {body}");
    }

    #[test]
    fn dont_skip_if_first_real_block_is_content() {
        let blocks = [
            Block::Paragraph(Inline::Text("hello".into())),
            Block::Heading { level: 1, text: Inline::Text("Foo".into()) },
        ];
        let chs = vec![ChapterToRender {
            number: 1,
            title: "Foo",
            book_title: "",
            blocks: &blocks,
            footnotes: &[],
        }];
        let body = render(&chs).body;
        // Heading is too late — chapter content already started. Both headings appear.
        assert_eq!(body.matches("## Foo").count(), 2, "got: {body}");
    }

    #[test]
    fn linebreak_in_heading_becomes_space() {
        let s = render_one(vec![Block::Heading {
            level: 1,
            text: Inline::Concat(vec![
                Inline::Text("Title".into()),
                Inline::LineBreak,
                Inline::Text("subtitle".into()),
            ]),
        }]);
        assert!(s.contains("## Title subtitle\n"), "got: {s}");
        assert!(!s.contains("Title  \n"), "should not have hard break in heading; got: {s}");
    }

    #[test]
    fn linebreak_after_trailing_space_does_not_double() {
        let s = render_one(vec![Block::Heading {
            level: 1,
            text: Inline::Concat(vec![
                Inline::Text("Title ".into()), // text already ends with a space
                Inline::LineBreak,
                Inline::Text("subtitle".into()),
            ]),
        }]);
        assert!(s.contains("## Title subtitle\n"), "got: {s}");
        assert!(!s.contains("Title  subtitle"), "should collapse double space; got: {s}");
    }

    #[test]
    fn skip_heading_with_whitespace_difference() {
        // Source heading rendered as "1  High-Tech" (double space from <br/> as space).
        let blocks = [
            Block::Heading {
                level: 1,
                text: Inline::Concat(vec![
                    Inline::Text("1".into()),
                    Inline::LineBreak,
                    Inline::Text(" High-Tech".into()),
                ]),
            },
        ];
        let chs = vec![ChapterToRender {
            number: 1,
            title: "1 High-Tech",
            book_title: "",
            blocks: &blocks,
            footnotes: &[],
        }];
        let body = render(&chs).body;
        assert_eq!(body.matches("## 1 High-Tech").count(), 1, "got: {body}");
    }

    #[test]
    fn fuzzy_contains_match_skips_short_heading() {
        // Title is "Chapter 1: The Tar Pit"; source heading is just "The Tar Pit".
        let blocks = [
            Block::Heading { level: 1, text: Inline::Text("The Tar Pit".into()) },
        ];
        let chs = vec![ChapterToRender {
            number: 1,
            title: "Chapter 1: The Tar Pit",
            book_title: "",
            blocks: &blocks,
            footnotes: &[],
        }];
        let body = render(&chs).body;
        assert!(body.contains("## Chapter 1: The Tar Pit"));
        assert!(!body.contains("## The Tar Pit\n"), "should be deduped; got: {body}");
    }

    #[test]
    fn drops_running_header_matching_book_title() {
        let blocks = [
            Block::Heading { level: 1, text: Inline::Text("Chapter 1".into()) },
            Block::Heading { level: 1, text: Inline::Text("The Mythical Man-Month".into()) },
            Block::Paragraph(Inline::Text("body".into())),
        ];
        let chs = vec![ChapterToRender {
            number: 1,
            title: "Chapter 1",
            book_title: "The Mythical Man-Month",
            blocks: &blocks,
            footnotes: &[],
        }];
        let body = render(&chs).body;
        assert_eq!(body.matches("## Chapter 1").count(), 1, "got: {body}");
        assert!(!body.contains("## The Mythical Man-Month"), "running header should be dropped; got: {body}");
        assert!(body.contains("body"));
    }

    #[test]
    fn fuzzy_match_requires_minimum_length() {
        // "X" containing "Foo" or vice versa shouldn't match.
        let blocks = [
            Block::Heading { level: 1, text: Inline::Text("X".into()) },
            Block::Paragraph(Inline::Text("body".into())),
        ];
        let chs = vec![ChapterToRender {
            number: 1,
            title: "Foo Bar Baz",
            book_title: "",
            blocks: &blocks,
            footnotes: &[],
        }];
        let body = render(&chs).body;
        // Heading "X" doesn't match "Foo Bar Baz" — kept.
        assert_eq!(body.matches("## X").count(), 1, "got: {body}");
    }
}
