//! books-for-bots: convert EPUBs to YAML-headed markdown with chapter offsets.

pub use error::Error;

mod error;

pub type Result<T> = std::result::Result<T, Error>;

/// Top-level entry point used by the binary.
pub fn run_from_args() -> Result<()> {
    Err(Error::NotImplemented)
}
