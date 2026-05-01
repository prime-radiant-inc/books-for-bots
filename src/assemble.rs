use crate::block::{Block, Inline};

/// Resolve the title for a single spine document, in priority order:
/// 1. TOC label
/// 2. First H1 or H2 in the parsed blocks
/// 3. The HTML <title> element (passed as `html_title`, may be empty)
/// 4. `Untitled (<filename>)`
///
/// H1/H2 is preferred over HTML <title> because real-world EPUBs put the
/// book title in the HTML <title> of every spine document, making it a
/// poor source for chapter titles.
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
    if let Some(t) = html_title {
        let trimmed = t.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
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
    fn html_title_when_no_h1_h2() {
        // Falls through to HTML title only if there's no body heading.
        let t = resolve_title(None, Some("Page Title"), &[], "x.xhtml");
        assert_eq!(t, "Page Title");
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
    fn h1_wins_over_html_title() {
        // Real-world: HTML <title> usually contains the book title, not the
        // chapter title. The body's first H1/H2 is more reliable.
        let blocks = vec![
            Block::Heading { level: 1, text: Inline::Text("Real Chapter Title".into()) },
        ];
        let t = resolve_title(None, Some("Book Title"), &blocks, "ch1.xhtml");
        assert_eq!(t, "Real Chapter Title");
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

/// One assembled chapter ready to render.
#[derive(Debug, Clone)]
pub struct Chapter {
    /// 1-based chapter number.
    pub number: usize,
    pub title: String,
    /// Manifest path of the source spine document — used for resolving
    /// internal links from this chapter.
    pub source_path: String,
    pub blocks: Vec<Block>,
}

/// Rewrites IDs and links *within* one chapter's blocks:
/// - `Inline::FootnoteRef("fn5")` → `FootnoteRef("c{n}-fn5")`
/// - `FootnoteDef { id: "fn5", .. }` → `id: "c{n}-fn5"`
/// - `Block::Anchor { id: "foo" }` → `id: "c{n}-foo"`
pub fn namespace_chapter(blocks: &mut Vec<Block>, chapter_n: usize) {
    for b in blocks.iter_mut() {
        namespace_block(b, chapter_n);
    }
}

fn namespace_block(b: &mut Block, n: usize) {
    match b {
        Block::Heading { text, .. } | Block::Paragraph(text) => namespace_inline(text, n),
        Block::BlockQuote(children) => for c in children { namespace_block(c, n); },
        Block::List { items, .. } => for item in items { for c in item { namespace_block(c, n); } },
        Block::Table { header, rows } => {
            for c in header { namespace_inline(c, n); }
            for r in rows { for c in r { namespace_inline(c, n); } }
        }
        Block::FootnoteDef { id, content } => {
            *id = format!("c{n}-{id}");
            for c in content { namespace_block(c, n); }
        }
        Block::Anchor { id } => *id = format!("c{n}-{id}"),
        Block::CodeBlock { .. } | Block::Image { .. } | Block::HorizontalRule => {}
    }
}

fn namespace_inline(i: &mut Inline, n: usize) {
    match i {
        Inline::FootnoteRef(id) => *id = format!("c{n}-{id}"),
        Inline::Concat(xs) | Inline::Emphasis(xs) | Inline::Strong(xs) => {
            for x in xs { namespace_inline(x, n); }
        }
        Inline::Link { children, .. } => for c in children { namespace_inline(c, n); },
        _ => {}
    }
}

#[cfg(test)]
mod ns_tests {
    use super::*;

    #[test]
    fn footnote_ref_namespaced() {
        let mut blocks = vec![Block::Paragraph(Inline::Concat(vec![
            Inline::Text("see ".into()),
            Inline::FootnoteRef("fn1".into()),
        ]))];
        namespace_chapter(&mut blocks, 3);
        let Block::Paragraph(Inline::Concat(parts)) = &blocks[0] else { panic!() };
        assert!(matches!(&parts[1], Inline::FootnoteRef(s) if s == "c3-fn1"));
    }

    #[test]
    fn footnote_def_namespaced() {
        let mut blocks = vec![Block::FootnoteDef {
            id: "fn1".into(),
            content: vec![Block::Paragraph(Inline::Text("note".into()))],
        }];
        namespace_chapter(&mut blocks, 7);
        let Block::FootnoteDef { id, .. } = &blocks[0] else { panic!() };
        assert_eq!(id, "c7-fn1");
    }
}

use std::collections::BTreeMap;

/// Rewrite cross-document hrefs to in-file anchors using heading auto-slugs.
///
/// - Within-doc fragments (`#foo`) → unchanged. Will resolve only if the
///   target was a heading and `foo` matches its auto-slug.
/// - Cross-doc with fragment (`b.xhtml#foo`) → `#<target chapter slug>`.
///   The deeper fragment is dropped; agents seek to the target chapter via
///   the YAML offsets and read for the named anchor's surrounding text.
/// - Cross-doc no fragment (`b.xhtml`) → `#<target chapter slug>`.
/// - External URLs (http, https, mailto) → unchanged.
pub fn rewrite_internal_links(chapters: &mut [Chapter]) {
    let path_to_slug = build_chapter_slug_map(chapters);
    for chap in chapters.iter_mut() {
        let owning_dir = std::path::Path::new(&chap.source_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        for b in chap.blocks.iter_mut() {
            rewrite_links_in_block(b, &owning_dir, &path_to_slug);
        }
    }
}

fn rewrite_links_in_block(
    b: &mut Block,
    owning_dir: &str,
    map: &BTreeMap<String, String>,
) {
    match b {
        Block::Heading { text, .. } | Block::Paragraph(text) => rewrite_links_in_inline(text, owning_dir, map),
        Block::BlockQuote(c) => for x in c { rewrite_links_in_block(x, owning_dir, map); },
        Block::List { items, .. } => for item in items { for x in item { rewrite_links_in_block(x, owning_dir, map); } },
        Block::Table { header, rows } => {
            for c in header { rewrite_links_in_inline(c, owning_dir, map); }
            for r in rows { for c in r { rewrite_links_in_inline(c, owning_dir, map); } }
        }
        Block::FootnoteDef { content, .. } => for c in content { rewrite_links_in_block(c, owning_dir, map); },
        _ => {}
    }
}

fn rewrite_links_in_inline(
    i: &mut Inline,
    owning_dir: &str,
    map: &BTreeMap<String, String>,
) {
    match i {
        Inline::Link { href, children } => {
            *href = rewrite_one_href(href, owning_dir, map);
            for c in children { rewrite_links_in_inline(c, owning_dir, map); }
        }
        Inline::Concat(xs) | Inline::Emphasis(xs) | Inline::Strong(xs) => {
            for x in xs { rewrite_links_in_inline(x, owning_dir, map); }
        }
        _ => {}
    }
}

fn rewrite_one_href(
    href: &str,
    owning_dir: &str,
    map: &BTreeMap<String, String>,
) -> String {
    if href.is_empty() { return href.to_string(); }
    if href.starts_with("http://") || href.starts_with("https://") || href.starts_with("mailto:") {
        return href.to_string();
    }
    if href.starts_with('#') {
        return href.to_string();
    }
    let path_part = match href.split_once('#') {
        Some((p, _f)) => p.to_string(),
        None => href.to_string(),
    };
    let resolved = if owning_dir.is_empty() {
        path_part.clone()
    } else {
        format!("{owning_dir}/{path_part}")
    };
    let normalized = normalize_path(&resolved);
    if let Some(target_slug) = map.get(&normalized) {
        format!("#{target_slug}")
    } else {
        href.to_string()
    }
}

/// GFM-style heading auto-slug: lowercase, drop non-alphanumeric/space/hyphen/underscore,
/// then replace spaces with hyphens. Collisions resolved by appending -1, -2, etc.
fn auto_slug(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

/// Build path → unique-slug map. Collisions on slug get -1, -2 suffixes.
fn build_chapter_slug_map(chapters: &[Chapter]) -> BTreeMap<String, String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    for ch in chapters {
        let base = auto_slug(&ch.title);
        let cnt = counts.entry(base.clone()).or_insert(0);
        let slug = if *cnt == 0 { base.clone() } else { format!("{base}-{cnt}") };
        *cnt += 1;
        out.insert(ch.source_path.clone(), slug);
    }
    out
}

fn normalize_path(p: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => { parts.pop(); }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod link_tests {
    use super::*;

    fn p_link(href: &str) -> Block {
        Block::Paragraph(Inline::Link {
            href: href.to_string(),
            children: vec![Inline::Text("x".into())],
        })
    }

    #[test]
    fn intra_doc_fragment_unchanged() {
        let mut chap = Chapter { number: 4, title: "T".into(), source_path: "OEBPS/c.xhtml".into(), blocks: vec![p_link("#sec1")] };
        rewrite_internal_links(std::slice::from_mut(&mut chap));
        let Block::Paragraph(Inline::Link { href, .. }) = &chap.blocks[0] else { panic!() };
        assert_eq!(href, "#sec1");
    }

    #[test]
    fn cross_doc_with_fragment_uses_target_slug() {
        let a = Chapter { number: 1, title: "First".into(), source_path: "OEBPS/a.xhtml".into(), blocks: vec![p_link("b.xhtml#foo")] };
        let b = Chapter { number: 2, title: "Second Chapter".into(), source_path: "OEBPS/b.xhtml".into(), blocks: vec![] };
        let mut chs = vec![a, b];
        rewrite_internal_links(&mut chs);
        let Block::Paragraph(Inline::Link { href, .. }) = &chs[0].blocks[0] else { panic!() };
        assert_eq!(href, "#second-chapter");
    }

    #[test]
    fn cross_doc_no_fragment_uses_target_slug() {
        let a = Chapter { number: 1, title: "A".into(), source_path: "OEBPS/a.xhtml".into(), blocks: vec![p_link("b.xhtml")] };
        let b = Chapter { number: 2, title: "Chapter Five".into(), source_path: "OEBPS/b.xhtml".into(), blocks: vec![] };
        let mut chs = vec![a, b];
        rewrite_internal_links(&mut chs);
        let Block::Paragraph(Inline::Link { href, .. }) = &chs[0].blocks[0] else { panic!() };
        assert_eq!(href, "#chapter-five");
    }

    #[test]
    fn external_unchanged() {
        let mut chap = Chapter { number: 1, title: "T".into(), source_path: "x".into(), blocks: vec![p_link("https://example.com")] };
        rewrite_internal_links(std::slice::from_mut(&mut chap));
        let Block::Paragraph(Inline::Link { href, .. }) = &chap.blocks[0] else { panic!() };
        assert_eq!(href, "https://example.com");
    }

    #[test]
    fn duplicate_titles_get_collision_suffix() {
        let a = Chapter { number: 1, title: "Chapter".into(), source_path: "a".into(), blocks: vec![p_link("b#x")] };
        let b = Chapter { number: 2, title: "Chapter".into(), source_path: "b".into(), blocks: vec![] };
        let mut chs = vec![a, b];
        rewrite_internal_links(&mut chs);
        let Block::Paragraph(Inline::Link { href, .. }) = &chs[0].blocks[0] else { panic!() };
        // First "Chapter" → "chapter", second → "chapter-1".
        assert_eq!(href, "#chapter-1");
    }

    #[test]
    fn slug_handles_punctuation_and_unicode() {
        // "Step 1. Understand the Customer" → "step-1-understand-the-customer"
        assert_eq!(auto_slug("Step 1. Understand the Customer"), "step-1-understand-the-customer");
        // Double spaces, dashes preserved, em-dash dropped.
        assert_eq!(auto_slug("Hello — World"), "hello-world");
    }
}
