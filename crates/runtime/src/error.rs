//! TODO: Implement

use thiserror::Error;

/// Placeholder Error type
#[derive(Error, Debug)]
pub enum Error {
    #[error("Not implemented")]
    NotImplemented,
}
