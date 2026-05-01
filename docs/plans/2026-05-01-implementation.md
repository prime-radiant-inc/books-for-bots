# books-for-bots Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust CLI that converts an EPUB into a single YAML-headed markdown file with chapter byte/line offsets, plus an `images/` subdirectory.

**Architecture:** Five-stage pipeline (load → extract → assemble → render → write). A `Block`/`Inline` tree is the only intermediate representation. Hand-written GFM serializer captures byte and line offsets for each chapter heading. Hand-written YAML frontmatter emits numeric offsets in fixed-width leading-padded fields, so the frontmatter byte size is known before the body is rendered and the whole file is produced in a single pass.

**Tech Stack:** Rust 2021. Production deps: `epub` (epub parser), `scraper` (html5ever DOM), `clap` (CLI), `slug`, `thiserror`. Dev deps: `epub-builder` (synthetic test fixtures), `insta` (snapshot tests), `tempfile`.

**Reference:** `docs/specs/2026-05-01-design.md` is the spec. This plan implements it.

---

## Module map

| File | Responsibility |
|---|---|
| `src/main.rs` | CLI entry. Parses args, calls `books_for_bots::convert`, sets exit code. |
| `src/lib.rs` | Public `convert()` function. Composes the pipeline. |
| `src/cli.rs` | clap derive `Args` struct. |
| `src/error.rs` | `thiserror`-derived `Error` enum. All fallible operations return `Result<T, Error>`. |
| `src/block.rs` | `Block` and `Inline` enums. No behavior. |
| `src/slug.rs` | Slug derivation from title+author or filename fallback. |
| `src/load.rs` | Wraps the `epub` crate. Returns a typed `Book` with metadata, spine, images, cover id. |
| `src/extract.rs` | Walks an `scraper` DOM into a `Vec<Block>`. |
| `src/assemble.rs` | Per-chapter title resolution, footnote ID namespacing, internal link rewriting, anchor injection. |
| `src/images.rs` | Basename collision resolver. |
| `src/render.rs` | Block tree → markdown string. Captures body-relative byte/line offsets for each chapter heading. |
| `src/frontmatter.rs` | Hand-emits the YAML frontmatter with leading-padded fixed-width offsets. |
| `src/write.rs` | Orchestrates: render body, build frontmatter, concatenate, write file, copy images. |
| `tests/common/mod.rs` | Test helper that builds in-process epubs via `epub-builder`. |
| `tests/integration.rs` | End-to-end pipeline tests. |
| `tests/determinism.rs` | Two-run byte-compare. |
| `tests/offsets.rs` | Verifies chapter byte/line offsets seek to the correct heading. |

---

## Task 1: Cargo scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`

- [ ] **Step 1: Initialize the crate**

Run: `cargo init --name books-for-bots`
Expected: creates `Cargo.toml`, `src/main.rs`, no Git changes (we already have a git repo).

- [ ] **Step 2: Replace `Cargo.toml`**

```toml
[package]
name = "books-for-bots"
version = "0.1.0"
edition = "2021"
description = "Convert EPUB to YAML-headed markdown with chapter offsets, for token-efficient agent reading"
license = "MIT"

[[bin]]
name = "books-for-bots"
path = "src/main.rs"

[lib]
name = "books_for_bots"
path = "src/lib.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
epub = "2"
scraper = "0.20"
slug = "0.1"
thiserror = "1"

[dev-dependencies]
epub-builder = "0.7"
insta = "1"
tempfile = "3"
```

- [ ] **Step 3: Replace `src/main.rs`**

```rust
use std::process::ExitCode;

fn main() -> ExitCode {
    match books_for_bots::run_from_args() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("books-for-bots: {e}");
            ExitCode::from(1)
        }
    }
}
```

- [ ] **Step 4: Create `src/lib.rs`**

```rust
//! books-for-bots: convert EPUBs to YAML-headed markdown with chapter offsets.

pub use error::Error;

mod error;

pub type Result<T> = std::result::Result<T, Error>;

/// Top-level entry point used by the binary.
pub fn run_from_args() -> Result<()> {
    Err(Error::NotImplemented)
}
```

- [ ] **Step 5: Create a placeholder `src/error.rs`** so `lib.rs` compiles

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("not implemented yet")]
    NotImplemented,
}
```

- [ ] **Step 6: Build and run**

Run: `cargo build`
Expected: compiles cleanly.

Run: `cargo run -- --help 2>&1 || true`
Expected: prints "not implemented yet" and exits 1.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/
git commit -m "Cargo scaffold for books-for-bots"
```

---

## Task 2: Error type

**Files:**
- Modify: `src/error.rs` (full replace)

- [ ] **Step 1: Write the failing test**

Append to `src/error.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn io_error_is_wrapped_with_context() {
        let inner = io::Error::new(io::ErrorKind::NotFound, "no file");
        let e: Error = Error::Io {
            source: inner,
            path: "x.epub".into(),
        };
        let msg = e.to_string();
        assert!(msg.contains("x.epub"), "got: {msg}");
    }

    #[test]
    fn output_exists_error_mentions_force() {
        let e = Error::OutputExists("out/foo".into());
        assert!(e.to_string().contains("--force"), "got: {e}");
    }
}
```

- [ ] **Step 2: Run the test (expected to fail)**

Run: `cargo test error::tests`
Expected: FAIL — variants don't exist yet.

- [ ] **Step 3: Replace `src/error.rs` with the full enum**

```rust
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error reading {path}: {source}")]
    Io {
        #[source]
        source: std::io::Error,
        path: PathBuf,
    },

    #[error("not a valid EPUB: {0}")]
    InvalidEpub(String),

    #[error("EPUB structure error: {0}")]
    EpubStructure(String),

    #[error("HTML parse error in {document}: {message}")]
    HtmlParse { document: String, message: String },

    #[error("output directory {0} already exists; pass --force to overwrite")]
    OutputExists(PathBuf),

    #[error("image referenced but missing from manifest: {0}")]
    MissingImage(String),

    #[error("offset {value} for chapter {chapter:?} exceeds 10-digit field")]
    OffsetOverflow { chapter: String, value: u64 },

    #[error("not implemented yet")]
    NotImplemented,
}

#[cfg(test)]
mod tests { /* keep the tests from Step 1 */ }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test error::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/error.rs
git commit -m "Define Error enum with thiserror"
```

---

## Task 3: CLI parser

**Files:**
- Create: `src/cli.rs`
- Modify: `src/lib.rs` to expose `cli` and use it in `run_from_args`.

- [ ] **Step 1: Write the failing test**

Create `src/cli.rs`:

```rust
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "books-for-bots", about = "Convert an EPUB to YAML-headed markdown")]
pub struct Args {
    /// Path to the input .epub file.
    pub input: PathBuf,

    /// Directory under which `<slug>/book.md` is written.
    #[arg(long, default_value = "output")]
    pub output_dir: PathBuf,

    /// Overwrite an existing output directory.
    #[arg(long)]
    pub force: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let a = Args::try_parse_from(["books-for-bots", "book.epub"]).unwrap();
        assert_eq!(a.input.to_str().unwrap(), "book.epub");
        assert_eq!(a.output_dir.to_str().unwrap(), "output");
        assert!(!a.force);
    }

    #[test]
    fn overrides() {
        let a = Args::try_parse_from(
            ["books-for-bots", "book.epub", "--output-dir", "out", "--force"]
        ).unwrap();
        assert_eq!(a.output_dir.to_str().unwrap(), "out");
        assert!(a.force);
    }
}
```

- [ ] **Step 2: Wire `cli` into `lib.rs`**

