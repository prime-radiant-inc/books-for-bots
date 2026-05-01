use crate::block::{Block, Inline};

/// Resolve the title for a single spine document, in priority order:
/// 1. TOC label
/// 2. The HTML <title> element (passed as `html_title`, may be empty)
/// 3. First H1 or H2 in the parsed blocks
/// 4. `Untitled (<filename>)`
pub fn resolve_title(
    toc_label: Option<&str>,
    html_title: Option<&str>,
    blocks: &[Block],
    spine_filename: &str,
) -> String {
    if let Some(t) = toc_label {
        let trimmed = t.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Some(t) = html_title {
        let trimmed = t.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    for b in blocks {
        if let Block::Heading { level, text } = b {
            if *level <= 2 {
                let s = inline_to_plain(text);
                if !s.trim().is_empty() {
                    return s;
                }
            }
        }
    }
    format!("Untitled ({spine_filename})")
}

fn inline_to_plain(i: &Inline) -> String {
    match i {
        Inline::Text(s) => s.clone(),
        Inline::Concat(xs) | Inline::Emphasis(xs) | Inline::Strong(xs) => {
            xs.iter().map(inline_to_plain).collect::<String>()
        }
        Inline::Link { children, .. } => children.iter().map(inline_to_plain).collect::<String>(),
        Inline::Code(s) => s.clone(),
        Inline::Image { alt, .. } => alt.clone(),
        Inline::FootnoteRef(_) | Inline::LineBreak => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{Block, Inline};

    #[test]
    fn toc_label_wins() {
        let t = resolve_title(Some("Intro"), Some("Page"), &[], "x.xhtml");
        assert_eq!(t, "Intro");
    }

    #[test]
    fn html_title_when_no_toc() {
        let t = resolve_title(None, Some("Page"), &[], "x.xhtml");
        assert_eq!(t, "Page");
    }

    #[test]
    fn first_h1_h2_when_no_toc_or_title() {
        let blocks = vec![
            Block::Paragraph(Inline::Text("preface".into())),
            Block::Heading { level: 2, text: Inline::Text("Real Title".into()) },
        ];
        let t = resolve_title(None, None, &blocks, "x.xhtml");
        assert_eq!(t, "Real Title");
    }

    #[test]
    fn fallback_uses_filename() {
        let t = resolve_title(None, None, &[], "ch07.xhtml");
        assert_eq!(t, "Untitled (ch07.xhtml)");
    }

    #[test]
    fn whitespace_only_is_skipped() {
        let t = resolve_title(Some("   "), Some(""), &[], "x.xhtml");
        assert_eq!(t, "Untitled (x.xhtml)");
    }
}
