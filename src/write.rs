use crate::assemble::{namespace_chapter, resolve_title, rewrite_internal_links, Chapter};
use crate::block::Block;
use crate::cli::Args;
use crate::error::Error;
use crate::frontmatter::{self, FrontmatterChapter};
use crate::images::resolve_basenames;
use crate::load::{self, Book};
use crate::render::{self, ChapterToRender};
use crate::Result;
use std::collections::BTreeMap;
use std::fs;

pub fn convert(args: &Args) -> Result<()> {
    let book = load::open(&args.input)?;

    // 1. Determine output dir.
    let slug = crate::slug::from_metadata(&book.metadata.title, &book.metadata.authors)
        .unwrap_or_else(|| crate::slug::from_filename(&args.input));
    let book_dir = args.output_dir.join(&slug);
    if book_dir.exists() {
        if !args.force {
            return Err(Error::OutputExists(book_dir));
        }
        fs::remove_dir_all(&book_dir).map_err(|source| Error::Io { source, path: book_dir.clone() })?;
    }
    fs::create_dir_all(&book_dir).map_err(|source| Error::Io { source, path: book_dir.clone() })?;

    // 2. Extract each spine doc into Blocks; resolve title; collect footnotes.
    let mut chapters: Vec<Chapter> = Vec::with_capacity(book.spine.len());
    for (i, doc) in book.spine.iter().enumerate() {
        let mut blocks = crate::extract::parse(&doc.html);
        let html_title = parse_html_title(&doc.html);
        let title = resolve_title(
            doc.toc_title.as_deref(),
            html_title.as_deref(),
            &blocks,
            &doc.manifest_path,
        );
        let n = i + 1;
        namespace_chapter(&mut blocks, n);
        chapters.push(Chapter { number: n, title, source_path: doc.manifest_path.clone(), blocks });
    }
    rewrite_internal_links(&mut chapters);

    // 3. Resolve image basenames, rewrite image src refs.
    //    Hard-errors if any chapter references an image not in the manifest.
    let basenames = resolve_basenames(book.images.keys().cloned());
    rewrite_image_srcs(&mut chapters, &basenames, &book)?;

    // 4. Split out footnote defs from each chapter's block stream.
    let mut chapter_renderables: Vec<(String, Vec<Block>, Vec<Block>)> = Vec::new();
    for ch in &chapters {
        let (body, footnotes) = split_footnotes(&ch.blocks);
        chapter_renderables.push((ch.title.clone(), body, footnotes));
    }

    // 5. Render body.
    let to_render: Vec<ChapterToRender> = chapters
        .iter()
        .zip(chapter_renderables.iter())
        .map(|(ch, (_t, body, fns))| ChapterToRender {
            number: ch.number,
            title: &ch.title,
            blocks: body,
            footnotes: fns,
        })
        .collect();
    let body_result = render::render(&to_render);

    // 6. Build frontmatter using preliminary (body-relative) offsets to know its size.
    let preliminary: Vec<FrontmatterChapter> = chapters
        .iter()
        .zip(body_result.chapter_offsets.iter())
        .map(|(ch, off)| FrontmatterChapter { title: &ch.title, offset: off.clone() })
        .collect();
    let preliminary_fm = frontmatter::render(&book.metadata, &preliminary)?;
    let fm_bytes = preliminary_fm.len() as u64;
    let fm_lines = preliminary_fm.bytes().filter(|b| *b == b'\n').count() as u64;

    // 7. Add frontmatter offsets to body offsets.
    let final_offsets: Vec<FrontmatterChapter> = chapters
        .iter()
        .zip(body_result.chapter_offsets.iter())
        .map(|(ch, off)| FrontmatterChapter {
            title: &ch.title,
            offset: render::ChapterOffset {
                byte: off.byte + fm_bytes,
                line: off.line + fm_lines,
            },
        })
        .collect();
    let final_fm = frontmatter::render(&book.metadata, &final_offsets)?;
    debug_assert_eq!(final_fm.len(), preliminary_fm.len(), "padded frontmatter changed size");

    // 8. Write book.md.
    let book_md = format!("{final_fm}{}", body_result.body);
    let book_md_path = book_dir.join("book.md");
    fs::write(&book_md_path, book_md).map_err(|source| Error::Io { source, path: book_md_path.clone() })?;

    // 9. Copy images (skip cover).
    let images_dir = book_dir.join("images");
    let mut wrote_image_dir = false;
    for (manifest_path, bytes) in &book.images {
        if Some(manifest_path) == book.cover_image.as_ref() { continue; }
        let basename = match basenames.get(manifest_path) {
            Some(b) => b,
            None => continue,
        };
        if !wrote_image_dir {
            fs::create_dir_all(&images_dir).map_err(|source| Error::Io { source, path: images_dir.clone() })?;
            wrote_image_dir = true;
        }
        let out = images_dir.join(basename);
        fs::write(&out, bytes).map_err(|source| Error::Io { source, path: out.clone() })?;
    }

    Ok(())
}