Replace `src/lib.rs`:

```rust
//! books-for-bots: convert EPUBs to YAML-headed markdown with chapter offsets.

pub use error::Error;

mod cli;
mod error;

pub type Result<T> = std::result::Result<T, Error>;

pub fn run_from_args() -> Result<()> {
    use clap::Parser;
    let _args = cli::Args::parse();
    Err(Error::NotImplemented)
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test cli::tests`
Expected: PASS (both tests).

- [ ] **Step 4: Smoke-test `--help`**

Run: `cargo run -- --help`
Expected: clap usage output mentioning `<INPUT>`, `--output-dir`, `--force`.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/lib.rs
git commit -m "Add clap-based CLI parser"
```

---

## Task 4: Block and Inline data model

**Files:**
- Create: `src/block.rs`
- Modify: `src/lib.rs` to add `pub mod block;` (exposed for test fixtures and integration).

- [ ] **Step 1: Create `src/block.rs`**

```rust
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
```

- [ ] **Step 2: Add module to `lib.rs`**

Modify `src/lib.rs`:

```rust
pub mod block;
mod cli;
mod error;
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean compile.

- [ ] **Step 4: Commit**

```bash
git add src/block.rs src/lib.rs
git commit -m "Define Block/Inline IR types"
```

---

## Task 5: Slug derivation

**Files:**
- Create: `src/slug.rs`
- Modify: `src/lib.rs` to add `pub mod slug;`.

- [ ] **Step 1: Write the failing tests**

Create `src/slug.rs`:

```rust
use std::path::Path;

/// Slug from epub metadata. `authors` is the list as parsed from the OPF.
/// Joins title and the first author with a hyphen, then slugifies.
/// Returns `None` if title is empty.
pub fn from_metadata(title: &str, authors: &[String]) -> Option<String> {
    if title.trim().is_empty() {
        return None;
    }
    let combined = match authors.first() {
        Some(a) if !a.trim().is_empty() => format!("{title} {a}"),
        _ => title.to_string(),
    };
    Some(::slug::slugify(combined))
}

/// Fallback when metadata is missing: slugify the file stem.
pub fn from_filename(path: &Path) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("book");
    ::slug::slugify(stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_typical() {
        let s = from_metadata("How to Take Smart Notes", &["Sönke Ahrens".to_string()]).unwrap();
        assert_eq!(s, "how-to-take-smart-notes-sonke-ahrens");
    }

    #[test]
    fn metadata_no_author() {
        let s = from_metadata("Untitled Book", &[]).unwrap();
        assert_eq!(s, "untitled-book");
    }

    #[test]
    fn metadata_empty_title_returns_none() {
        assert!(from_metadata("   ", &["Anyone".to_string()]).is_none());
    }

    #[test]
    fn filename_fallback() {
        let p = Path::new("/tmp/Some Book - Foo.epub");
        assert_eq!(from_filename(p), "some-book-foo");
    }
}
```

- [ ] **Step 2: Add to `lib.rs`**

Modify `src/lib.rs`:

```rust
pub mod block;
pub mod slug;
mod cli;
mod error;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test slug::tests`
Expected: PASS (4 tests).

- [ ] **Step 4: Commit**

```bash
git add src/slug.rs src/lib.rs
git commit -m "Slug derivation from metadata or filename"
```

---

## Task 6: Load module — wraps the `epub` crate

**Files:**
- Create: `src/load.rs`
- Create: `tests/common/mod.rs` (test fixture builder)
- Modify: `src/lib.rs` to add `pub mod load;`.
- Modify: `src/error.rs` for any new variants needed (none expected; reuse).

- [ ] **Step 1: Create the test fixture builder**

Create `tests/common/mod.rs`:

```rust
//! Builds in-process EPUBs for tests. No real-book content is committed;
//! all fixtures are constructed at test time from synthetic prose.

use epub_builder::{EpubBuilder, EpubContent, ReferenceType, ZipLibrary};
use std::io::Cursor;

pub struct Fixture {
    pub bytes: Vec<u8>,
}

pub struct ChapterSpec {
    pub title: &'static str,
    pub html: &'static str,
}

pub fn build_minimal_book(
    title: &str,
    author: &str,
    chapters: &[ChapterSpec],
) -> Fixture {
    let mut buf = Cursor::new(Vec::new());
    let mut builder = EpubBuilder::new(ZipLibrary::new().expect("zip lib")).expect("builder");
    builder
        .metadata("title", title)
        .expect("title")
        .metadata("author", author)
        .expect("author");
    for (i, ch) in chapters.iter().enumerate() {
        let path = format!("chap_{i:03}.xhtml");
        let html = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><head><title>{}</title></head>
<body>{}</body></html>"#,
            ch.title, ch.html
        );
        builder
            .add_content(
                EpubContent::new(&path, html.as_bytes())
                    .title(ch.title)
                    .reftype(ReferenceType::Text),
            )
            .expect("add content");
    }
    builder.generate(&mut buf).expect("generate");
    Fixture { bytes: buf.into_inner() }
}
```

- [ ] **Step 2: Write the failing test**

Create `src/load.rs`:

```rust
use crate::Result;
use std::collections::BTreeMap;
use std::path::Path;

pub struct Book {
    pub metadata: Metadata,
    pub spine: Vec<SpineDoc>,
    /// Manifest path → bytes for every image-typed resource.
    pub images: BTreeMap<String, Vec<u8>>,
    /// Manifest path of the cover image, if declared.
    pub cover_image: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct Metadata {
    pub title: String,
    pub authors: Vec<String>,
    pub publisher: Option<String>,
    pub published: Option<String>,
    pub isbn: Option<String>,
    pub language: Option<String>,
    pub source_file: String,
}

pub struct SpineDoc {
    /// Manifest path (relative to the OPF directory).
    pub manifest_path: String,
    /// Resolved title from the navigation document, if any.
    pub toc_title: Option<String>,
    /// Raw (UTF-8) HTML body of the spine document.
    pub html: String,
}

pub fn open(_path: &Path) -> Result<Book> {
    todo!("implement in step 4")
}
```

Create `tests/load_smoke.rs`:

```rust
mod common;

use books_for_bots::load;

#[test]
fn loads_minimal_two_chapter_book() {
    let fx = common::build_minimal_book(
        "Hello",
        "An Author",
        &[
            common::ChapterSpec { title: "One", html: "<p>First chapter.</p>" },
            common::ChapterSpec { title: "Two", html: "<p>Second chapter.</p>" },
        ],
    );
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), &fx.bytes).unwrap();

    let book = load::open(tmp.path()).expect("open");
    assert_eq!(book.metadata.title, "Hello");
    assert_eq!(book.metadata.authors, vec!["An Author".to_string()]);
    assert_eq!(book.spine.len(), 2);
    assert_eq!(book.spine[0].toc_title.as_deref(), Some("One"));
    assert!(book.spine[0].html.contains("First chapter"));
}
```

- [ ] **Step 3: Run the test (expected to fail)**

Run: `cargo test --test load_smoke`
Expected: FAIL with `not yet implemented`.

- [ ] **Step 4: Implement `load::open`**

Replace the `pub fn open` stub:

