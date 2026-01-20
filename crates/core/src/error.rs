//! TODO: Implement

use thiserror::Error;

/// Placeholder Error type
#[derive(Error, Debug)]
pub enum Error {
    /// Feature not yet implemented
    #[error("Not implemented")]
    NotImplemented,
}
