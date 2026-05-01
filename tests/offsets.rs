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

    let path = out.join("off-t/off-t.md");
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

    assert_eq!(titles.len(), 3, "expected 3 chapters in frontmatter");

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
    let all = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = all.lines().collect();
    for (i, &n) in lines_n.iter().enumerate() {
        let expected_prefix = format!("## {}", titles[i]);
        let actual = lines.get((n as usize).saturating_sub(1)).copied().unwrap_or("");
        assert!(actual.starts_with(&expected_prefix),
            "line {} for chapter {:?} is {:?}", n, titles[i], actual);
    }
}