```rust
pub fn open(path: &Path) -> Result<Book> {
    use crate::Error;
    use epub::doc::EpubDoc;

    let mut doc = EpubDoc::new(path).map_err(|e| Error::InvalidEpub(e.to_string()))?;

    let metadata = Metadata {
        title: doc.mdata("title").unwrap_or_default(),
        authors: doc.metadata.get("creator").cloned().unwrap_or_default(),
        publisher: doc.mdata("publisher"),
        published: doc.mdata("date"),
        isbn: doc.mdata("identifier"),
        language: doc.mdata("language"),
        source_file: path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string(),
    };

    let toc_titles: BTreeMap<String, String> = doc
        .toc
        .iter()
        .map(|n| (n.content.to_string_lossy().to_string(), n.label.clone()))
        .collect();

    // Walk the spine. EpubDoc maintains a cursor; iterate by index.
    let spine_count = doc.spine.len();
    let mut spine = Vec::with_capacity(spine_count);
    for i in 0..spine_count {
        doc.set_current_page(i);
        let manifest_path = doc
            .get_current_path()
            .map(|p| p.to_string_lossy().to_string())
            .ok_or_else(|| Error::EpubStructure(format!("spine index {i} has no path")))?;
        let html = doc
            .get_current_str()
            .map(|(s, _)| s)
            .ok_or_else(|| Error::EpubStructure(format!("spine index {i} unreadable")))?;
        let toc_title = toc_titles.get(&manifest_path).cloned();
        spine.push(SpineDoc { manifest_path, toc_title, html });
    }

    let mut images: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let resource_paths: Vec<String> = doc
        .resources
        .iter()
        .filter(|(_, (_, mime))| mime.starts_with("image/"))
        .map(|(_, (path, _))| path.to_string_lossy().to_string())
        .collect();
    for rp in resource_paths {
        if let Some((bytes, _)) = doc.get_resource_by_path(&rp) {
            images.insert(rp, bytes);
        }
    }

    let cover_image = doc
        .get_cover()
        .and_then(|(_bytes, id_or_path)| Some(id_or_path));

    Ok(Book { metadata, spine, images, cover_image })
}
```

> **Note:** the exact `epub` crate API names above are version-2.x style. If `mdata` is named differently, consult `cargo doc --open` for the installed version and adjust. The test will tell you when names mismatch.

- [ ] **Step 5: Run the test**

Run: `cargo test --test load_smoke`
Expected: PASS.

- [ ] **Step 6: Add unit test for missing-file error**

Append to `src/load.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn missing_file_is_invalid_epub() {
        let r = open(Path::new("/nonexistent/book.epub"));
        assert!(matches!(r, Err(crate::Error::InvalidEpub(_))));
    }
}
```

Run: `cargo test load::tests`
Expected: PASS.

- [ ] **Step 7: Wire into `lib.rs`**

Modify `src/lib.rs`:

```rust
pub mod block;
pub mod load;
pub mod slug;
mod cli;
mod error;
```

- [ ] **Step 8: Commit**

```bash
git add src/load.rs src/lib.rs tests/common/mod.rs tests/load_smoke.rs Cargo.toml Cargo.lock
git commit -m "Load module: wraps epub crate, returns typed Book"
```

---

## Task 7: Image collision resolver

**Files:**
- Create: `src/images.rs`
- Modify: `src/lib.rs` to add `pub mod images;`.

- [ ] **Step 1: Write the failing tests**

Create `src/images.rs`:

```rust
use std::collections::BTreeMap;
use std::path::Path;

/// Map manifest path → output basename. Collisions on basename are resolved
/// by appending `-2`, `-3`, etc., assigned in sorted manifest-path order.
pub fn resolve_basenames<I, S>(manifest_paths: I) -> BTreeMap<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut sorted: Vec<String> = manifest_paths.into_iter().map(|s| s.as_ref().to_string()).collect();
    sorted.sort();

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut out: BTreeMap<String, String> = BTreeMap::new();

    for path in sorted {
        let basename = Path::new(&path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("image")
            .to_string();
        let count = counts.entry(basename.clone()).or_insert(0);
        *count += 1;
        let output = if *count == 1 {
            basename
        } else {
            // foo.jpg → foo-2.jpg
            let (stem, ext) = match basename.rsplit_once('.') {
                Some((s, e)) => (s.to_string(), format!(".{e}")),
                None => (basename.clone(), String::new()),
            };
            format!("{stem}-{count}{ext}")
        };
        out.insert(path, output);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_collision_keeps_basename() {
        let m = resolve_basenames(["OEBPS/images/cat.jpg", "OEBPS/images/dog.jpg"]);
        assert_eq!(m.get("OEBPS/images/cat.jpg").unwrap(), "cat.jpg");
        assert_eq!(m.get("OEBPS/images/dog.jpg").unwrap(), "dog.jpg");
    }

    #[test]
    fn collision_suffixed() {
        let m = resolve_basenames(["OEBPS/images/foo.jpg", "OEBPS/figs/foo.jpg"]);
        assert_eq!(m.get("OEBPS/figs/foo.jpg").unwrap(), "foo.jpg");
        assert_eq!(m.get("OEBPS/images/foo.jpg").unwrap(), "foo-2.jpg");
    }

    #[test]
    fn three_way_collision_in_sorted_order() {
        let m = resolve_basenames([
            "z/foo.png", "a/foo.png", "m/foo.png",
        ]);
        assert_eq!(m.get("a/foo.png").unwrap(), "foo.png");
        assert_eq!(m.get("m/foo.png").unwrap(), "foo-2.png");
        assert_eq!(m.get("z/foo.png").unwrap(), "foo-3.png");
    }

    #[test]
    fn no_extension() {
        let m = resolve_basenames(["a/x", "b/x"]);
        assert_eq!(m.get("a/x").unwrap(), "x");
        assert_eq!(m.get("b/x").unwrap(), "x-2");
    }
}
```

- [ ] **Step 2: Add module to `lib.rs`**

```rust
pub mod block;
pub mod images;
pub mod load;
pub mod slug;
mod cli;
mod error;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test images::tests`
Expected: PASS (4 tests).

- [ ] **Step 4: Commit**

```bash
git add src/images.rs src/lib.rs
git commit -m "Image basename collision resolver"
```

---

## Task 8: Extract — block-level structures

Builds the DOM-walking foundation. Each subsequent extract task adds element types incrementally with TDD.

**Files:**
- Create: `src/extract.rs`
- Modify: `src/lib.rs` to add `pub mod extract;`.

- [ ] **Step 1: Skeleton + paragraph + heading test**

Create `src/extract.rs`:

```rust
use crate::block::{Block, Inline};
use scraper::{Html, Node, ElementRef};
use ego_tree::NodeRef;

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
```

- [ ] **Step 2: Add `ego-tree` dep if needed**

Run: `cargo build`
Expected: may complain about `ego-tree`. If so, add to `Cargo.toml` `[dependencies]`:

```toml
ego-tree = "0.6"
```

(`scraper` re-exports its tree types but the `Node` enum lives in `ego-tree`.)

- [ ] **Step 3: Run the tests**

Run: `cargo test extract::tests`
Expected: 5 tests PASS.

- [ ] **Step 4: Wire into `lib.rs`**

```rust
pub mod block;
pub mod extract;
pub mod images;
pub mod load;
pub mod slug;
mod cli;
mod error;
```

- [ ] **Step 5: Commit**

```bash
git add src/extract.rs src/lib.rs Cargo.toml Cargo.lock
git commit -m "Extract: paragraphs, headings, transparent containers, whitespace collapsing"
```

---

## Task 9: Extract — inline formatting

Adds emphasis, strong, code, links, line breaks, inline images.

**Files:**
- Modify: `src/extract.rs`

- [ ] **Step 1: Write failing tests**

Append to `src/extract.rs` (`mod tests`):

```rust
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
```

- [ ] **Step 2: Run failing tests**

Run: `cargo test extract::tests`
Expected: 4 new tests FAIL.

- [ ] **Step 3: Extend `inline_of` to handle inline element names**

