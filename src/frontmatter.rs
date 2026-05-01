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
        let line_field_1 = s1.lines().find(|l| l.trim_start().starts_with("line:")).unwrap();
        let line_field_2 = s2.lines().find(|l| l.trim_start().starts_with("line:")).unwrap();
        assert_eq!(line_field_1.len(), line_field_2.len(), "line field width changed");
    }

    #[test]
    fn padded_value_parses_as_int() {
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
