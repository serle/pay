use std::io;
use thiserror::Error;

use crate::domain::DomainError;

/// Storage-level errors
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Entity not found")]
    NotFound,

    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    #[error("Domain error: {0}")]
    DomainError(#[from] DomainError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_formats_correctly() {
        assert_eq!(StorageError::NotFound.to_string(), "Entity not found");

        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let storage_err = StorageError::from(io_err);
        assert!(storage_err.to_string().contains("I/O error"));
    }

    #[test]
    fn domain_error_conversion() {
        let domain_err = DomainError::InsufficientFunds;
        let storage_err = StorageError::from(domain_err);

        match storage_err {
            StorageError::DomainError(DomainError::InsufficientFunds) => {}
            _ => panic!("Expected DomainError variant"),
        }
    }

    #[test]
    fn io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let storage_err = StorageError::from(io_err);

        match storage_err {
            StorageError::IoError(_) => {}
            _ => panic!("Expected IoError variant"),
        }
    }
}
