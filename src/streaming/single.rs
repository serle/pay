use std::marker::PhantomData;
use futures::{Stream, StreamExt};

use super::error::ErrorPolicy;
use crate::domain::{AmountType, Transaction};
use crate::engine::TransactionProcessor;
use crate::storage::{ClientAccountManager, TransactionStoreManager};

/// Single stream processing session
pub struct ProcessingSession<A, M, T, P>
where
    A: AmountType,
    M: ClientAccountManager<A>,
    T: TransactionStoreManager<A>,
    P: ErrorPolicy,
{
    processor: TransactionProcessor<A, M, T>,
    error_policy: P,
    _phantom: PhantomData<A>,
}

impl<A, M, T, P> ProcessingSession<A, M, T, P>
where
    A: AmountType,
    M: ClientAccountManager<A>,
    T: TransactionStoreManager<A>,
    P: ErrorPolicy,
{
    /// Create a new processing session
    pub fn new(processor: TransactionProcessor<A, M, T>, error_policy: P) -> Self {
        Self {
            processor,
            error_policy,
            _phantom: PhantomData,
        }
    }

    /// Process a stream of transactions
    /// Returns true if all transactions were processed successfully (or skipped per policy)
    /// Returns false if processing was aborted due to error policy
    pub async fn process_stream<S>(&mut self, mut stream: S) -> bool
    where
        S: Stream<Item = Result<Transaction<A>, crate::io::IoError>> + Unpin,
    {
        while let Some(result) = stream.next().await {
            match result {
                Ok(transaction) => {
                    if let Err(e) = self.processor.process_transaction(transaction)
                        && !self.error_policy.handle_engine_error(e)
                    {
                        return false;
                    }
                }
                Err(e) => {
                    if !self.error_policy.handle_io_error(e) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Get a reference to the underlying account manager
    pub fn account_manager(&self) -> &M {
        self.processor.account_manager()
    }

    /// Consume the session and return the processor
    pub fn into_processor(self) -> TransactionProcessor<A, M, T> {
        self.processor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::FixedPoint;
    use crate::storage::{ClientAccountEntry, ConcurrentAccountManager, ConcurrentTransactionStore};
    use crate::streaming::error::{AbortOnError, SilentSkip, SkipErrors};
    use futures::stream;

    #[tokio::test]
    async fn processes_valid_transactions() {
        let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let processor = TransactionProcessor::new(account_manager, store);
        let mut session = ProcessingSession::new(processor, SilentSkip);

        let transactions = vec![
            Ok(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            }),
            Ok(Transaction::Deposit {
                client_id: 2,
                tx_id: 2,
                amount: FixedPoint::from_raw(20_000),
            }),
        ];

        let tx_stream = stream::iter(transactions);
        let success = session.process_stream(tx_stream).await;

        assert!(success);

        // Verify accounts were updated
        let entry1 = session.account_manager().entry(1).unwrap();
        assert_eq!(entry1.read().available(), FixedPoint::from_raw(10_000));

        let entry2 = session.account_manager().entry(2).unwrap();
        assert_eq!(entry2.read().available(), FixedPoint::from_raw(20_000));
    }

    #[tokio::test]
    async fn skip_errors_continues_on_io_error() {
        let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let processor = TransactionProcessor::new(account_manager, store);
        let mut session = ProcessingSession::new(processor, SkipErrors);

        let transactions = vec![
            Ok(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            }),
            Err(crate::io::IoError::InvalidTransactionType(
                "invalid".to_string(),
            )),
            Ok(Transaction::Deposit {
                client_id: 2,
                tx_id: 2,
                amount: FixedPoint::from_raw(20_000),
            }),
        ];

        let tx_stream = stream::iter(transactions);
        let success = session.process_stream(tx_stream).await;

        assert!(success);

        // Verify valid transactions were processed
        let entry1 = session.account_manager().entry(1).unwrap();
        assert_eq!(entry1.read().available(), FixedPoint::from_raw(10_000));

        let entry2 = session.account_manager().entry(2).unwrap();
        assert_eq!(entry2.read().available(), FixedPoint::from_raw(20_000));
    }

    #[tokio::test]
    async fn abort_on_error_stops_on_io_error() {
        let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let processor = TransactionProcessor::new(account_manager, store);
        let mut session = ProcessingSession::new(processor, AbortOnError);

        let transactions = vec![
            Ok(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            }),
            Err(crate::io::IoError::InvalidTransactionType(
                "invalid".to_string(),
            )),
            Ok(Transaction::Deposit {
                client_id: 2,
                tx_id: 2,
                amount: FixedPoint::from_raw(20_000),
            }),
        ];

        let tx_stream = stream::iter(transactions);
        let success = session.process_stream(tx_stream).await;

        assert!(!success);

        // First transaction should be processed
        let entry1 = session.account_manager().entry(1).unwrap();
        assert_eq!(entry1.read().available(), FixedPoint::from_raw(10_000));

        // Second transaction should NOT be processed (after error)
        let entry2 = session.account_manager().entry(2).unwrap();
        assert_eq!(entry2.read().available(), FixedPoint::zero());
    }

    #[tokio::test]
    async fn skip_errors_continues_on_engine_error() {
        let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let processor = TransactionProcessor::new(account_manager, store);
        let mut session = ProcessingSession::new(processor, SkipErrors);

        let transactions = vec![
            Ok(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            }),
            // Try to withdraw more than available (will fail)
            Ok(Transaction::Withdrawal {
                client_id: 1,
                tx_id: 2,
                amount: FixedPoint::from_raw(20_000),
            }),
            Ok(Transaction::Deposit {
                client_id: 2,
                tx_id: 3,
                amount: FixedPoint::from_raw(5_000),
            }),
        ];

        let tx_stream = stream::iter(transactions);
        let success = session.process_stream(tx_stream).await;

        assert!(success);

        // First deposit should succeed
        let entry1 = session.account_manager().entry(1).unwrap();
        assert_eq!(entry1.read().available(), FixedPoint::from_raw(10_000));

        // Third deposit should succeed despite second transaction failing
        let entry2 = session.account_manager().entry(2).unwrap();
        assert_eq!(entry2.read().available(), FixedPoint::from_raw(5_000));
    }

    #[tokio::test]
    async fn abort_on_error_stops_on_engine_error() {
        let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let processor = TransactionProcessor::new(account_manager, store);
        let mut session = ProcessingSession::new(processor, AbortOnError);

        let transactions = vec![
            Ok(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            }),
            // Try to withdraw more than available (will fail)
            Ok(Transaction::Withdrawal {
                client_id: 1,
                tx_id: 2,
                amount: FixedPoint::from_raw(20_000),
            }),
            Ok(Transaction::Deposit {
                client_id: 2,
                tx_id: 3,
                amount: FixedPoint::from_raw(5_000),
            }),
        ];

        let tx_stream = stream::iter(transactions);
        let success = session.process_stream(tx_stream).await;

        assert!(!success);

        // First deposit should succeed
        let entry1 = session.account_manager().entry(1).unwrap();
        assert_eq!(entry1.read().available(), FixedPoint::from_raw(10_000));

        // Third deposit should NOT be processed (aborted after engine error)
        let entry2 = session.account_manager().entry(2).unwrap();
        assert_eq!(entry2.read().available(), FixedPoint::zero());
    }

    #[tokio::test]
    async fn processes_empty_stream() {
        let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let processor = TransactionProcessor::new(account_manager, store);
        let mut session = ProcessingSession::new(processor, SilentSkip);

        let transactions: Vec<Result<Transaction<FixedPoint>, crate::io::IoError>> = vec![];
        let tx_stream = stream::iter(transactions);
        let success = session.process_stream(tx_stream).await;

        assert!(success);
    }

    #[tokio::test]
    async fn into_processor_returns_processor() {
        let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
        let store = ConcurrentTransactionStore::new();
        let processor = TransactionProcessor::new(account_manager, store);
        let session = ProcessingSession::new(processor, SilentSkip);

        let _processor = session.into_processor();
        // Test passes if this compiles and runs
    }
}
