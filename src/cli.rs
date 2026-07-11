use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "books-for-bots",
    version,
    about = "Convert an EPUB to YAML-headed markdown"
)]
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
    fn version_flag_prints_crate_version() {
        let err = Args::try_parse_from(["books-for-bots", "--version"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
        assert!(err.to_string().contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn overrides() {
        let a = Args::try_parse_from([
            "books-for-bots",
            "book.epub",
            "--output-dir",
            "out",
            "--force",
        ])
        .unwrap();
        assert_eq!(a.output_dir.to_str().unwrap(), "out");
        assert!(a.force);
    }
}
