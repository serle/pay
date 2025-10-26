pub mod account;
pub mod amount;
pub mod error;
pub mod operations;
pub mod transaction;

// Re-export commonly used types
pub use account::ClientAccount;
pub use amount::{AmountType, FixedPoint};
pub use error::DomainError;
pub use operations::{
    apply_chargeback, apply_deposit, apply_dispute, apply_resolve, apply_withdrawal,
};
pub use transaction::{Transaction, TransactionRecord};
