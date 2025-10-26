use std::io;
use thiserror::Error;

use crate::domain::DomainError;
use crate::storage::StorageError;

/// IO-level errors for CSV parsing and stream processing
#[derive(Error, Debug)]
pub enum IoError {
    #[error("CSV parsing error: {0}")]
    Csv(#[from] csv::Error),

    #[error("CSV async parsing error: {0}")]
    CsvAsync(#[from] csv_async::Error),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid transaction type: {0}")]
    InvalidTransactionType(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid amount format: {0}")]
    InvalidAmount(String),

    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),

    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_formats_correctly() {
        assert_eq!(
            IoError::InvalidTransactionType("foo".to_string()).to_string(),
            "Invalid transaction type: foo"
        );
        assert_eq!(
            IoError::MissingField("amount".to_string()).to_string(),
            "Missing required field: amount"
        );
        assert_eq!(
            IoError::InvalidAmount("xyz".to_string()).to_string(),
            "Invalid amount format: xyz"
        );
    }

    #[test]
    fn domain_error_conversion() {
        let domain_err = DomainError::InsufficientFunds;
        let io_err = IoError::from(domain_err);

        match io_err {
            IoError::Domain(DomainError::InsufficientFunds) => {}
            _ => panic!("Expected Domain error variant"),
        }
    }

    #[test]
    fn io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let wrapped = IoError::from(io_err);

        match wrapped {
            IoError::Io(_) => {}
            _ => panic!("Expected Io error variant"),
        }
    }
}
