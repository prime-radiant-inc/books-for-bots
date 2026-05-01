use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("not implemented yet")]
    NotImplemented,
}