fn parse_html_title(html: &str) -> Option<String> {
    let re = scraper::Selector::parse("title").unwrap();
    let doc = scraper::Html::parse_document(html);
    doc.select(&re).next().map(|el| el.text().collect::<String>())
}

fn rewrite_image_srcs(
    chapters: &mut [Chapter],
    basenames: &BTreeMap<String, String>,
    book: &Book,
) -> Result<()> {
    for ch in chapters.iter_mut() {
        let owning_dir = std::path::Path::new(&ch.source_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        for b in ch.blocks.iter_mut() {
            walk_block_images(b, &owning_dir, book, basenames)?;
        }
    }
    Ok(())
}

fn manifest_path_for_src(src: &str, owning_dir: &str) -> String {
    let resolved = if src.starts_with('/') {
        src.trim_start_matches('/').to_string()
    } else if owning_dir.is_empty() {
        src.to_string()
    } else {
        format!("{owning_dir}/{src}")
    };
    normalize(&resolved)
}

fn resolve_one_src(
    src: &str,
    owning_dir: &str,
    book: &Book,
    basenames: &BTreeMap<String, String>,
) -> Result<String> {
    if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
        return Ok(src.to_string());
    }
    let normalized = manifest_path_for_src(src, owning_dir);
    if !book.images.contains_key(&normalized) {
        return Err(Error::MissingImage(src.to_string()));
    }
    let basename = basenames
        .get(&normalized)
        .ok_or_else(|| Error::MissingImage(normalized.clone()))?;
    Ok(format!("images/{basename}"))
}

fn walk_block_images(
    b: &mut Block,
    owning_dir: &str,
    book: &Book,
    basenames: &BTreeMap<String, String>,
) -> Result<()> {
    match b {
        Block::Image { src, .. } => *src = resolve_one_src(src, owning_dir, book, basenames)?,
        Block::Heading { text, .. } | Block::Paragraph(text) => {
            walk_inline_images(text, owning_dir, book, basenames)?
        }
        Block::BlockQuote(c) => for x in c { walk_block_images(x, owning_dir, book, basenames)?; },
        Block::List { items, .. } => for it in items { for x in it { walk_block_images(x, owning_dir, book, basenames)?; } },
        Block::Table { header, rows } => {
            for c in header { walk_inline_images(c, owning_dir, book, basenames)?; }
            for r in rows { for c in r { walk_inline_images(c, owning_dir, book, basenames)?; } }
        }
        Block::FootnoteDef { content, .. } => for x in content { walk_block_images(x, owning_dir, book, basenames)?; },
        _ => {}
    }
    Ok(())
}

fn walk_inline_images(
    i: &mut crate::block::Inline,
    owning_dir: &str,
    book: &Book,
    basenames: &BTreeMap<String, String>,
) -> Result<()> {
    use crate::block::Inline;
    match i {
        Inline::Image { src, .. } => *src = resolve_one_src(src, owning_dir, book, basenames)?,
        Inline::Concat(xs) | Inline::Emphasis(xs) | Inline::Strong(xs) => for x in xs { walk_inline_images(x, owning_dir, book, basenames)?; },
        Inline::Link { children, .. } => for c in children { walk_inline_images(c, owning_dir, book, basenames)?; },
        _ => {}
    }
    Ok(())
}

fn split_footnotes(blocks: &[Block]) -> (Vec<Block>, Vec<Block>) {
    let mut body = Vec::with_capacity(blocks.len());
    let mut fns = Vec::new();
    for b in blocks {
        match b {
            Block::FootnoteDef { .. } => fns.push(b.clone()),
            _ => body.push(b.clone()),
        }
    }
    (body, fns)
}

fn normalize(p: &str) -> String {
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
