//! books-for-bots: convert EPUBs to YAML-headed markdown with chapter offsets.

pub use error::Error;

pub mod block;
pub mod slug;
mod cli;
mod error;

pub type Result<T> = std::result::Result<T, Error>;

pub fn run_from_args() -> Result<()> {
    use clap::Parser;
    let _args = cli::Args::parse();
    Err(Error::NotImplemented)
}
