use crate::block::{Block, Inline};
use scraper::{Html, Node, ElementRef};

pub fn parse(html: &str) -> Vec<Block> {
    let cleaned = strip_head(html);
    let doc = Html::parse_document(&cleaned);
    let body = doc
        .select(&scraper::Selector::parse("body").unwrap())
        .next()
        .unwrap_or_else(|| doc.root_element());
    extract_blocks(body)
}

fn strip_head(html: &str) -> String {
    // XHTML lets <script>/<style> be self-closing; HTML5's parser doesn't,
    // and treats <script /> as opening a raw-text element that consumes the
    // rest of the document. Drop the entire <head>…</head> to avoid this.
    // The load module already extracted what it needs (title, etc.) from
    // metadata; we don't render anything from the head here.
    if let Some(start) = html.find("<head") {
        if let Some(open_end) = html[start..].find('>') {
            let after_open = start + open_end + 1;
            if let Some(close_off) = html[after_open..].find("</head>") {
                let after_close = after_open + close_off + "</head>".len();
                let mut out = String::with_capacity(html.len());
                out.push_str(&html[..start]);
                out.push_str(&html[after_close..]);
                return out;
            }
        }
    }
    html.to_string()
}

fn extract_blocks(parent: ElementRef) -> Vec<Block> {
    let mut out = Vec::new();
    for child in parent.children() {
        if let Some(el) = ElementRef::wrap(child) {
            extract_into(el, &mut out);
        }
    }
    out
}

