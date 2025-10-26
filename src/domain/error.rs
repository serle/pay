use thiserror::Error;

/// Domain-level errors representing business rule violations
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("Insufficient funds for withdrawal")]
    InsufficientFunds,

    #[error("Account is locked")]
    AccountLocked,

    #[error("Invalid amount")]
    InvalidAmount,

    #[error("Arithmetic overflow")]
    Overflow,

    #[error("Transaction is already disputed")]
    AlreadyDisputed,

    #[error("Transaction is not disputed")]
    NotDisputed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_formats_correctly() {
        assert_eq!(
            DomainError::InsufficientFunds.to_string(),
            "Insufficient funds for withdrawal"
        );
        assert_eq!(DomainError::AccountLocked.to_string(), "Account is locked");
        assert_eq!(DomainError::InvalidAmount.to_string(), "Invalid amount");
        assert_eq!(DomainError::Overflow.to_string(), "Arithmetic overflow");
        assert_eq!(
            DomainError::AlreadyDisputed.to_string(),
            "Transaction is already disputed"
        );
        assert_eq!(
            DomainError::NotDisputed.to_string(),
            "Transaction is not disputed"
        );
    }

    #[test]
    fn error_is_cloneable() {
        let err = DomainError::InsufficientFunds;
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn error_comparison_works() {
        assert_eq!(
            DomainError::InsufficientFunds,
            DomainError::InsufficientFunds
        );
        assert_ne!(DomainError::InsufficientFunds, DomainError::AccountLocked);
    }
}
