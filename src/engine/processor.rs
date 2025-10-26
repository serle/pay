use std::marker::PhantomData;
use tracing::{debug, warn};

use super::error::EngineError;
use crate::domain::{
    AmountType, Transaction, TransactionRecord, apply_chargeback, apply_deposit, apply_dispute,
    apply_resolve, apply_withdrawal,
};
use crate::storage::{ClientAccountEntry, ClientAccountManager, TransactionStoreManager};

/// Transaction processor orchestrating domain operations and storage
pub struct TransactionProcessor<A, M, T>
where
    A: AmountType,
    M: ClientAccountManager<A>,
    T: TransactionStoreManager<A>,
{
    account_manager: M,
    transaction_store: T,
    _phantom: PhantomData<A>,
}

impl<A, M, T> TransactionProcessor<A, M, T>
where
    A: AmountType,
    M: ClientAccountManager<A>,
    T: TransactionStoreManager<A>,
{
    /// Create a new transaction processor
    pub fn new(account_manager: M, transaction_store: T) -> Self {
        Self {
            account_manager,
            transaction_store,
            _phantom: PhantomData,
        }
    }

    /// Process a single transaction
    pub fn process_transaction(&mut self, tx: Transaction<A>) -> Result<(), EngineError> {
        match tx {
            Transaction::Deposit {
                client_id,
                tx_id,
                amount,
            } => self.process_deposit(client_id, tx_id, amount),
            Transaction::Withdrawal {
                client_id,
                tx_id,
                amount,
            } => self.process_withdrawal(client_id, tx_id, amount),
            Transaction::Dispute { client_id, tx_id } => self.process_dispute(client_id, tx_id),
            Transaction::Resolve { client_id, tx_id } => self.process_resolve(client_id, tx_id),
            Transaction::Chargeback { client_id, tx_id } => {
                self.process_chargeback(client_id, tx_id)
            }
        }
    }

    /// Get reference to account manager for snapshot operations
    pub fn account_manager(&self) -> &M {
        &self.account_manager
    }

    fn process_deposit(
        &mut self,
        client_id: u16,
        tx_id: u32,
        amount: A,
    ) -> Result<(), EngineError> {
        debug!(client_id, tx_id, "Processing deposit");

        // Apply deposit to account
        let mut entry = self.account_manager.entry(client_id)?;
        entry.try_update(|account| apply_deposit(account, amount))?;

        // Record transaction for potential disputes
        self.transaction_store
            .insert(tx_id, TransactionRecord::new(client_id, amount));

        Ok(())
    }

    fn process_withdrawal(
        &mut self,
        client_id: u16,
        tx_id: u32,
        amount: A,
    ) -> Result<(), EngineError> {
        debug!(client_id, tx_id, "Processing withdrawal");

        // Apply withdrawal to account
        let mut entry = self.account_manager.entry(client_id)?;
        entry.try_update(|account| apply_withdrawal(account, amount))?;

        // Record transaction (withdrawals cannot be disputed, but track for completeness)
        self.transaction_store
            .insert(tx_id, TransactionRecord::new(client_id, amount));

        Ok(())
    }

    fn process_dispute(&mut self, client_id: u16, tx_id: u32) -> Result<(), EngineError> {
        debug!(client_id, tx_id, "Processing dispute");

        // Look up the original transaction
        let record = self
            .transaction_store
            .get(tx_id)
            .ok_or(EngineError::TransactionNotFound(tx_id))?;

        // Verify transaction belongs to this client
        if record.client_id != client_id {
            warn!(
                client_id,
                tx_id,
                record_client_id = record.client_id,
                "Dispute client mismatch"
            );
            return Err(EngineError::TransactionNotFound(tx_id));
        }

        let amount = record.amount;

        // Apply dispute to account (move funds to held + track dispute)
        let mut entry = self.account_manager.entry(client_id)?;
        entry.try_update(|account| apply_dispute(account, tx_id, amount))?;

        Ok(())
    }

    fn process_resolve(&mut self, client_id: u16, tx_id: u32) -> Result<(), EngineError> {
        debug!(client_id, tx_id, "Processing resolve");

        // Look up the original transaction
        let record = self
            .transaction_store
            .get(tx_id)
            .ok_or(EngineError::TransactionNotFound(tx_id))?;

        // Verify transaction belongs to this client
        if record.client_id != client_id {
            warn!(
                client_id,
                tx_id,
                record_client_id = record.client_id,
                "Resolve client mismatch"
            );
            return Err(EngineError::TransactionNotFound(tx_id));
        }

        let amount = record.amount;

        // Apply resolve to account (move funds from held to available + remove dispute)
        let mut entry = self.account_manager.entry(client_id)?;
        entry.try_update(|account| apply_resolve(account, tx_id, amount))?;

        Ok(())
    }

    fn process_chargeback(&mut self, client_id: u16, tx_id: u32) -> Result<(), EngineError> {
        debug!(client_id, tx_id, "Processing chargeback");

        // Look up the original transaction
        let record = self
            .transaction_store
            .get(tx_id)
            .ok_or(EngineError::TransactionNotFound(tx_id))?;

        // Verify transaction belongs to this client
        if record.client_id != client_id {
            warn!(
                client_id,
                tx_id,
                record_client_id = record.client_id,
                "Chargeback client mismatch"
            );
            return Err(EngineError::TransactionNotFound(tx_id));
        }

        let amount = record.amount;

        // Apply chargeback to account (remove held funds, lock, and remove dispute)
        let mut entry = self.account_manager.entry(client_id)?;
        entry.try_update(|account| apply_chargeback(account, tx_id, amount))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DomainError, FixedPoint};
    use crate::storage::{ClientAccountEntry, ConcurrentAccountManager, ConcurrentTransactionStore, StorageError};

    #[test]
    fn process_deposit_creates_account_and_credits() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let mut processor = TransactionProcessor::new(manager, store);

        let tx = Transaction::Deposit {
            client_id: 1,
            tx_id: 1,
            amount: FixedPoint::from_raw(10_000),
        };

        processor.process_transaction(tx).unwrap();

        let entry = processor.account_manager.entry(1).unwrap();
        let account = entry.read();
        assert_eq!(account.available(), FixedPoint::from_raw(10_000));
    }

    #[test]
    fn process_withdrawal_debits_account() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let mut processor = TransactionProcessor::new(manager, store);

        // Deposit first
        processor
            .process_transaction(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            })
            .unwrap();

        // Withdraw
        processor
            .process_transaction(Transaction::Withdrawal {
                client_id: 1,
                tx_id: 2,
                amount: FixedPoint::from_raw(3_000),
            })
            .unwrap();

        let entry = processor.account_manager.entry(1).unwrap();
        let account = entry.read();
        assert_eq!(account.available(), FixedPoint::from_raw(7_000));
    }

    #[test]
    fn process_withdrawal_insufficient_funds_fails() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let mut processor = TransactionProcessor::new(manager, store);

        processor
            .process_transaction(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(1_000),
            })
            .unwrap();

        let result = processor.process_transaction(Transaction::Withdrawal {
            client_id: 1,
            tx_id: 2,
            amount: FixedPoint::from_raw(2_000),
        });

        assert!(result.is_err());

        // Account unchanged
        let entry = processor.account_manager.entry(1).unwrap();
        let account = entry.read();
        assert_eq!(account.available(), FixedPoint::from_raw(1_000));
    }

    #[test]
    fn dispute_requires_existing_transaction() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let mut processor = TransactionProcessor::new(manager, store);

        let result = processor.process_transaction(Transaction::Dispute {
            client_id: 1,
            tx_id: 999,
        });

        assert!(matches!(result, Err(EngineError::TransactionNotFound(999))));
    }

    #[test]
    fn dispute_marks_transaction_as_disputed() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let mut processor = TransactionProcessor::new(manager, store);

        // Deposit
        processor
            .process_transaction(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            })
            .unwrap();

        // Dispute
        processor
            .process_transaction(Transaction::Dispute {
                client_id: 1,
                tx_id: 1,
            })
            .unwrap();

        // Check account state and dispute tracking
        let entry = processor.account_manager.entry(1).unwrap();
        let account = entry.read();
        assert_eq!(account.available(), FixedPoint::zero());
        assert_eq!(account.held(), FixedPoint::from_raw(10_000));
        assert_eq!(account.total(), FixedPoint::from_raw(10_000));
        assert!(account.is_disputed(1)); // Dispute tracked in account
    }

    #[test]
    fn cannot_dispute_twice() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let mut processor = TransactionProcessor::new(manager, store);

        processor
            .process_transaction(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            })
            .unwrap();

        processor
            .process_transaction(Transaction::Dispute {
                client_id: 1,
                tx_id: 1,
            })
            .unwrap();

        let result = processor.process_transaction(Transaction::Dispute {
            client_id: 1,
            tx_id: 1,
        });

        assert!(matches!(
            result,
            Err(EngineError::Storage(StorageError::DomainError(DomainError::AlreadyDisputed)))
        ));
    }

    #[test]
    fn resolve_releases_held_funds() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let mut processor = TransactionProcessor::new(manager, store);

        // Deposit and dispute
        processor
            .process_transaction(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            })
            .unwrap();

        processor
            .process_transaction(Transaction::Dispute {
                client_id: 1,
                tx_id: 1,
            })
            .unwrap();

        // Resolve
        processor
            .process_transaction(Transaction::Resolve {
                client_id: 1,
                tx_id: 1,
            })
            .unwrap();

        let entry = processor.account_manager.entry(1).unwrap();
        let account = entry.read();
        assert_eq!(account.available(), FixedPoint::from_raw(10_000));
        assert_eq!(account.held(), FixedPoint::zero());
        assert_eq!(account.total(), FixedPoint::from_raw(10_000));
        assert!(!account.is_disputed(1)); // Dispute resolved, tracked in account
    }

    #[test]
    fn resolve_requires_disputed_transaction() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let mut processor = TransactionProcessor::new(manager, store);

        processor
            .process_transaction(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            })
            .unwrap();

        let result = processor.process_transaction(Transaction::Resolve {
            client_id: 1,
            tx_id: 1,
        });

        assert!(matches!(
            result,
            Err(EngineError::Storage(StorageError::DomainError(DomainError::NotDisputed)))
        ));
    }

    #[test]
    fn chargeback_removes_held_and_locks_account() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let mut processor = TransactionProcessor::new(manager, store);

        // Deposit and dispute
        processor
            .process_transaction(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            })
            .unwrap();

        processor
            .process_transaction(Transaction::Dispute {
                client_id: 1,
                tx_id: 1,
            })
            .unwrap();

        // Chargeback
        processor
            .process_transaction(Transaction::Chargeback {
                client_id: 1,
                tx_id: 1,
            })
            .unwrap();

        let entry = processor.account_manager.entry(1).unwrap();
        let account = entry.read();
        assert_eq!(account.available(), FixedPoint::zero());
        assert_eq!(account.held(), FixedPoint::zero());
        assert_eq!(account.total(), FixedPoint::zero());
        assert!(account.is_locked());
    }

    #[test]
    fn chargeback_requires_disputed_transaction() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let mut processor = TransactionProcessor::new(manager, store);

        processor
            .process_transaction(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            })
            .unwrap();

        let result = processor.process_transaction(Transaction::Chargeback {
            client_id: 1,
            tx_id: 1,
        });

        assert!(matches!(
            result,
            Err(EngineError::Storage(StorageError::DomainError(DomainError::NotDisputed)))
        ));
    }

    #[test]
    fn operations_on_locked_account_fail() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let mut processor = TransactionProcessor::new(manager, store);

        // Deposit, dispute, chargeback to lock account
        processor
            .process_transaction(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            })
            .unwrap();

        processor
            .process_transaction(Transaction::Dispute {
                client_id: 1,
                tx_id: 1,
            })
            .unwrap();

        processor
            .process_transaction(Transaction::Chargeback {
                client_id: 1,
                tx_id: 1,
            })
            .unwrap();

        // Try to deposit to locked account
        let result = processor.process_transaction(Transaction::Deposit {
            client_id: 1,
            tx_id: 2,
            amount: FixedPoint::from_raw(5_000),
        });

        assert!(result.is_err());
    }

    #[test]
    fn full_dispute_resolve_cycle() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let mut processor = TransactionProcessor::new(manager, store);

        // Initial deposit
        processor
            .process_transaction(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            })
            .unwrap();

        let entry = processor.account_manager.entry(1).unwrap();
        assert_eq!(entry.read().total(), FixedPoint::from_raw(10_000));

        // Dispute
        processor
            .process_transaction(Transaction::Dispute {
                client_id: 1,
                tx_id: 1,
            })
            .unwrap();

        let entry = processor.account_manager.entry(1).unwrap();
        let account = entry.read();
        assert_eq!(account.available(), FixedPoint::zero());
        assert_eq!(account.held(), FixedPoint::from_raw(10_000));
        assert_eq!(account.total(), FixedPoint::from_raw(10_000));

        // Resolve
        processor
            .process_transaction(Transaction::Resolve {
                client_id: 1,
                tx_id: 1,
            })
            .unwrap();

        let entry = processor.account_manager.entry(1).unwrap();
        let account = entry.read();
        assert_eq!(account.available(), FixedPoint::from_raw(10_000));
        assert_eq!(account.held(), FixedPoint::zero());
        assert_eq!(account.total(), FixedPoint::from_raw(10_000));
    }

    #[test]
    fn full_dispute_chargeback_cycle() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let mut processor = TransactionProcessor::new(manager, store);

        // Initial deposit
        processor
            .process_transaction(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            })
            .unwrap();

        // Dispute
        processor
            .process_transaction(Transaction::Dispute {
                client_id: 1,
                tx_id: 1,
            })
            .unwrap();

        let entry = processor.account_manager.entry(1).unwrap();
        assert_eq!(entry.read().total(), FixedPoint::from_raw(10_000));

        // Chargeback
        processor
            .process_transaction(Transaction::Chargeback {
                client_id: 1,
                tx_id: 1,
            })
            .unwrap();

        let entry = processor.account_manager.entry(1).unwrap();
        let account = entry.read();
        assert_eq!(account.total(), FixedPoint::zero());
        assert!(account.is_locked());
    }

    #[test]
    fn dispute_client_mismatch_fails() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let mut processor = TransactionProcessor::new(manager, store);

        // Client 1 deposits
        processor
            .process_transaction(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            })
            .unwrap();

        // Client 2 tries to dispute client 1's transaction
        let result = processor.process_transaction(Transaction::Dispute {
            client_id: 2,
            tx_id: 1,
        });

        assert!(matches!(result, Err(EngineError::TransactionNotFound(1))));
    }
}
