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
        std::fs::read(out.join("det-t/det-t.md")).unwrap()
    };

    let a = run("out_a");
    let b = run("out_b");
    assert_eq!(a, b, "outputs differ between runs");
}
