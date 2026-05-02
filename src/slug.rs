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
