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
