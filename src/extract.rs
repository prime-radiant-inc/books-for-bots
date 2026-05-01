use crate::block::{Block, Inline};
use scraper::{Html, Node, ElementRef};

pub fn parse(html: &str) -> Vec<Block> {
    let doc = Html::parse_document(html);
    let body = doc
        .select(&scraper::Selector::parse("body").unwrap())
        .next()
        .unwrap_or_else(|| doc.root_element());
    extract_blocks(body)
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

fn extract_into(el: ElementRef, out: &mut Vec<Block>) {
    let name = el.value().name();
    match name {
        "p" => {
            let inl = inline_of(el);
            if !inl.is_empty() {
                out.push(Block::Paragraph(inl));
            }
        }
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level: u8 = name.as_bytes()[1] - b'0';
            out.push(Block::Heading { level, text: inline_of(el) });
        }
        "div" | "span" | "section" | "article" | "header" | "footer" | "main" | "nav" => {
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

fn inline_of(el: ElementRef) -> Inline {
    let mut parts = Vec::new();
    for child in el.children() {
        match child.value() {
            Node::Text(t) => {
                let collapsed = collapse_ws_inline(&t, !parts.is_empty());
                if !collapsed.is_empty() {
                    parts.push(Inline::Text(collapsed));
                }
            }
            Node::Element(_) => {
                if let Some(ce) = ElementRef::wrap(child) {
                    let tag = ce.value().name();
                    let inner = match tag {
                        "em" | "i" => {
                            let kids = match inline_of(ce) {
                                Inline::Concat(v) => v,
                                other => vec![other],
                            };
                            Inline::Emphasis(kids)
                        }
                        "strong" | "b" => {
                            let kids = match inline_of(ce) {
                                Inline::Concat(v) => v,
                                other => vec![other],
                            };
                            Inline::Strong(kids)
                        }
                        "code" => Inline::Code(plain_text(ce)),
                        "br" => Inline::LineBreak,
                        "a" => {
                            let href = ce.value().attr("href").unwrap_or("").to_string();
                            let kids = match inline_of(ce) {
                                Inline::Concat(v) => v,
                                other => vec![other],
                            };
                            Inline::Link { href, children: kids }
                        }
                        "img" => {
                            let src = ce.value().attr("src").unwrap_or("").to_string();
                            let alt = ce.value().attr("alt").unwrap_or("").to_string();
                            let title = ce.value().attr("title").map(str::to_string);
                            Inline::Image { src, alt, title }
                        }
                        _ => inline_of(ce), // transparent
                    };
                    if !inner.is_empty() || matches!(inner, Inline::LineBreak | Inline::Image{..} | Inline::Code(_)) {
                        parts.push(inner);
                    }
                }
            }
            _ => {}
        }
    }
    match parts.len() {
        0 => Inline::empty(),
        1 => parts.into_iter().next().unwrap(),
        _ => Inline::Concat(parts),
    }
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
}