Modify `src/extract.rs` — wrap the `Node::Element(_)` arm of `inline_of` to look at the element's tag name:

```rust
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
```

Add the helper:

```rust
fn plain_text(el: ElementRef) -> String {
    let mut s = String::new();
    for child in el.descendants() {
        if let Node::Text(t) = child.value() {
            s.push_str(t);
        }
    }
    s
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test extract::tests`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/extract.rs
git commit -m "Extract: inline formatting (em/strong/code/a/br/img)"
```

---

## Task 10: Extract — lists, blockquote, hr, code blocks

**Files:**
- Modify: `src/extract.rs`

- [ ] **Step 1: Write failing tests**

Append to the test module:

```rust
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
```

- [ ] **Step 2: Run failing tests**

Run: `cargo test extract::tests`
Expected: 7 new tests FAIL.

- [ ] **Step 3: Extend `extract_into` block-level matcher**

Add arms to the `match name` block in `extract_into`:

```rust
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
```

Add this helper above `extract_into`:

```rust
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
        let inl = inline_of(li);
        if inl.is_empty() {
            return vec![Block::Paragraph(Inline::empty())];
        }
        return vec![Block::Paragraph(inl)];
    }
    let mut out = Vec::new();
    let mut inline_buf: Vec<Inline> = Vec::new();
    let flush = |buf: &mut Vec<Inline>, out: &mut Vec<Block>| {
        if !buf.is_empty() {
            let inl = if buf.len() == 1 { buf.remove(0) } else { Inline::Concat(std::mem::take(buf)) };
            if !inl.is_empty() {
                out.push(Block::Paragraph(inl));
            }
            buf.clear();
        }
    };
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
                        // Inline-ish: reuse inline_of by wrapping into a virtual parent.
                        // Simplest: build an Inline by recursing inline_of on this element only.
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
            let kids = match inline_of(el) {
                Inline::Concat(v) => v,
                other => vec![other],
            };
            Inline::Emphasis(kids)
        }
        "strong" | "b" => {
            let kids = match inline_of(el) {
                Inline::Concat(v) => v,
                other => vec![other],
            };
            Inline::Strong(kids)
        }
        "code" => Inline::Code(plain_text(el)),
        "br" => Inline::LineBreak,
        "a" => {
            let href = el.value().attr("href").unwrap_or("").to_string();
            let kids = match inline_of(el) {
                Inline::Concat(v) => v,
                other => vec![other],
            };
            Inline::Link { href, children: kids }
        }
        "img" => Inline::Image {
            src: el.value().attr("src").unwrap_or("").to_string(),
            alt: el.value().attr("alt").unwrap_or("").to_string(),
            title: el.value().attr("title").map(str::to_string),
        },
        _ => inline_of(el),
    }
}

- [ ] **Step 4: Run tests**

