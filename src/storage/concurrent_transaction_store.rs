use dashmap::DashMap;

use crate::domain::{AmountType, TransactionRecord};
use super::traits::TransactionStoreManager;

/// DashMap-based concurrent transaction store (lock-free, thread-safe)
/// Transactions are immutable once inserted
pub struct ConcurrentTransactionStore<A: AmountType> {
    records: DashMap<u32, TransactionRecord<A>>,
}

impl<A: AmountType> ConcurrentTransactionStore<A> {
    /// Create a new empty concurrent transaction store
    pub fn new() -> Self {
        Self {
            records: DashMap::new(),
        }
    }
}

impl<A: AmountType> TransactionStoreManager<A> for ConcurrentTransactionStore<A> {
    fn insert(&mut self, tx_id: u32, record: TransactionRecord<A>) {
        self.records.insert(tx_id, record);
    }

    fn get(&self, tx_id: u32) -> Option<TransactionRecord<A>> {
        self.records.get(&tx_id).map(|r| r.clone())
    }

    fn contains(&self, tx_id: u32) -> bool {
        self.records.contains_key(&tx_id)
    }
}

impl<A: AmountType> Default for ConcurrentTransactionStore<A> {
    fn default() -> Self {
        Self::new()
    }
}

// Implement TransactionStoreManager for Arc<ConcurrentTransactionStore> to enable sharing
// This allows multiple threads/tasks to share the same transaction store
impl<A: AmountType> TransactionStoreManager<A> for std::sync::Arc<ConcurrentTransactionStore<A>> {
    fn insert(&mut self, tx_id: u32, record: TransactionRecord<A>) {
        // Arc provides interior mutability via DashMap, so we can insert through &self
        // We just need to get a reference to the inner store
        self.records.insert(tx_id, record);
    }

    fn get(&self, tx_id: u32) -> Option<TransactionRecord<A>> {
        (**self).get(tx_id)
    }

    fn contains(&self, tx_id: u32) -> bool {
        (**self).contains(tx_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::FixedPoint;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn new_store_is_empty() {
        let store = ConcurrentTransactionStore::<FixedPoint>::new();
        assert!(!store.contains(1));
        assert!(store.get(1).is_none());
    }

    #[test]
    fn insert_and_retrieve_record() {
        let mut store = ConcurrentTransactionStore::new();
        let record = TransactionRecord::new(1, FixedPoint::from_raw(10_000));

        store.insert(100, record.clone());

        assert!(store.contains(100));
        let retrieved = store.get(100).unwrap();
        assert_eq!(retrieved.client_id, 1);
        assert_eq!(retrieved.amount, FixedPoint::from_raw(10_000));
    }

    #[test]
    fn get_returns_none_for_nonexistent() {
        let store = ConcurrentTransactionStore::<FixedPoint>::new();
        assert!(store.get(999).is_none());
        assert!(!store.contains(999));
    }

    #[test]
    fn get_returns_clone_not_reference() {
        let mut store = ConcurrentTransactionStore::new();
        let record = TransactionRecord::new(1, FixedPoint::from_raw(1000));
        store.insert(1, record.clone());

        let retrieved1 = store.get(1).unwrap();
        let retrieved2 = store.get(1).unwrap();

        // Both are clones, equal in value
        assert_eq!(retrieved1, retrieved2);
        assert_eq!(retrieved1.client_id, 1);
    }

    #[test]
    fn multiple_transactions() {
        let mut store = ConcurrentTransactionStore::new();

        store.insert(1, TransactionRecord::new(1, FixedPoint::from_raw(1_000)));
        store.insert(2, TransactionRecord::new(2, FixedPoint::from_raw(2_000)));
        store.insert(3, TransactionRecord::new(1, FixedPoint::from_raw(3_000)));

        assert!(store.contains(1));
        assert!(store.contains(2));
        assert!(store.contains(3));

        assert_eq!(store.get(1).unwrap().client_id, 1);
        assert_eq!(store.get(2).unwrap().client_id, 2);
        assert_eq!(store.get(3).unwrap().client_id, 1);
    }

    #[test]
    fn concurrent_access_from_multiple_threads() {
        let mut store = ConcurrentTransactionStore::<FixedPoint>::new();

        // Pre-populate some transactions
        for i in 0..100 {
            store.insert(i, TransactionRecord::new(1, FixedPoint::from_raw(1000)));
        }

        let store = Arc::new(store);

        // Spawn 10 threads, each reading 100 transactions
        let handles: Vec<_> = (0..10)
            .map(|_thread_id| {
                let store_clone = Arc::clone(&store);
                thread::spawn(move || {
                    for i in 0..100 {
                        assert!(store_clone.contains(i));
                        let record = store_clone.get(i).unwrap();
                        assert_eq!(record.client_id, 1);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all 100 transactions still exist
        for i in 0..100 {
            assert!(store.contains(i));
        }
    }

    #[test]
    fn concurrent_writes() {
        // Note: We can't easily test concurrent writes with the current API
        // since insert requires &mut self. In a real concurrent scenario,
        // DashMap allows concurrent writes, but the trait signature requires &mut.
        // This is acceptable since typically one processor owns the store during
        // a processing session, and multiple processors would have separate stores
        // or access via Arc<RwLock<>> if needed.

        let mut store = ConcurrentTransactionStore::<FixedPoint>::new();

        // Sequential writes work fine
        for i in 0..1000 {
            store.insert(i, TransactionRecord::new((i % 10) as u16, FixedPoint::from_raw(i as i64 * 1000)));
        }

        assert_eq!(store.records.len(), 1000);
    }

    #[test]
    fn immutability_transactions_cannot_be_modified() {
        let mut store = ConcurrentTransactionStore::new();
        let record = TransactionRecord::new(1, FixedPoint::from_raw(1000));
        store.insert(1, record);

        // Get returns a clone, not a mutable reference
        let _retrieved = store.get(1).unwrap();
        // No get_mut method available - transactions are immutable!

        // Original record unchanged
        assert_eq!(store.get(1).unwrap().amount, FixedPoint::from_raw(1000));
    }
}
