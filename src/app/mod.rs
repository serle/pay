pub mod cli;
pub mod error;

// Re-export commonly used types
pub use cli::{CliApp, SnapshotWriter};
pub use error::AppError;
