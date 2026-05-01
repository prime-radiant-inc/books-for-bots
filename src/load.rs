//! Typed wrapper over the `epub` crate.
//!
//! Loads an EPUB file and returns a [`Book`] with typed metadata, a linear
//! spine of HTML documents, and all image resources extracted into a map.

use crate::Result;
use epub::doc::EpubDoc;
use std::collections::BTreeMap;
use std::path::Path;

/// All data extracted from an EPUB file.
pub struct Book {
    pub metadata: Metadata,
    pub spine: Vec<SpineDoc>,
    /// Manifest path → bytes for every image-typed resource.
    pub images: BTreeMap<String, Vec<u8>>,
    /// Manifest path of the cover image, if declared.
    pub cover_image: Option<String>,
}

/// Bibliographic metadata extracted from the OPF package document.
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

/// One document in the spine.
pub struct SpineDoc {
    /// Manifest path (relative to the OPF directory).
    pub manifest_path: String,
    /// Resolved title from the navigation document, if any.
    pub toc_title: Option<String>,
    /// Raw (UTF-8) HTML body of the spine document.
    pub html: String,
}

/// Opens the EPUB at `path` and returns a fully-loaded [`Book`].
///
/// # Errors
///
/// Returns [`crate::Error::InvalidEpub`] if the file cannot be opened or
/// parsed as a valid EPUB, or [`crate::Error::EpubStructure`] if required
/// structural elements are missing.
pub fn open(path: &Path) -> Result<Book> {
    let mut doc = EpubDoc::new(path).map_err(|e| {
        crate::Error::InvalidEpub(format!("{}: {e}", path.display()))
    })?;

    let source_file = path.display().to_string();

    // --- Metadata ---
    let title = doc
        .mdata("title")
        .map(|m| m.value.clone())
        .unwrap_or_default();

    let authors = doc
        .metadata
        .iter()
        .filter(|m| m.property == "creator")
        .map(|m| m.value.clone())
        .collect::<Vec<_>>();

    let publisher = doc.mdata("publisher").map(|m| m.value.clone());
    let published = doc.mdata("date").map(|m| m.value.clone());
    let language = doc.mdata("language").map(|m| m.value.clone());

    // ISBN lives in an <dc:identifier> whose scheme refinement is "ISBN"
    let isbn = doc
        .metadata
        .iter()
        .find(|m| {
            m.property == "identifier"
                && m.refined
                    .iter()
                    .any(|r| r.property == "scheme" && r.value.eq_ignore_ascii_case("ISBN"))
        })
        .map(|m| m.value.clone());

    let metadata = Metadata {
        title,
        authors,
        publisher,
        published,
        isbn,
        language,
        source_file,
    };

    // --- Cover image ---
    // get_cover_id() gives us the resource *id*; we need the manifest path.
    let cover_image = doc.get_cover_id().and_then(|id| {
        doc.resources
            .get(&id)
            .map(|r| r.path.to_string_lossy().into_owned())
    });

    // --- Build TOC lookup: canonical path string → label ---
    // NavPoint.content is an absolute PathBuf (root_base-prefixed).
    // We normalise to the same string form used by ResourceItem.path.
    let toc_map: BTreeMap<String, String> = doc
        .toc
        .iter()
        .map(|nav| {
            let key = nav.content.to_string_lossy().into_owned();
            (key, nav.label.clone())
        })
        .collect();

    // --- Spine documents ---
    let spine_len = doc.spine.len();
    let mut spine = Vec::with_capacity(spine_len);

    for i in 0..spine_len {
        doc.set_current_chapter(i);

        let manifest_path = doc
            .get_current_path()
            .map(|p| p.to_string_lossy().into_owned())
            .ok_or_else(|| {
                crate::Error::EpubStructure(format!("spine item {i} has no path"))
            })?;

        let html = doc
            .get_current_str()
            .map(|(s, _mime)| s)
            .ok_or_else(|| {
                crate::Error::EpubStructure(format!(
                    "spine item {i} ({manifest_path}) could not be read"
                ))
            })?;

        let toc_title = toc_map.get(&manifest_path).cloned();

        spine.push(SpineDoc {
            manifest_path,
            toc_title,
            html,
        });
    }

    // --- Images ---
    let image_paths: Vec<std::path::PathBuf> = doc
        .resources
        .values()
        .filter(|r| r.mime.starts_with("image/"))
        .map(|r| r.path.clone())
        .collect();

    let mut images = BTreeMap::new();
    for path in &image_paths {
        let key = path.to_string_lossy().into_owned();
        if let Some(bytes) = doc.get_resource_by_path(path) {
            images.insert(key, bytes);
        }
    }

    Ok(Book {
        metadata,
        spine,
        images,
        cover_image,
    })
}

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
