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
                let collapsed = collapse_ws(&t);
                if !collapsed.is_empty() {
                    parts.push(Inline::Text(collapsed));
                }
            }
            Node::Element(_) => {
                if let Some(ce) = ElementRef::wrap(child) {
                    let inner = inline_of(ce);
                    if !inner.is_empty() {
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
}
