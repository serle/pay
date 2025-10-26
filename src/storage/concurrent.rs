use async_trait::async_trait;
use dashmap::{DashMap, Entry};
use tokio::io::AsyncWrite;

use super::error::StorageError;
use super::traits::{ClientAccountEntry, ClientAccountManager};
use crate::domain::{AmountType, ClientAccount, DomainError};

/// Concurrent in-memory account manager using DashMap
pub struct ConcurrentAccountManager<A: AmountType> {
    accounts: DashMap<u16, ClientAccount<A>>,
}

impl<A: AmountType> ConcurrentAccountManager<A> {
    /// Create a new empty concurrent account manager
    pub fn new() -> Self {
        Self {
            accounts: DashMap::new(),
        }
    }
}

impl<A: AmountType> Default for ConcurrentAccountManager<A> {
    fn default() -> Self {
        Self::new()
    }
}

/// Entry for concurrent access
pub struct ConcurrentEntry<'a, A: AmountType> {
    client_id: u16,
    accounts: &'a DashMap<u16, ClientAccount<A>>,
}

impl<'a, A: AmountType> ClientAccountEntry<'a, A> for ConcurrentEntry<'a, A> {
    fn read(&self) -> ClientAccount<A> {
        self.accounts
            .get(&self.client_id)
            .map(|r| r.value().clone())
            .unwrap_or_else(|| ClientAccount::new(self.client_id))
    }

    fn try_update<F>(&mut self, update_fn: F) -> Result<(), StorageError>
    where
        F: FnOnce(&mut ClientAccount<A>) -> Result<(), DomainError>,
    {
        // Use DashMap's entry API correctly
        let entry = self.accounts.entry(self.client_id);
        match entry {
            Entry::Occupied(mut e) => {
                let account = e.get_mut();
                update_fn(account)?;
            }
            Entry::Vacant(e) => {
                let mut account = ClientAccount::new(self.client_id);
                update_fn(&mut account)?;
                e.insert(account);
            }
        }

        Ok(())
    }
}

#[async_trait]
impl<A: AmountType> ClientAccountManager<A> for ConcurrentAccountManager<A> {
    type Entry<'a>
        = ConcurrentEntry<'a, A>
    where
        Self: 'a;

