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
    assert_eq!(book.spine[1].toc_title.as_deref(), Some("Two"));
    assert!(book.spine[1].html.contains("Second chapter"));
    assert!(book.cover_image.is_none(), "synthetic fixture has no cover");
}
