use async_trait::async_trait;
use tokio::io::AsyncWrite;

use super::error::StorageError;
use crate::domain::{AmountType, ClientAccount, DomainError, TransactionRecord};

/// Trait for managing transaction records (for dispute resolution)
/// Transactions are immutable once inserted
pub trait TransactionStoreManager<A: AmountType>: Send + Sync {
    /// Insert a transaction record (immutable after insertion)
    fn insert(&mut self, tx_id: u32, record: TransactionRecord<A>);

    /// Get a transaction record by ID (returns clone, not reference)
    fn get(&self, tx_id: u32) -> Option<TransactionRecord<A>>;

    /// Check if a transaction exists
    fn contains(&self, tx_id: u32) -> bool;
}

/// Trait for managing client accounts with pluggable storage backends
#[async_trait]
pub trait ClientAccountManager<A: AmountType>: Send + Sync {
    type Entry<'a>: ClientAccountEntry<'a, A>
    where
        Self: 'a;

    /// Get or create an entry for the given client ID
    fn entry(&self, client_id: u16) -> Result<Self::Entry<'_>, StorageError>;

    /// Read-only access to an account
    fn get(&self, client_id: u16) -> Result<Option<&ClientAccount<A>>, StorageError>;

    /// Async snapshot of all accounts to a writer
    async fn snapshot<W>(&self, writer: W) -> Result<(), StorageError>
    where
        W: AsyncWrite + Unpin + Send;

    /// Iterate over all accounts
    fn iter(&self) -> Box<dyn Iterator<Item = &ClientAccount<A>> + Send + '_>;
}

/// Entry pattern for atomic account operations
pub trait ClientAccountEntry<'a, A: AmountType> {
    /// Non-locking read (clones the account data)
    fn read(&self) -> ClientAccount<A>;

    /// Atomic read-modify-write with validation
    fn try_update<F>(&mut self, update_fn: F) -> Result<(), StorageError>
    where
        F: FnOnce(&mut ClientAccount<A>) -> Result<(), DomainError>;
}