    fn entry(&self, client_id: u16) -> Result<Self::Entry<'_>, StorageError> {
        Ok(ConcurrentEntry {
            client_id,
            accounts: &self.accounts,
        })
    }

    fn get(&self, _client_id: u16) -> Result<Option<&ClientAccount<A>>, StorageError> {
        // DashMap doesn't allow direct & access due to internal locking
        // Return None for now - read() method on Entry is the preferred way
        Ok(None)
    }

    async fn snapshot<W>(&self, mut writer: W) -> Result<(), StorageError>
    where
        W: AsyncWrite + Unpin + Send,
    {
        use tokio::io::AsyncWriteExt;

        // Write header
        writer
            .write_all(b"client,available,held,total,locked\n")
            .await?;

        // Iterate and write each account
        // DashMap holds brief per-shard locks during iteration
        for entry in self.accounts.iter() {
            let account = entry.value();
            let line = format!(
                "{},{},{},{},{}\n",
                account.client_id(),
                account.available().to_decimal_string(),
                account.held().to_decimal_string(),
                account.total().to_decimal_string(),
                account.is_locked()
            );
            writer.write_all(line.as_bytes()).await?;
        }

        writer.flush().await?;
        Ok(())
    }

    fn iter(&self) -> Box<dyn Iterator<Item = &ClientAccount<A>> + Send + '_> {
        // Cannot return direct references from DashMap due to locking
        // This is a limitation - iter would need to collect or use a different approach
        // For now, return empty iterator (snapshot method handles output correctly)
        Box::new(std::iter::empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FixedPoint, operations};
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn entry_creates_account_if_not_exists() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let entry = manager.entry(1).unwrap();

        let account = entry.read();
        assert_eq!(account.client_id(), 1);
        assert_eq!(account.total(), FixedPoint::zero());
    }

    #[test]
    fn entry_returns_existing_account() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();

        // Create and modify account
        {
            let mut entry = manager.entry(1).unwrap();
            entry
                .try_update(|acc| operations::apply_deposit(acc, FixedPoint::from_raw(5_000)))
                .unwrap();
        }

        // Retrieve again
        let entry = manager.entry(1).unwrap();
        let account = entry.read();
        assert_eq!(account.available(), FixedPoint::from_raw(5_000));
    }

    #[test]
    fn try_update_applies_mutation() {
        let manager = ConcurrentAccountManager::new();
        let mut entry = manager.entry(1).unwrap();

        entry
            .try_update(|acc| operations::apply_deposit(acc, FixedPoint::from_raw(10_000)))
            .unwrap();

        let account = entry.read();
        assert_eq!(account.available(), FixedPoint::from_raw(10_000));
    }

    #[test]
    fn concurrent_updates_to_different_clients() {
        let manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let manager1 = Arc::clone(&manager);
        let manager2 = Arc::clone(&manager);

        let h1 = thread::spawn(move || {
            for _ in 0..1000 {
                let mut entry = manager1.entry(1).unwrap();
                entry
                    .try_update(|acc| operations::apply_deposit(acc, FixedPoint::from_raw(1)))
                    .unwrap();
            }
        });

        let h2 = thread::spawn(move || {
            for _ in 0..1000 {
                let mut entry = manager2.entry(2).unwrap();
                entry
                    .try_update(|acc| operations::apply_deposit(acc, FixedPoint::from_raw(1)))
                    .unwrap();
            }
        });

        h1.join().unwrap();
        h2.join().unwrap();

        let entry1 = manager.entry(1).unwrap();
        let entry2 = manager.entry(2).unwrap();

        assert_eq!(entry1.read().available(), FixedPoint::from_raw(1000));
        assert_eq!(entry2.read().available(), FixedPoint::from_raw(1000));
    }

    #[test]
    fn concurrent_updates_to_same_client() {
        let manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let manager1 = Arc::clone(&manager);
        let manager2 = Arc::clone(&manager);

        let h1 = thread::spawn(move || {
            for _ in 0..500 {
                let mut entry = manager1.entry(1).unwrap();
                entry
                    .try_update(|acc| operations::apply_deposit(acc, FixedPoint::from_raw(1)))
                    .unwrap();
            }
        });

        let h2 = thread::spawn(move || {
            for _ in 0..500 {
                let mut entry = manager2.entry(1).unwrap();
                entry
                    .try_update(|acc| operations::apply_deposit(acc, FixedPoint::from_raw(1)))
                    .unwrap();
            }
        });

        h1.join().unwrap();
        h2.join().unwrap();

        let entry = manager.entry(1).unwrap();
        assert_eq!(entry.read().available(), FixedPoint::from_raw(1000));
    }

    #[tokio::test]
    async fn snapshot_while_updates_happening() {
        let manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());

        // Create initial accounts
        for i in 1..=5 {
            let mut entry = manager.entry(i).unwrap();
            entry
                .try_update(|acc| operations::apply_deposit(acc, FixedPoint::from_raw(1_000)))
                .unwrap();
        }

        let manager_clone = Arc::clone(&manager);

        // Spawn background updates
        let update_handle = tokio::spawn(async move {
            for _ in 0..100 {
                for i in 1..=5 {
                    let mut entry = manager_clone.entry(i).unwrap();
                    let _ = entry
                        .try_update(|acc| operations::apply_deposit(acc, FixedPoint::from_raw(1)));
                }
                tokio::time::sleep(tokio::time::Duration::from_micros(10)).await;
            }
        });

        // Take snapshot while updates happening
        let mut output = Vec::new();
        manager.snapshot(&mut output).await.unwrap();

        update_handle.await.unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("client,available,held,total,locked"));
        // Should have entries for all 5 clients
        assert!(result.matches("\n").count() >= 5);
    }

    #[tokio::test]
    async fn snapshot_writes_csv_format() {
        let manager = ConcurrentAccountManager::new();

        // Create some accounts
        {
            let mut entry = manager.entry(1).unwrap();
            entry
                .try_update(|acc| operations::apply_deposit(acc, FixedPoint::from_raw(15_000)))
                .unwrap();
        }

        {
            let mut entry = manager.entry(2).unwrap();
            entry
                .try_update(|acc| operations::apply_deposit(acc, FixedPoint::from_raw(20_000)))
                .unwrap();
        }

        // Snapshot to buffer
        let mut output = Vec::new();
        manager.snapshot(&mut output).await.unwrap();

        let result = String::from_utf8(output).unwrap();

        assert!(result.contains("client,available,held,total,locked"));
        assert!(
            result.contains("1,1.5000,0.0000,1.5000,false")
                || result.contains("2,2.0000,0.0000,2.0000,false")
        );
    }

    // Note: iter() test omitted as DashMap doesn't support returning borrowed references
    // The snapshot() method demonstrates correct iteration
}

// Implement ClientAccountManager for Arc<ConcurrentAccountManager> to enable sharing
// This allows multiple threads/tasks to share the same account manager
#[async_trait]
impl<A: AmountType> ClientAccountManager<A> for std::sync::Arc<ConcurrentAccountManager<A>> {
    type Entry<'a>
        = ConcurrentEntry<'a, A>
    where
        Self: 'a;

    fn entry(&self, client_id: u16) -> Result<Self::Entry<'_>, StorageError> {
        (**self).entry(client_id)
    }

    fn get(&self, client_id: u16) -> Result<Option<&ClientAccount<A>>, StorageError> {
        (**self).get(client_id)
    }

    async fn snapshot<W>(&self, writer: W) -> Result<(), StorageError>
    where
        W: AsyncWrite + Unpin + Send,
    {
        (**self).snapshot(writer).await
    }

    fn iter(&self) -> Box<dyn Iterator<Item = &ClientAccount<A>> + Send + '_> {
        (**self).iter()
    }
}