Run: `cargo test extract::tests`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/extract.rs
git commit -m "Extract: lists, blockquote, hr, code blocks"
```

---

## Task 11: Extract — tables

**Files:**
- Modify: `src/extract.rs`

- [ ] **Step 1: Write failing test**

```rust
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
```

- [ ] **Step 2: Run failing tests**

Run: `cargo test extract::tests::simple_table extract::tests::table_no_thead`
Expected: FAIL.

- [ ] **Step 3: Add `"table"` arm to `extract_into`**

```rust
        "table" => {
            // Find header: prefer <thead>, else first <tr> with <th>.
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
                    .map(inline_of)
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
```

- [ ] **Step 4: Run tests**

Run: `cargo test extract::tests`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/extract.rs
git commit -m "Extract: tables"
```

---

## Task 12: Extract — footnotes

EPUB footnotes use either `epub:type="noteref"` on a link, or a `<sup>` containing a link with an in-page `href="#fnX"`. Definitions live in `<aside epub:type="footnote">` or `<div class="footnote">`.

**Files:**
- Modify: `src/extract.rs`

- [ ] **Step 1: Write failing tests**

```rust
    #[test]
    fn noteref_with_epub_type() {
        let html = r#"<html><body><p>See<a epub:type="noteref" href="#fn1">1</a>.</p></body></html>"#;
        let b = parse(html);
        let Block::Paragraph(Inline::Concat(parts)) = &b[0] else { panic!() };
        assert!(parts.iter().any(|i| matches!(i, Inline::FootnoteRef(s) if s == "fn1")));
    }

    #[test]
    fn sup_anchor_is_noteref() {
        let html = r#"<html><body><p>x<sup><a href="#fn2">2</a></sup>.</p></body></html>"#;
        let b = parse(html);
        let Block::Paragraph(Inline::Concat(parts)) = &b[0] else { panic!() };
        assert!(parts.iter().any(|i| matches!(i, Inline::FootnoteRef(s) if s == "fn2")));
    }

    #[test]
    fn footnote_def_aside() {
        let html = r#"<html><body><aside epub:type="footnote" id="fn1"><p>Note one.</p></aside></body></html>"#;
        let b = parse(html);
        assert_eq!(b.len(), 1);
        let Block::FootnoteDef { id, content } = &b[0] else { panic!() };
        assert_eq!(id, "fn1");
        assert_eq!(content, &vec![Block::Paragraph(Inline::Text("Note one.".into()))]);
    }
```

- [ ] **Step 2: Run failing tests** — expect FAIL.

- [ ] **Step 3: Update inline `<a>` arm to detect notereferences**

In `inline_of`, replace the `"a"` arm with:

```rust
                        "a" => {
                            let href = ce.value().attr("href").unwrap_or("").to_string();
                            let is_noteref = ce
                                .value()
                                .attr("epub:type")
                                .map(|t| t.contains("noteref"))
                                .unwrap_or(false)
                                || href.starts_with('#') && parent_is_sup(ce);
                            if is_noteref {
                                let id = href.trim_start_matches('#').to_string();
                                Inline::FootnoteRef(id)
                            } else {
                                let kids = match inline_of(ce) {
                                    Inline::Concat(v) => v,
                                    other => vec![other],
                                };
                                Inline::Link { href, children: kids }
                            }
                        }
```

Add helper:

```rust
fn parent_is_sup(el: ElementRef) -> bool {
    el.parent()
        .and_then(ElementRef::wrap)
        .map(|p| p.value().name() == "sup")
        .unwrap_or(false)
}
```

- [ ] **Step 4: Add footnote-def block arm**

In `extract_into`, add before the catch-all:

```rust
        "aside" | "div" if is_footnote_container(el) => {
            let id = el.value().attr("id").unwrap_or("").to_string();
            let content = extract_blocks(el);
            out.push(Block::FootnoteDef { id, content });
        }
```

Note: `aside` already isn't in the transparent list, so this needs to come before the catch-all but after the `div` case. Reorganize: instead of grouping `div` with transparent containers, give `div` its own arm that checks for footnote class first, falling through to transparent.

Helper:

```rust
fn is_footnote_container(el: ElementRef) -> bool {
    let v = el.value();
    let is_aside_footnote = v.name() == "aside"
        && v.attr("epub:type").map(|t| t.contains("footnote")).unwrap_or(false);
    let is_div_footnote = v.name() == "div"
        && v.attr("class").map(|c| c.split_whitespace().any(|t| t == "footnote")).unwrap_or(false);
    is_aside_footnote || is_div_footnote
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test extract::tests`
Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add src/extract.rs
git commit -m "Extract: footnote refs and definitions"
```

---

## Task 13: Chapter title resolution

**Files:**
- Create: `src/assemble.rs`
- Modify: `src/lib.rs` to add `pub mod assemble;`.

- [ ] **Step 1: Write failing tests**

Create `src/assemble.rs`:

```rust
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
```

- [ ] **Step 2: Add module**

Modify `src/lib.rs`:

```rust
pub mod assemble;
pub mod block;
pub mod extract;
pub mod images;
pub mod load;
pub mod slug;
mod cli;
mod error;
```

- [ ] **Step 3: Run tests**

Run: `cargo test assemble::tests::`
Expected: 5 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add src/assemble.rs src/lib.rs
git commit -m "Chapter title resolution priority"
```

---

## Task 14: Assemble — namespacing, link rewriting, anchor injection

**Files:**
- Modify: `src/assemble.rs`

- [ ] **Step 1: Write failing tests**

Append to `src/assemble.rs`:

```rust
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
///
/// Internal cross-document link rewriting is done in `rewrite_internal_links`
/// (Step 4) once all chapters are known.
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
```

- [ ] **Step 2: Run tests**

Run: `cargo test assemble::ns_tests`
Expected: PASS.

- [ ] **Step 3: Add internal link rewriting**

Add to `src/assemble.rs`:

```rust
use std::collections::BTreeMap;

/// Rewrite cross-document hrefs to in-file anchors.
///
/// `path_to_chapter`: manifest_path → chapter number.
/// In-document links (`#foo` with no path) are rewritten to `#c{n}-foo` for the
/// chapter that owns the link. Links to other chapters' files are rewritten to
/// `#c{m}-bar` (or `#chapter-{m}` for a bare-document href like `other.xhtml`).
pub fn rewrite_internal_links(
    chapters: &mut [Chapter],
    path_to_chapter: &BTreeMap<String, usize>,
) {
    for chap in chapters.iter_mut() {
        let owning = chap.number;
        let owning_dir = std::path::Path::new(&chap.source_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        for b in chap.blocks.iter_mut() {
            rewrite_links_in_block(b, owning, &owning_dir, path_to_chapter);
        }
    }
}

fn rewrite_links_in_block(
    b: &mut Block,
    owning: usize,
    owning_dir: &str,
    map: &BTreeMap<String, usize>,
) {
    match b {
        Block::Heading { text, .. } | Block::Paragraph(text) => rewrite_links_in_inline(text, owning, owning_dir, map),
        Block::BlockQuote(c) => for x in c { rewrite_links_in_block(x, owning, owning_dir, map); },
        Block::List { items, .. } => for item in items { for x in item { rewrite_links_in_block(x, owning, owning_dir, map); } },
        Block::Table { header, rows } => {
            for c in header { rewrite_links_in_inline(c, owning, owning_dir, map); }
            for r in rows { for c in r { rewrite_links_in_inline(c, owning, owning_dir, map); } }
        }
        Block::FootnoteDef { content, .. } => for c in content { rewrite_links_in_block(c, owning, owning_dir, map); },
        _ => {}
    }
}

fn rewrite_links_in_inline(
    i: &mut Inline,
    owning: usize,
    owning_dir: &str,
    map: &BTreeMap<String, usize>,
) {
    match i {
        Inline::Link { href, children } => {
            *href = rewrite_one_href(href, owning, owning_dir, map);
            for c in children { rewrite_links_in_inline(c, owning, owning_dir, map); }
        }
        Inline::Concat(xs) | Inline::Emphasis(xs) | Inline::Strong(xs) => {
            for x in xs { rewrite_links_in_inline(x, owning, owning_dir, map); }
        }
        _ => {}
    }
}

fn rewrite_one_href(
    href: &str,
    owning: usize,
    owning_dir: &str,
    map: &BTreeMap<String, usize>,
) -> String {
    if href.is_empty() { return href.to_string(); }
    if href.starts_with("http://") || href.starts_with("https://") || href.starts_with("mailto:") {
        return href.to_string();
    }
    if let Some(frag) = href.strip_prefix('#') {
        return format!("#c{owning}-{frag}");
    }
    let (path_part, frag_part) = match href.split_once('#') {
        Some((p, f)) => (p.to_string(), Some(f.to_string())),
        None => (href.to_string(), None),
    };
    // Resolve relative to owning chapter's directory.
    let resolved = if owning_dir.is_empty() {
        path_part.clone()
    } else {
        format!("{owning_dir}/{path_part}")
    };
    let normalized = normalize_path(&resolved);
    if let Some(&target) = map.get(&normalized) {
        match frag_part {
            Some(f) => format!("#c{target}-{f}"),
            None => format!("#chapter-{target}"),
        }
    } else {
        // Unknown target: leave href alone (won't be a working link, but won't crash).
        href.to_string()
    }
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
    fn intra_doc_fragment() {
        let mut chap = Chapter { number: 4, title: "T".into(), source_path: "OEBPS/c.xhtml".into(), blocks: vec![p_link("#sec1")] };
        let map: BTreeMap<String, usize> = BTreeMap::new();
        rewrite_internal_links(std::slice::from_mut(&mut chap), &map);
        let Block::Paragraph(Inline::Link { href, .. }) = &chap.blocks[0] else { panic!() };
        assert_eq!(href, "#c4-sec1");
    }

    #[test]
    fn cross_doc_with_fragment() {
        let mut chap = Chapter { number: 2, title: "T".into(), source_path: "OEBPS/a.xhtml".into(), blocks: vec![p_link("b.xhtml#foo")] };
        let mut map = BTreeMap::new();
        map.insert("OEBPS/b.xhtml".to_string(), 5);
        rewrite_internal_links(std::slice::from_mut(&mut chap), &map);
        let Block::Paragraph(Inline::Link { href, .. }) = &chap.blocks[0] else { panic!() };
        assert_eq!(href, "#c5-foo");
    }

    #[test]
    fn cross_doc_no_fragment() {
        let mut chap = Chapter { number: 2, title: "T".into(), source_path: "OEBPS/a.xhtml".into(), blocks: vec![p_link("b.xhtml")] };
        let mut map = BTreeMap::new();
        map.insert("OEBPS/b.xhtml".to_string(), 5);
        rewrite_internal_links(std::slice::from_mut(&mut chap), &map);
        let Block::Paragraph(Inline::Link { href, .. }) = &chap.blocks[0] else { panic!() };
        assert_eq!(href, "#chapter-5");
    }

    #[test]
    fn external_unchanged() {
        let mut chap = Chapter { number: 1, title: "T".into(), source_path: "x".into(), blocks: vec![p_link("https://example.com")] };
        rewrite_internal_links(std::slice::from_mut(&mut chap), &BTreeMap::new());
        let Block::Paragraph(Inline::Link { href, .. }) = &chap.blocks[0] else { panic!() };
        assert_eq!(href, "https://example.com");
    }
}
```

- [ ] **Step 4: Add anchor injection**

Add to `src/assemble.rs`:

```rust
/// For every element in the original DOM that had an `id="foo"` and isn't
/// a heading, the extractor preserves nothing — the assembler injects
/// `Block::Anchor { id: "foo" }` immediately before the block that bore the id.
///
/// In this pipeline, anchor injection is performed at extraction time by
/// scanning the DOM for non-heading elements with `id` attributes and
/// emitting an `Anchor` block before the corresponding extracted block.
/// This function only namespaces them (already handled in `namespace_chapter`).
///
/// This is a documentation-only function; the actual injection lives in
/// `extract::extract_into`. See Task 15.
pub fn _anchor_doc() {}
```

- [ ] **Step 5: Run all tests**

Run: `cargo test assemble::`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/assemble.rs
git commit -m "Assemble: chapter struct, footnote namespacing, internal link rewriting"
```

---

## Task 15: Extract — anchor preservation for non-heading IDs

**Files:**
- Modify: `src/extract.rs`

- [ ] **Step 1: Write failing test**

```rust
    #[test]
    fn id_on_paragraph_yields_anchor_block() {
        let html = r#"<html><body><p id="sec1">Body.</p></body></html>"#;
        let b = parse(html);
        assert_eq!(b, vec![
            Block::Anchor { id: "sec1".into() },
            Block::Paragraph(Inline::Text("Body.".into())),
        ]);
    }

    #[test]
    fn id_on_heading_does_not_yield_anchor() {
        let html = r#"<html><body><h2 id="x">Title</h2></body></html>"#;
        let b = parse(html);
        assert_eq!(b, vec![Block::Heading { level: 2, text: Inline::Text("Title".into()) }]);
    }
```

- [ ] **Step 2: Run failing tests** — expect FAIL.

- [ ] **Step 3: Modify `extract_into`**

At the top of the function, before the `match name`, capture an optional anchor:

```rust
    let anchor_id = el
        .value()
        .attr("id")
        .filter(|_| !matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6"))
        .map(str::to_string);
    if let Some(id) = anchor_id {
        out.push(Block::Anchor { id });
    }
```

- [ ] **Step 4: Run tests** — all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/extract.rs
git commit -m "Extract: anchor blocks for non-heading IDs"
```

---

## Task 16: Render — paragraphs, headings, hr, blockquote

The renderer holds a `String` buffer and a `Vec<ChapterOffset>`. Every emit increments byte and line counters. The dedicated `start_chapter` method captures the offset at the start of each chapter heading line.

**Files:**
- Create: `src/render.rs`
- Modify: `src/lib.rs` to add `pub mod render;`.

- [ ] **Step 1: Write failing tests**

Create `src/render.rs`:

```rust
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
        // Record the offset at the start of the heading line — this is what
        // we want consumers to seek to.
        self.offsets.push(ChapterOffset { byte: self.current_byte(), line: self.line });
        self.write_raw(&format!("## {title}\n\n"));
    }
}
```

- [ ] **Step 2: Implement `render_block` for paragraph/heading/hr/blockquote/anchor**

Add to `Renderer`:

```rust
    fn render_block(&mut self, b: &Block) {
        match b {
            Block::Paragraph(i) => {
                self.ensure_blank_line();
                self.render_inline(i);
                self.write_raw("\n\n");
            }
            Block::Heading { level, text } => {
                self.ensure_blank_line();
                // Per spec: in-chapter headings shift down one (h1→##, h2→###...).
                // But the chapter heading itself is emitted by start_chapter.
                // In-document <h1> becomes ##; <h2> becomes ###; etc.
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
            // others handled in later tasks
            _ => { /* placeholder until Tasks 17–20 */ }
        }
    }

    fn render_inline(&mut self, i: &Inline) {
        match i {
            Inline::Text(s) => self.write_raw(s),
            Inline::Concat(xs) => for x in xs { self.render_inline(x); },
            // others in next task
            _ => {}
        }
    }
```

- [ ] **Step 3: Test**

Append to `src/render.rs`:

```rust
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
```

Run: `cargo test render::tests`
Expected: 5 tests PASS.

- [ ] **Step 4: Wire into `lib.rs`**

```rust
pub mod assemble;
pub mod block;
pub mod extract;
pub mod images;
pub mod load;
pub mod render;
pub mod slug;
mod cli;
mod error;
```

- [ ] **Step 5: Commit**

```bash
git add src/render.rs src/lib.rs
git commit -m "Render: paragraph, heading, hr, blockquote, chapter offsets"
```

---

## Task 17: Render — inline (emphasis, strong, code, link, image, line break, footnote ref)

**Files:**
- Modify: `src/render.rs`

- [ ] **Step 1: Write failing tests**

Add to `mod tests`:

```rust
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
        assert!(s.contains("a  \nb"));  // GFM hard break: two trailing spaces + newline
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
```

- [ ] **Step 2: Run failing tests** — expect FAIL.

- [ ] **Step 3: Implement inline arms**

Replace `render_inline`:

```rust
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
```

Add helper:

```rust
fn backtick_fence_for(s: &str) -> String {
    let mut max_run = 0usize;
    let mut cur = 0usize;
    for c in s.chars() {
        if c == '`' { cur += 1; max_run = max_run.max(cur); } else { cur = 0; }
    }
    "`".repeat(max_run + 1)
}
```

Add `Block::Image` arm to `render_block`:

```rust
            Block::Image { src, alt, title } => {
                self.ensure_blank_line();
                match title {
                    Some(t) => self.write_raw(&format!(r#"![{alt}]({src} "{t}")"#)),
                    None => self.write_raw(&format!("![{alt}]({src})")),
                }
                self.write_raw("\n\n");
            }
```

- [ ] **Step 4: Run tests** — all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/render.rs
git commit -m "Render: inline formatting and block images"
```

---

## Task 18: Render — lists

**Files:**
- Modify: `src/render.rs`

- [ ] **Step 1: Write failing tests**

```rust
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
```

- [ ] **Step 2: Run failing tests** — expect FAIL.

- [ ] **Step 3: Implement list rendering**

Add to `render_block`:

```rust
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
                        if first_line {
                            self.write_raw(&marker);
                            first_line = false;
                        } else if !line.trim().is_empty() {
                            self.write_raw(&indent);
                        }
                        self.write_raw(line);
                    }
                    if !trimmed.ends_with('\n') {
                        self.write_raw("\n");
                    }
                }
                self.write_raw("\n");
            }
```

- [ ] **Step 4: Run tests** — all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/render.rs
git commit -m "Render: lists with two-space nested indent"
```

---

## Task 19: Render — tables

**Files:**
- Modify: `src/render.rs`

- [ ] **Step 1: Write failing test**

```rust
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
```

- [ ] **Step 2: Implement table rendering**

Add this method to `Renderer`:

```rust
    fn render_cell(&mut self, i: &Inline) {
        let mut tmp = Renderer::new();
        tmp.render_inline(i);
        let escaped = tmp.buf.replace('\n', "<br>").replace('|', "\\|");
        self.write_raw(&escaped);
    }
```

Add the table arm to `render_block`:

```rust
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
```

- [ ] **Step 3: Run tests** — PASS.

- [ ] **Step 4: Commit**

```bash
git add src/render.rs
git commit -m "Render: GFM tables with pipe escaping and br for cell newlines"
```

---

## Task 20: Render — code blocks and footnote definitions

**Files:**
- Modify: `src/render.rs`

- [ ] **Step 1: Write failing tests**

```rust
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
```

- [ ] **Step 2: Implement**

```rust
            Block::CodeBlock { lang, code } => {
                self.ensure_blank_line();
                let fence_len = code
                    .lines()
                    .map(|l| {
                        let mut n = 0usize;
                        for c in l.chars() {
                            if c == '`' { n += 1; } else if n > 0 { break; }
                        }
                        n
                    })
                    .max()
                    .unwrap_or(0)
                    .max(2)
                    + 1;
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
                let body = sub.buf.trim();
                self.write_raw(&format!("[^{id}]: "));
                // Indent continuation lines by 4 spaces per CommonMark footnotes.
                let mut first = true;
                for line in body.split_inclusive('\n') {
                    if first { first = false; } else { self.write_raw("    "); }
                    self.write_raw(line);
                }
                self.write_raw("\n\n");
            }
```

- [ ] **Step 3: Run tests** — all PASS.

- [ ] **Step 4: Commit**

```bash
git add src/render.rs
git commit -m "Render: fenced code blocks and footnote definitions"
```

---

## Task 21: Frontmatter emission

**Files:**
- Create: `src/frontmatter.rs`
- Modify: `src/lib.rs` to add `pub mod frontmatter;`.

- [ ] **Step 1: Write failing tests**

Create `src/frontmatter.rs`:

```rust
use crate::load::Metadata;
use crate::render::ChapterOffset;

pub const NUMERIC_WIDTH: usize = 10;

pub struct FrontmatterChapter<'a> {
    pub title: &'a str,
    pub offset: ChapterOffset,
}

/// Render the frontmatter as a UTF-8 string ending in `---\n`. The byte size
/// and line count are intrinsic to the returned string.
pub fn render(meta: &Metadata, chapters: &[FrontmatterChapter<'_>]) -> Result<String, crate::Error> {
    use crate::Error;
    let mut s = String::new();
    s.push_str("---\n");
    write_kv(&mut s, "title", &meta.title);
    if !meta.authors.is_empty() {
        let joined = meta.authors.iter().map(|a| yaml_inline_string(a)).collect::<Vec<_>>().join(", ");
        s.push_str(&format!("authors: [{joined}]\n"));
    }
    if let Some(p) = &meta.publisher { write_kv(&mut s, "publisher", p); }
    if let Some(p) = &meta.published { write_kv(&mut s, "published", p); }
    if let Some(p) = &meta.isbn { write_kv(&mut s, "isbn", p); }
    if let Some(p) = &meta.language { write_kv(&mut s, "language", p); }
    write_kv(&mut s, "source_file", &meta.source_file);
    s.push_str("chapters:\n");
    for ch in chapters {
        s.push_str(&format!("  - title: {}\n", yaml_inline_string(ch.title)));
        s.push_str(&format!("    line: {}\n", pad_number(ch.offset.line, NUMERIC_WIDTH, &ch.title)?));
        s.push_str(&format!("    byte: {}\n", pad_number(ch.offset.byte, NUMERIC_WIDTH, &ch.title)?));
    }
    s.push_str("---\n");
    Ok(s)
}

fn write_kv(s: &mut String, key: &str, value: &str) {
    s.push_str(&format!("{key}: {}\n", yaml_inline_string(value)));
}

fn yaml_inline_string(v: &str) -> String {
    let needs_quote = v.is_empty()
        || v.contains(':')
        || v.contains('#')
        || v.contains('\'')
        || v.contains('"')
        || v.contains('\n')
        || v.starts_with(' ')
        || v.ends_with(' ');
    if !needs_quote {
        v.to_string()
    } else {
        format!("\"{}\"", v.replace('\\', r"\\").replace('"', r#"\""#))
    }
}

fn pad_number(n: u64, width: usize, chapter: &str) -> Result<String, crate::Error> {
    let s = n.to_string();
    if s.len() > width {
        return Err(crate::Error::OffsetOverflow { chapter: chapter.to_string(), value: n });
    }
    Ok(format!("{:>width$}", n, width = width))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load::Metadata;

    fn meta() -> Metadata {
        Metadata {
            title: "Test Book".into(),
            authors: vec!["A. Author".into()],
            publisher: None,
            published: Some("2024".into()),
            isbn: None,
            language: Some("en".into()),
            source_file: "x.epub".into(),
        }
    }

    #[test]
    fn padded_byte_field_has_constant_width() {
        let m = meta();
        let c1 = FrontmatterChapter { title: "Intro", offset: ChapterOffset { byte: 0, line: 1 } };
        let c2 = FrontmatterChapter { title: "Two", offset: ChapterOffset { byte: 9_999_999, line: 99_999 } };
        let s1 = render(&m, &[c1]).unwrap();
        let s2 = render(&m, &[c2]).unwrap();
        // Both files have one chapter line; both should have the same line length
        // for the `line:` and `byte:` fields.
        let line_field_1 = s1.lines().find(|l| l.trim_start().starts_with("line:")).unwrap();
        let line_field_2 = s2.lines().find(|l| l.trim_start().starts_with("line:")).unwrap();
        assert_eq!(line_field_1.len(), line_field_2.len(), "line field width changed");
    }

    #[test]
    fn padded_value_parses_as_int() {
        // Demonstrate that leading whitespace within a YAML scalar is ignored
        // by typical parsers. We don't depend on a YAML lib at runtime, but the
        // value itself should still trim cleanly to its digits.
        let m = meta();
        let c = FrontmatterChapter { title: "X", offset: ChapterOffset { byte: 42, line: 7 } };
        let s = render(&m, &[c]).unwrap();
        let byte_line = s.lines().find(|l| l.trim_start().starts_with("byte:")).unwrap();
        let value_part = byte_line.split_once(':').unwrap().1.trim();
        assert_eq!(value_part.parse::<u64>().unwrap(), 42);
    }

    #[test]
    fn overflow_errors() {
        let m = meta();
        let c = FrontmatterChapter { title: "huge", offset: ChapterOffset { byte: 99_999_999_999, line: 1 } };
        assert!(render(&m, &[c]).is_err());
    }

    #[test]
    fn shape_smoke() {
        let m = meta();
        let c = FrontmatterChapter { title: "Hello", offset: ChapterOffset { byte: 0, line: 1 } };
        let s = render(&m, &[c]).unwrap();
        assert!(s.starts_with("---\n"));
        assert!(s.ends_with("---\n"));
        assert!(s.contains("title: Test Book\n"));
        assert!(s.contains("authors: [A. Author]\n"));
    }
}
```

- [ ] **Step 2: Add module to `lib.rs`** and run tests.

```rust
pub mod assemble;
pub mod block;
pub mod extract;
pub mod frontmatter;
pub mod images;
pub mod load;
pub mod render;
pub mod slug;
mod cli;
mod error;
```

Run: `cargo test frontmatter::tests`
Expected: 4 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add src/frontmatter.rs src/lib.rs
git commit -m "Frontmatter emission with leading-padded fixed-width offsets"
```

---

## Task 22: Write — pipeline orchestration

This is the integrating module. It owns the load → extract → assemble → render → frontmatter → write sequence and produces the file system output.

**Files:**
- Create: `src/write.rs`
- Modify: `src/lib.rs` to add `mod write;` and a public `convert(args: &cli::Args) -> Result<()>`.
- Modify: `src/lib.rs` `run_from_args` to call `convert`.

- [ ] **Step 1: Implement and test in one block — write the full driver and an integration test**

Create `src/write.rs`:

```rust
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
use std::path::{Path, PathBuf};

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
    let mut path_to_chapter: BTreeMap<String, usize> = BTreeMap::new();
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
        path_to_chapter.insert(doc.manifest_path.clone(), n);
        chapters.push(Chapter { number: n, title, source_path: doc.manifest_path.clone(), blocks });
    }
    rewrite_internal_links(&mut chapters, &path_to_chapter);

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
            None => continue, // not referenced anywhere — skip safely
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
    // Pass through external URLs unchanged.
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
    use crate::block::Inline;
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
```

- [ ] **Step 2: Wire into `lib.rs`**

Replace `src/lib.rs`:

```rust
//! books-for-bots: convert EPUBs to YAML-headed markdown with chapter offsets.

pub use error::Error;

pub mod assemble;
pub mod block;
pub mod cli;
pub mod extract;
pub mod frontmatter;
pub mod images;
pub mod load;
pub mod render;
pub mod slug;
pub mod write;
mod error;

pub type Result<T> = std::result::Result<T, Error>;

pub fn run_from_args() -> Result<()> {
    use clap::Parser;
    let args = cli::Args::parse();
    write::convert(&args)
}
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean compile.

- [ ] **Step 4: End-to-end smoke test**

Create `tests/integration.rs`:

```rust
mod common;

use books_for_bots::{cli::Args, write};
use std::path::PathBuf;

#[test]
fn end_to_end_minimal_book() {
    let fx = common::build_minimal_book(
        "Smoke Test",
        "Tester",
        &[
            common::ChapterSpec { title: "Intro", html: "<p>First paragraph.</p>" },
            common::ChapterSpec { title: "Body",  html: "<p>Second paragraph.</p>" },
        ],
    );
    let tmp = tempfile::tempdir().unwrap();
    let in_path = tmp.path().join("smoke.epub");
    std::fs::write(&in_path, &fx.bytes).unwrap();
    let out = tmp.path().join("out");

    let args = Args { input: in_path, output_dir: out.clone(), force: false };
    write::convert(&args).expect("convert");

    let book_md = std::fs::read_to_string(out.join("smoke-test-tester/book.md")).unwrap();
    assert!(book_md.starts_with("---\n"));
    assert!(book_md.contains("title: Smoke Test\n"));
    assert!(book_md.contains("## Intro"));
    assert!(book_md.contains("## Body"));
    assert!(book_md.contains("First paragraph."));
}
```

Run: `cargo test --test integration`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/write.rs src/lib.rs tests/integration.rs
git commit -m "Pipeline orchestration: load through write, end-to-end test"
```

---

## Task 23: Determinism test

**Files:**
- Create: `tests/determinism.rs`

- [ ] **Step 1: Write the test**

```rust
mod common;

use books_for_bots::{cli::Args, write};

#[test]
fn two_runs_produce_identical_output() {
    let fx = common::build_minimal_book(
        "Det",
        "T",
        &[
            common::ChapterSpec { title: "A", html: "<p>x</p><p>y</p>" },
            common::ChapterSpec { title: "B", html: "<ul><li>1</li><li>2</li></ul>" },
        ],
    );
    let tmp = tempfile::tempdir().unwrap();
    let in_path = tmp.path().join("d.epub");
    std::fs::write(&in_path, &fx.bytes).unwrap();

    let run = |sub: &str| {
        let out = tmp.path().join(sub);
        let args = Args { input: in_path.clone(), output_dir: out.clone(), force: false };
        write::convert(&args).expect("convert");
        std::fs::read(out.join("det-t/book.md")).unwrap()
    };

    let a = run("out_a");
    let b = run("out_b");
    assert_eq!(a, b, "outputs differ between runs");
}
```

- [ ] **Step 2: Run**

Run: `cargo test --test determinism`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/determinism.rs
git commit -m "Determinism test: two runs are byte-identical"
```

---

## Task 24: Offset verification test

**Files:**
- Create: `tests/offsets.rs`

- [ ] **Step 1: Write the test**

```rust
mod common;

use books_for_bots::{cli::Args, write};
use std::io::{Read, Seek, SeekFrom};

#[test]
fn chapter_offsets_seek_to_correct_heading() {
    let fx = common::build_minimal_book(
        "Off",
        "T",
        &[
            common::ChapterSpec { title: "First", html: "<p>aaa</p>" },
            common::ChapterSpec { title: "Second", html: "<p>bbb bbb bbb bbb</p>" },
            common::ChapterSpec { title: "Third", html: "<p>ccc</p>" },
        ],
    );
    let tmp = tempfile::tempdir().unwrap();
    let in_path = tmp.path().join("off.epub");
    std::fs::write(&in_path, &fx.bytes).unwrap();
    let out = tmp.path().join("out");
    write::convert(&Args { input: in_path, output_dir: out.clone(), force: false }).unwrap();

    let path = out.join("off-t/book.md");
    let s = std::fs::read_to_string(&path).unwrap();

    // Parse chapter offsets without a YAML lib: scan lines.
    let mut titles = Vec::new();
    let mut bytes = Vec::new();
    let mut lines_n = Vec::new();
    let mut in_chapters = false;
    let mut current_title: Option<String> = None;
    for line in s.lines() {
        if line == "chapters:" { in_chapters = true; continue; }
        if !in_chapters { continue; }
        if line == "---" { break; }
        if let Some(rest) = line.strip_prefix("  - title: ") {
            current_title = Some(rest.trim_matches('"').to_string());
        } else if let Some(rest) = line.strip_prefix("    line:") {
            lines_n.push(rest.trim().parse::<u64>().unwrap());
        } else if let Some(rest) = line.strip_prefix("    byte:") {
            bytes.push(rest.trim().parse::<u64>().unwrap());
            titles.push(current_title.take().unwrap());
        }
    }

    // For each chapter, seek to byte and read enough characters to find "## <title>".
    let mut f = std::fs::File::open(&path).unwrap();
    for (i, byte) in bytes.iter().enumerate() {
        f.seek(SeekFrom::Start(*byte)).unwrap();
        let mut buf = vec![0u8; 200];
        let n = f.read(&mut buf).unwrap();
        let s = std::str::from_utf8(&buf[..n]).unwrap();
        let expected_prefix = format!("## {}", titles[i]);
        assert!(s.starts_with(&expected_prefix),
            "byte offset {} for chapter {:?} does not start with {:?}; got: {:?}",
            byte, titles[i], expected_prefix, &s[..s.len().min(80)]);
    }

    // Verify line offsets too.
    let lines: Vec<&str> = std::fs::read_to_string(&path).unwrap().lines().collect::<Vec<_>>().leak();
    for (i, &n) in lines_n.iter().enumerate() {
        let expected_prefix = format!("## {}", titles[i]);
        let actual = lines.get((n as usize).saturating_sub(1)).copied().unwrap_or("");
        assert!(actual.starts_with(&expected_prefix),
            "line {} for chapter {:?} is {:?}", n, titles[i], actual);
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo test --test offsets`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/offsets.rs
git commit -m "Offset verification: byte and line offsets seek to chapter headings"
```

---

## Task 25: Final wiring and CLI smoke

**Files:**
- Modify: `src/main.rs` (no changes expected; verify works)

- [ ] **Step 1: Build release**

Run: `cargo build --release`
Expected: clean.

- [ ] **Step 2: Manual CLI smoke test**

```bash
# Should print usage:
./target/release/books-for-bots --help

# Should error cleanly on missing file:
./target/release/books-for-bots /nonexistent.epub; echo "exit=$?"
```

Expected: usage string from clap. Exit code 1 on missing file with message starting `books-for-bots: not a valid EPUB`.

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: all unit and integration tests PASS.

- [ ] **Step 4: Commit only if anything changed**

```bash
git status
# If nothing changed, this task is complete.
```

---

## Task 26: README

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write a minimal README**

```markdown
# books-for-bots

Convert an EPUB into a single YAML-headed Markdown file optimized for token-efficient reading by LLM agents.

## What it produces

```
output/<slug>/book.md
output/<slug>/images/<basename>
```

`book.md` opens with a YAML frontmatter that lists every chapter with its absolute byte and line offsets in the file. The body is plain GFM Markdown. No HTML cleanup that requires judgment, no reflow, no surprises — just a deterministic structural translation of the book.

## Install

```sh
cargo install --path .
```

## Use

```sh
books-for-bots my-book.epub --output-dir output
```

## Why offsets in the frontmatter?

So an agent can `Read book.md --offset N --limit M` and seek directly to the chapter it wants, without parsing the whole file or walking a directory tree.

## Design

See [`docs/specs/2026-05-01-design.md`](docs/specs/2026-05-01-design.md).
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "Add README"
```

---

## Final verification

Run the complete suite once more:

```bash
cargo test --release
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

If all pass, push:

```bash
git push origin main
```
