use thiserror::Error;

use crate::domain::DomainError;
use crate::storage::StorageError;

/// Engine-level errors for transaction processing
#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Transaction not found: {0}")]
    TransactionNotFound(u32),

    #[error("Transaction not under dispute: {0}")]
    TransactionNotDisputed(u32),

    #[error("Transaction already disputed: {0}")]
    TransactionAlreadyDisputed(u32),

    #[error("Cannot dispute a withdrawal")]
    CannotDisputeWithdrawal,

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
            EngineError::TransactionNotFound(123).to_string(),
            "Transaction not found: 123"
        );
        assert_eq!(
            EngineError::TransactionNotDisputed(456).to_string(),
            "Transaction not under dispute: 456"
        );
        assert_eq!(
            EngineError::TransactionAlreadyDisputed(789).to_string(),
            "Transaction already disputed: 789"
        );
        assert_eq!(
            EngineError::CannotDisputeWithdrawal.to_string(),
            "Cannot dispute a withdrawal"
        );
    }

    #[test]
    fn domain_error_conversion() {
        let domain_err = DomainError::InsufficientFunds;
        let engine_err = EngineError::from(domain_err);

        match engine_err {
            EngineError::Domain(DomainError::InsufficientFunds) => {}
            _ => panic!("Expected Domain error variant"),
        }
    }

    #[test]
    fn storage_error_conversion() {
        let storage_err = StorageError::NotFound;
        let engine_err = EngineError::from(storage_err);

        match engine_err {
            EngineError::Storage(StorageError::NotFound) => {}
            _ => panic!("Expected Storage error variant"),
        }
    }
}
