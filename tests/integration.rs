mod common;

use books_for_bots::{cli::Args, write};

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

    let book_md = std::fs::read_to_string(out.join("smoke-test-tester/smoke-test-tester.md")).unwrap();
    assert!(book_md.starts_with("---\n"));
    assert!(book_md.contains("title: Smoke Test\n"));
    assert!(book_md.contains("## Intro"));
    assert!(book_md.contains("## Body"));
    assert!(book_md.contains("First paragraph."));
}