fn is_block_tag(name: &str) -> bool {
    matches!(name, "p" | "ul" | "ol" | "blockquote" | "table" | "pre" |
                  "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "hr")
}

fn extract_li(li: ElementRef) -> Vec<Block> {
    // If <li> contains any block-level descendants, render mixed: each text-or-inline
    // run becomes a Paragraph, each block-level element recurses through extract_into.
    let has_block_child = li
        .children()
        .filter_map(ElementRef::wrap)
        .any(|c| is_block_tag(c.value().name()));
    if !has_block_child {
        // Pure inline: one paragraph for the whole <li>.
        let inl = inline_of(li, false);
        if inl.is_empty() {
            return vec![Block::Paragraph(Inline::empty())];
        }
        return vec![Block::Paragraph(inl)];
    }
    let mut out = Vec::new();
    let mut inline_buf: Vec<Inline> = Vec::new();
    fn flush(buf: &mut Vec<Inline>, out: &mut Vec<Block>) {
        if !buf.is_empty() {
            let inl = if buf.len() == 1 { buf.remove(0) } else { Inline::Concat(std::mem::take(buf)) };
            if !inl.is_empty() {
                out.push(Block::Paragraph(inl));
            }
            buf.clear();
        }
    }
    for child in li.children() {
        match child.value() {
            Node::Text(t) => {
                let s = collapse_ws(&t);
                if !s.is_empty() { inline_buf.push(Inline::Text(s)); }
            }
            Node::Element(_) => {
                if let Some(ce) = ElementRef::wrap(child) {
                    if is_block_tag(ce.value().name()) {
                        flush(&mut inline_buf, &mut out);
                        extract_into(ce, &mut out);
                    } else {
                        let inner_inline = inline_of_single(ce);
                        if !inner_inline.is_empty() {
                            inline_buf.push(inner_inline);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    flush(&mut inline_buf, &mut out);
    if out.is_empty() {
        out.push(Block::Paragraph(Inline::empty()));
    }
    out
}

/// Render one element as Inline (handles em/strong/code/a/br/img and falls
/// back to inline_of for other inline-only tags).
fn inline_of_single(el: ElementRef) -> Inline {
    let tag = el.value().name();
    match tag {
        "em" | "i" => {
            let kids = match inline_of(el, false) {
                Inline::Concat(v) => v,
                other => vec![other],
            };
            Inline::Emphasis(kids)
        }
        "strong" | "b" => {
            let kids = match inline_of(el, false) {
                Inline::Concat(v) => v,
                other => vec![other],
            };
            Inline::Strong(kids)
        }
        "code" => Inline::Code(plain_text(el)),
        "br" => Inline::LineBreak,
        "a" => {
            let href = el.value().attr("href").unwrap_or("").to_string();
            let is_noteref = el
                .value()
                .attr("epub:type")
                .map(|t| t.contains("noteref"))
                .unwrap_or(false)
                || (href.starts_with('#') && parent_is_sup(el));
            if is_noteref {
                let id = href.trim_start_matches('#').to_string();
                Inline::FootnoteRef(id)
            } else if href.is_empty() {
                inline_of(el, false)
            } else {
                let kids = match inline_of(el, false) {
                    Inline::Concat(v) => v,
                    other => vec![other],
                };
                Inline::Link { href, children: kids }
            }
        }
        "img" => Inline::Image {
            src: el.value().attr("src").unwrap_or("").to_string(),
            alt: el.value().attr("alt").unwrap_or("").to_string(),
            title: el.value().attr("title").map(str::to_string),
        },
        _ => inline_of(el, false),
    }
}

fn extract_into(el: ElementRef, out: &mut Vec<Block>) {
    let name = el.value().name();

    match name {
        "p" => {
            let inl = inline_of(el, false);
            if !inl.is_empty() {
                out.push(Block::Paragraph(inl));
            }
        }
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level: u8 = name.as_bytes()[1] - b'0';
            out.push(Block::Heading { level, text: inline_of(el, false) });
        }
        "ul" | "ol" => {
            let ordered = name == "ol";
            let items: Vec<Vec<Block>> = el
                .children()
                .filter_map(ElementRef::wrap)
                .filter(|c| c.value().name() == "li")
                .map(extract_li)
                .collect();
            out.push(Block::List { ordered, items });
        }
        "blockquote" => {
            let inner = extract_blocks(el);
            out.push(Block::BlockQuote(inner));
        }
        "hr" => out.push(Block::HorizontalRule),
        "table" => {
            // Find header: prefer rows that contain <th>; first such row is header.
            let mut header: Vec<Inline> = Vec::new();
            let mut rows: Vec<Vec<Inline>> = Vec::new();
            let trs: Vec<ElementRef> = el
                .descendants()
                .filter_map(ElementRef::wrap)
                .filter(|n| n.value().name() == "tr")
                .collect();
            let mut header_set = false;
            for tr in trs {
                let cells: Vec<Inline> = tr
                    .children()
                    .filter_map(ElementRef::wrap)
                    .filter(|c| matches!(c.value().name(), "th" | "td"))
                    .map(|el| inline_of(el, false))
                    .collect();
                let is_header_row = !header_set
                    && tr.children().filter_map(ElementRef::wrap).any(|c| c.value().name() == "th");
                if is_header_row {
                    header = cells;
                    header_set = true;
                } else {
                    rows.push(cells);
                }
            }
            out.push(Block::Table { header, rows });
        }
        "pre" => {
            let code_el = el
                .children()
                .filter_map(ElementRef::wrap)
                .find(|c| c.value().name() == "code");
            let lang = code_el
                .as_ref()
                .and_then(|c| c.value().attr("class"))
                .and_then(|cls| {
                    cls.split_whitespace()
                        .find_map(|t| t.strip_prefix("language-").map(str::to_string))
                });
            let code = match code_el {
                Some(c) => plain_text(c),
                None => plain_text(el),
            };
            out.push(Block::CodeBlock { lang, code });
        }
        "aside" if is_footnote_container(el) => {
            let id = el.value().attr("id").unwrap_or("").to_string();
            let content = extract_blocks(el);
            out.push(Block::FootnoteDef { id, content });
        }
        "div" if is_footnote_container(el) => {
            let id = el.value().attr("id").unwrap_or("").to_string();
            let content = extract_blocks(el);
            out.push(Block::FootnoteDef { id, content });
        }
        "div" | "aside" | "span" | "section" | "article" | "header" | "footer" | "main" | "nav" => {
            // Transparent: recurse into children.
            for child in el.children() {
                if let Some(ce) = ElementRef::wrap(child) {
                    extract_into(ce, out);
                }
            }
        }
        _ => {
            // Unknown tag: transparent fallback.
            for child in el.children() {
                if let Some(ce) = ElementRef::wrap(child) {
                    extract_into(ce, out);
                }
            }
        }
    }
}

fn dedupe_consecutive_breaks(i: Inline) -> Inline {
    match i {
        Inline::Concat(xs) => {
            let mut out: Vec<Inline> = Vec::with_capacity(xs.len());
            for x in xs {
                let x = dedupe_consecutive_breaks(x);
                if matches!(x, Inline::LineBreak) {
                    if matches!(out.last(), Some(Inline::LineBreak)) {
                        continue;
                    }
                }
                out.push(x);
            }
            match out.len() {
                0 => Inline::empty(),
                1 => out.into_iter().next().unwrap(),
                _ => Inline::Concat(out),
            }
        }
        Inline::Emphasis(xs) => Inline::Emphasis(xs.into_iter().map(dedupe_consecutive_breaks).collect()),
        Inline::Strong(xs) => Inline::Strong(xs.into_iter().map(dedupe_consecutive_breaks).collect()),
        Inline::Link { href, children } => Inline::Link {
            href,
            children: children.into_iter().map(dedupe_consecutive_breaks).collect(),
        },
        other => other,
    }
}

fn inline_of(el: ElementRef, has_outer_prior: bool) -> Inline {
    let mut parts = Vec::new();
    for child in el.children() {
        match child.value() {
            Node::Text(t) => {
                let has_prior = has_outer_prior || !parts.is_empty();
                let collapsed = collapse_ws_inline(&t, has_prior);
                if !collapsed.is_empty() {
                    parts.push(Inline::Text(collapsed));
                }
            }
            Node::Element(_) => {
                if let Some(ce) = ElementRef::wrap(child) {
                    let inner_outer_prior = has_outer_prior || !parts.is_empty();
                    let tag = ce.value().name();
                    let inner = match tag {
                        "em" | "i" => {
                            let kids = match inline_of(ce, false) {
                                Inline::Concat(v) => v,
                                other => vec![other],
                            };
                            Inline::Emphasis(kids)
                        }
                        "strong" | "b" => {
                            let kids = match inline_of(ce, false) {
                                Inline::Concat(v) => v,
                                other => vec![other],
                            };
                            Inline::Strong(kids)
                        }
                        "code" => Inline::Code(plain_text(ce)),
                        "br" => Inline::LineBreak,
                        "a" => {
                            let href = ce.value().attr("href").unwrap_or("").to_string();
                            let is_noteref = ce
                                .value()
                                .attr("epub:type")
                                .map(|t| t.contains("noteref"))
                                .unwrap_or(false)
                                || (href.starts_with('#') && parent_is_sup(ce));
                            if is_noteref {
                                let id = href.trim_start_matches('#').to_string();
                                Inline::FootnoteRef(id)
                            } else if href.is_empty() {
                                // Anchor-only <a id="..."> — return its content inline,
                                // not as a link. Avoids broken "[text]()" output.
                                inline_of(ce, inner_outer_prior)
                            } else {
                                let kids = match inline_of(ce, false) {
                                    Inline::Concat(v) => v,
                                    other => vec![other],
                                };
                                Inline::Link { href, children: kids }
                            }
                        }
                        "img" => {
                            let src = ce.value().attr("src").unwrap_or("").to_string();
                            let alt = ce.value().attr("alt").unwrap_or("").to_string();
                            let title = ce.value().attr("title").map(str::to_string);
                            Inline::Image { src, alt, title }
                        }
                        _ => inline_of(ce, inner_outer_prior), // transparent
                    };
                    if !inner.is_empty() || matches!(inner, Inline::LineBreak | Inline::Image{..} | Inline::Code(_)) {
                        parts.push(inner);
                    }
                }
            }
            _ => {}
        }
    }
    let result = match parts.len() {
        0 => Inline::empty(),
        1 => parts.into_iter().next().unwrap(),
        _ => Inline::Concat(parts),
    };
    dedupe_consecutive_breaks(result)
}

/// Collapse whitespace for inline text nodes.
/// Preserves a single leading space when `has_prior` is true and the text
/// starts with whitespace (inter-element space like " and ").
/// Preserves a single trailing space when the text ends with whitespace.
fn collapse_ws_inline(s: &str, has_prior: bool) -> String {
    let leading = has_prior && s.starts_with(|c: char| c.is_whitespace());
    let trailing = s.ends_with(|c: char| c.is_whitespace());
    let mut core = collapse_ws(s);
    if trailing && !core.ends_with(' ') && !core.is_empty() {
        core.push(' ');
    }
    if leading && !core.starts_with(' ') && !core.is_empty() {
        core.insert(0, ' ');
    }
    if leading && core.is_empty() {
        core.push(' ');
    }
    core
}

fn plain_text(el: ElementRef) -> String {
    let mut s = String::new();
    for child in el.descendants() {
        if let Node::Text(t) = child.value() {
            s.push_str(t);
        }
    }
    s
}

fn parent_is_sup(el: ElementRef) -> bool {
    el.parent()
        .and_then(ElementRef::wrap)
        .map(|p| p.value().name() == "sup")
        .unwrap_or(false)
}

fn is_footnote_container(el: ElementRef) -> bool {
    let v = el.value();
    let is_aside_footnote = v.name() == "aside"
        && v.attr("epub:type").map(|t| t.contains("footnote")).unwrap_or(false);
    let is_div_footnote = v.name() == "div"
        && v.attr("class").map(|c| c.split_whitespace().any(|t| t == "footnote")).unwrap_or(false);
    is_aside_footnote || is_div_footnote
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_space && !out.is_empty() {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraph() {
        let b = parse("<html><body><p>Hello world.</p></body></html>");
        assert_eq!(b, vec![Block::Paragraph(Inline::Text("Hello world.".to_string()))]);
    }

    #[test]
    fn heading_level() {
        let b = parse("<html><body><h3>Section</h3></body></html>");
        assert_eq!(
            b,
            vec![Block::Heading { level: 3, text: Inline::Text("Section".to_string()) }]
        );
    }

    #[test]
    fn div_is_transparent() {
        let b = parse("<html><body><div><p>Inner</p></div></body></html>");
        assert_eq!(b, vec![Block::Paragraph(Inline::Text("Inner".to_string()))]);
    }

    #[test]
    fn empty_p_is_dropped() {
        let b = parse("<html><body><p></p><p>  </p></body></html>");
        assert_eq!(b, vec![]);
    }

    #[test]
    fn whitespace_collapsed() {
        let b = parse("<html><body><p>foo   bar\n  baz</p></body></html>");
        assert_eq!(b, vec![Block::Paragraph(Inline::Text("foo bar baz".to_string()))]);
    }

    #[test]
    fn emphasis_and_strong() {
        let b = parse("<html><body><p><em>x</em> and <strong>y</strong></p></body></html>");
        assert_eq!(b, vec![Block::Paragraph(Inline::Concat(vec![
            Inline::Emphasis(vec![Inline::Text("x".into())]),
            Inline::Text(" and ".into()),
            Inline::Strong(vec![Inline::Text("y".into())]),
        ]))]);
    }

    #[test]
    fn inline_code() {
        let b = parse("<html><body><p>use <code>main()</code></p></body></html>");
        let Block::Paragraph(Inline::Concat(parts)) = &b[0] else { panic!() };
        assert!(matches!(&parts[1], Inline::Code(s) if s == "main()"));
    }

    #[test]
    fn link() {
        let b = parse(r#"<html><body><p><a href="x.html">go</a></p></body></html>"#);
        let Block::Paragraph(Inline::Link { href, children }) = &b[0] else { panic!() };
        assert_eq!(href, "x.html");
        assert_eq!(children, &vec![Inline::Text("go".into())]);
    }

    #[test]
    fn line_break() {
        let b = parse("<html><body><p>a<br/>b</p></body></html>");
        let Block::Paragraph(Inline::Concat(parts)) = &b[0] else { panic!() };
        assert!(matches!(parts[1], Inline::LineBreak));
    }

    #[test]
    fn unordered_list() {
        let b = parse("<html><body><ul><li>a</li><li>b</li></ul></body></html>");
        assert_eq!(b, vec![Block::List {
            ordered: false,
            items: vec![
                vec![Block::Paragraph(Inline::Text("a".into()))],
                vec![Block::Paragraph(Inline::Text("b".into()))],
            ],
        }]);
    }

    #[test]
    fn ordered_list() {
        let b = parse("<html><body><ol><li>x</li></ol></body></html>");
        assert!(matches!(b[0], Block::List { ordered: true, .. }));
    }

    #[test]
    fn nested_list() {
        let b = parse("<html><body><ul><li>a<ul><li>b</li></ul></li></ul></body></html>");
        let Block::List { items, .. } = &b[0] else { panic!() };
        assert!(matches!(items[0][1], Block::List { .. }));
    }

    #[test]
    fn blockquote() {
        let b = parse("<html><body><blockquote><p>q</p></blockquote></body></html>");
        assert_eq!(b, vec![Block::BlockQuote(vec![Block::Paragraph(Inline::Text("q".into()))])]);
    }

    #[test]
    fn horizontal_rule() {
        let b = parse("<html><body><hr/></body></html>");
        assert_eq!(b, vec![Block::HorizontalRule]);
    }

    #[test]
    fn pre_code_block_with_language() {
        let b = parse(
            r#"<html><body><pre><code class="language-rust">fn main() {}</code></pre></body></html>"#
        );
        assert_eq!(b, vec![Block::CodeBlock {
            lang: Some("rust".into()),
            code: "fn main() {}".into(),
        }]);
    }

    #[test]
    fn pre_no_code() {
        let b = parse("<html><body><pre>raw text</pre></body></html>");
        assert_eq!(b, vec![Block::CodeBlock { lang: None, code: "raw text".into() }]);
    }

    #[test]
    fn simple_table() {
        let html = r#"<html><body><table>
            <thead><tr><th>Name</th><th>Age</th></tr></thead>
            <tbody>
              <tr><td>Alice</td><td>30</td></tr>
              <tr><td>Bob</td><td>25</td></tr>
            </tbody></table></body></html>"#;
        let b = parse(html);
        assert_eq!(b, vec![Block::Table {
            header: vec![Inline::Text("Name".into()), Inline::Text("Age".into())],
            rows: vec![
                vec![Inline::Text("Alice".into()), Inline::Text("30".into())],
                vec![Inline::Text("Bob".into()), Inline::Text("25".into())],
            ],
        }]);
    }

    #[test]
    fn table_no_thead_uses_first_row_as_header() {
        let html = r#"<html><body><table>
            <tr><th>A</th><th>B</th></tr>
            <tr><td>1</td><td>2</td></tr>
        </table></body></html>"#;
        let b = parse(html);
        let Block::Table { header, rows } = &b[0] else { panic!() };
        assert_eq!(header.len(), 2);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn noteref_with_epub_type() {
        let html = r##"<html><body><p>See<a epub:type="noteref" href="#fn1">1</a>.</p></body></html>"##;
        let b = parse(html);
        let Block::Paragraph(Inline::Concat(parts)) = &b[0] else { panic!() };
        assert!(parts.iter().any(|i| matches!(i, Inline::FootnoteRef(s) if s == "fn1")));
    }

    #[test]
    fn sup_anchor_is_noteref() {
        let html = r##"<html><body><p>x<sup><a href="#fn2">2</a></sup>.</p></body></html>"##;
        let b = parse(html);
        let Block::Paragraph(Inline::Concat(parts)) = &b[0] else { panic!() };
        assert!(parts.iter().any(|i| matches!(i, Inline::FootnoteRef(s) if s == "fn2")));
    }

    #[test]
    fn footnote_def_aside() {
        let html = r##"<html><body><aside epub:type="footnote" id="fn1"><p>Note one.</p></aside></body></html>"##;
        let b = parse(html);
        assert_eq!(b.len(), 1);
        let Block::FootnoteDef { id, content } = &b[0] else { panic!() };
        assert_eq!(id, "fn1");
        assert_eq!(content, &vec![Block::Paragraph(Inline::Text("Note one.".into()))]);
    }

    #[test]
    fn whitespace_preserved_across_inline_elements() {
        // Real-world XHTML pattern from Kobo-style epubs: <em>did</em><span> warn you</span>
        // The space at the start of the span's text must be preserved.
        let html = r#"<html><body><p>they <em>did</em><span> warn you</span>.</p></body></html>"#;
        let b = parse(html);
        let Block::Paragraph(Inline::Concat(parts)) = &b[0] else { panic!("got: {b:?}") };
        // Should be: "they ", Emphasis("did"), " warn you", "."
        let mut concatenated = String::new();
        for p in parts {
            match p {
                Inline::Text(s) => concatenated.push_str(s),
                Inline::Emphasis(xs) => {
                    concatenated.push('*');
                    for x in xs {
                        if let Inline::Text(s) = x { concatenated.push_str(s); }
                    }
                    concatenated.push('*');
                }
                _ => {}
            }
        }
        assert_eq!(concatenated, "they *did* warn you.", "got: {concatenated:?}");
    }

    #[test]
    fn anchor_only_a_tag_no_link() {
        // <a id="..." > with no href is just an anchor target — don't render
        // as a markdown link with empty href.
        let html = r#"<html><body><p>before<a id="foo">.</a>after</p></body></html>"#;
        let b = parse(html);
        let Block::Paragraph(Inline::Concat(parts)) = &b[0] else { panic!("got: {b:?}") };
        // Should NOT contain Inline::Link with empty href.
        for p in parts {
            if let Inline::Link { href, .. } = p {
                panic!("unexpected Link with href={href:?}");
            }
        }
    }

    #[test]
    fn consecutive_brs_collapse_to_one_linebreak() {
        let html = r#"<html><body><p>a<br/><br/><br/>b</p></body></html>"#;
        let b = parse(html);
        let Block::Paragraph(Inline::Concat(parts)) = &b[0] else { panic!("got: {b:?}") };
        // a, LineBreak, b — only one LineBreak between a and b.
        let lb_count = parts.iter().filter(|i| matches!(i, Inline::LineBreak)).count();
        assert_eq!(lb_count, 1, "expected 1 LineBreak; got: {parts:?}");
    }

    #[test]
    fn xhtml_self_closing_script_does_not_eat_body() {
        let html = r#"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
  <title>Chapter</title>
  <script type="text/javascript" src="kobo.js" />
  <style type="text/css" id="x">.x{}</style>
</head>
<body><p>Body content here.</p></body></html>"#;
        let b = parse(html);
        assert_eq!(b, vec![Block::Paragraph(Inline::Text("Body content here.".into()))]);
    }
}
