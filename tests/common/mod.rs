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
