use std::io;
use thiserror::Error;

use crate::domain::DomainError;
use crate::engine::EngineError;
use crate::io::IoError;
use crate::storage::StorageError;

/// Top-level application errors unifying all layer errors
#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("CSV IO error: {0}")]
    CsvIo(#[from] IoError),

    #[error("Engine error: {0}")]
    Engine(#[from] EngineError),

    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_formats_correctly() {
        assert_eq!(
            AppError::FileNotFound("input.csv".to_string()).to_string(),
            "File not found: input.csv"
        );
        assert_eq!(
            AppError::InvalidArguments("missing file".to_string()).to_string(),
            "Invalid arguments: missing file"
        );
    }

    #[test]
    fn io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let app_err = AppError::from(io_err);

        match app_err {
            AppError::Io(_) => {}
            _ => panic!("Expected Io error variant"),
        }
    }

    #[test]
    fn domain_error_conversion() {
        let domain_err = DomainError::InsufficientFunds;
        let app_err = AppError::from(domain_err);

        match app_err {
            AppError::Domain(DomainError::InsufficientFunds) => {}
            _ => panic!("Expected Domain error variant"),
        }
    }

    #[test]
    fn engine_error_conversion() {
        let engine_err = EngineError::TransactionNotFound(123);
        let app_err = AppError::from(engine_err);

        match app_err {
            AppError::Engine(EngineError::TransactionNotFound(123)) => {}
            _ => panic!("Expected Engine error variant"),
        }
    }
}
