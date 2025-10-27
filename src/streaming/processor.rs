use std::marker::PhantomData;
use std::pin::Pin;

use futures::{Stream, StreamExt};
use futures::stream;

#[cfg(test)]
use std::sync::Arc;

use super::error::ErrorPolicy;
use crate::domain::{AmountType, Transaction};
use crate::engine::TransactionProcessor;
use crate::io::IoError;
use crate::storage::{ClientAccountManager, TransactionStoreManager};

/// Type alias for a boxed transaction stream
type TransactionStream<A> = Pin<Box<dyn Stream<Item = Result<Transaction<A>, IoError>> + Send>>;

/// Primary API for processing transaction streams
///
/// Supports single-stream and multi-stream topologies with configurable
/// parallelism, sharding, and stream combining strategies.
pub struct StreamProcessor<A, M, T, P>
where
    A: AmountType,
    M: ClientAccountManager<A> + Clone + Send + Sync + 'static,
    T: TransactionStoreManager<A> + Clone + Send + Sync + 'static,
    P: ErrorPolicy + Clone + Send + 'static,
{
    account_manager: M,
    transaction_store: T,
    error_policy: P,
    num_shards: usize,
    streams: Vec<TransactionStream<A>>,
    shard_assignment: ShardAssignment,
    stream_combinator: StreamCombinator,
    _phantom: PhantomData<A>,
}

/// How to assign streams to shards
pub enum ShardAssignment {
    /// Distribute streams round-robin across shards (default)
    /// Stream 0→Shard 0, Stream 1→Shard 1, ..., Stream N→Shard 0, ...
    RoundRobin,

    /// Assign streams sequentially to shards
    /// First N/S streams→Shard 0, next N/S→Shard 1, ...
    Sequential,

    /// Custom assignment function: stream_index -> shard_index
    Custom(Box<dyn Fn(usize) -> usize + Send + Sync>),
}

/// How to combine multiple streams within a single shard
#[derive(Debug, Clone, Copy)]
pub enum StreamCombinator {
    /// Merge streams concurrently (interleaved) - DEFAULT
    /// Good for: Independent streams, maximize I/O throughput
    Merge,

    /// Chain streams sequentially (one after another)
    /// Good for: Order-dependent streams within a shard
    Chain,
}

impl<A, M, T, P> StreamProcessor<A, M, T, P>
where
    A: AmountType + 'static,
    M: ClientAccountManager<A> + Clone + Send + Sync + 'static,
    T: TransactionStoreManager<A> + Clone + Send + Sync + 'static,
    P: ErrorPolicy + Clone + Send + 'static,
{
    /// Create a new stream processor with shared storage
    ///
    /// # Arguments
    /// * `account_manager` - Shared account manager (typically Arc<ConcurrentAccountManager>)
    /// * `transaction_store` - Shared transaction store (typically Arc<ConcurrentTransactionStore>)
    /// * `error_policy` - Error handling policy
    ///
    /// # Example
    /// ```rust,ignore
    /// let mgr = Arc::new(ConcurrentAccountManager::new());
    /// let store = Arc::new(ConcurrentTransactionStore::new());
    ///
    /// let processor = StreamProcessor::new(mgr, store, SilentSkip);
    /// ```
    pub fn new(
        account_manager: M,
        transaction_store: T,
        error_policy: P,
    ) -> Self {
        Self {
            account_manager,
            transaction_store,
            error_policy,
            num_shards: 1,
            streams: Vec::new(),
            shard_assignment: ShardAssignment::RoundRobin,
            stream_combinator: StreamCombinator::Merge,
            _phantom: PhantomData,
        }
    }

    /// Set number of parallel shards/processors (defaults to 1)
    ///
    /// Each shard runs in its own tokio task. The number should typically
    /// match or be less than available CPU cores.
    ///
    /// # Example
    /// ```rust,ignore
    /// // Single-threaded processing
    /// processor.with_shards(1)
    ///
    /// // Parallel processing across 4 shards
    /// processor.with_shards(4)
    ///
    /// // Use all CPU cores
    /// processor.with_shards(num_cpus::get())
    /// ```
    pub fn with_shards(mut self, num: usize) -> Self {
        self.num_shards = num.max(1);
        self
    }

    /// Set how to assign streams to shards (defaults to RoundRobin)
    ///
    /// # Examples
    /// ```rust,ignore
    /// // Round-robin: stream_0→shard_0, stream_1→shard_1, stream_2→shard_0, ...
    /// processor.with_shard_assignment(ShardAssignment::RoundRobin)
    ///
    /// // Sequential: first 3 streams→shard_0, next 3→shard_1, ...
    /// processor.with_shard_assignment(ShardAssignment::Sequential)
    ///
    /// // Custom logic
    /// processor.with_shard_assignment(ShardAssignment::Custom(
    ///     Box::new(|idx| idx % 2)  // Even streams to shard 0, odd to shard 1
    /// ))
    /// ```
    pub fn with_shard_assignment(mut self, assignment: ShardAssignment) -> Self {
        self.shard_assignment = assignment;
        self
    }

    /// Set how to combine multiple streams within a shard (defaults to Merge)
    ///
    /// # Examples
    /// ```rust,ignore
    /// // Merge: Streams in same shard processed concurrently (interleaved)
    /// processor.with_stream_combinator(StreamCombinator::Merge)
    ///
    /// // Chain: Streams in same shard processed sequentially
    /// processor.with_stream_combinator(StreamCombinator::Chain)
    /// ```
    pub fn with_stream_combinator(mut self, combinator: StreamCombinator) -> Self {
        self.stream_combinator = combinator;
        self
    }

    /// Add a stream to process (fluent interface)
    ///
    /// Stream will be assigned to a shard based on the shard assignment strategy.
    /// If multiple streams are assigned to the same shard, they will be combined
    /// according to the stream combining strategy.
    ///
    /// # Example
    /// ```rust,ignore
    /// StreamProcessor::new(mgr, store, SilentSkip)
    ///     .add_stream(csv_stream_1)
    ///     .add_stream(csv_stream_2)
    ///     .add_stream(csv_stream_3)
    ///     .process()
    ///     .await;
    /// ```
    pub fn add_stream<S>(mut self, stream: S) -> Self
    where
        S: Stream<Item = Result<Transaction<A>, IoError>> + Send + 'static,
    {
        self.streams.push(Box::pin(stream));
        self
    }

    /// Process all streams across parallel shards
    ///
    /// 1. Assigns streams to shards based on shard assignment strategy
    /// 2. Combines streams within each shard based on stream combining strategy
    /// 3. Spawns one task per shard
    /// 4. Each task processes its combined stream
    ///
    /// # Returns
    /// ProcessorResults containing per-shard results and overall success status
    ///
    /// # Example
    /// ```rust,ignore
    /// let results = StreamProcessor::new(mgr, store, SilentSkip)
    ///     .with_shards(4)
    ///     .add_stream(stream1)
    ///     .add_stream(stream2)
    ///     .process()
    ///     .await;
    ///
    /// if results.all_succeeded() {
    ///     println!("All shards processed successfully");
    /// }
    /// ```
    pub async fn process(self) -> ProcessorResults {
        let num_streams = self.streams.len();

        if num_streams == 0 {
            return ProcessorResults {
                shard_results: vec![],
                total_streams: 0,
            };
        }

        // Destructure self to get ownership of all fields
        let StreamProcessor {
            account_manager,
            transaction_store,
            error_policy,
            num_shards,
            streams,
            shard_assignment,
            stream_combinator,
            _phantom,
        } = self;

        // Assign streams to shards
        let mut shards: Vec<Vec<_>> = (0..num_shards).map(|_| Vec::new()).collect();
        let total_streams = streams.len();

        for (stream_idx, stream) in streams.into_iter().enumerate() {
            let shard_idx = match &shard_assignment {
                ShardAssignment::RoundRobin => stream_idx % num_shards,
                ShardAssignment::Sequential => {
                    let chunk_size = total_streams.div_ceil(num_shards);
                    (stream_idx / chunk_size).min(num_shards - 1)
                }
                ShardAssignment::Custom(f) => f(stream_idx) % num_shards,
            };

            shards[shard_idx].push(stream);
        }

        // Spawn one task per shard
        let handles: Vec<_> = shards
            .into_iter()
            .enumerate()
            .map(|(shard_id, shard_streams)| {
                let mgr = account_manager.clone();
                let store = transaction_store.clone();
                let policy = error_policy.clone();
                let combinator = stream_combinator;

                tokio::spawn(async move {
                    if shard_streams.is_empty() {
                        return ShardResult {
                            shard_id,
                            streams_processed: 0,
                            success: true,
                        };
                    }

                    let stream_count = shard_streams.len();

                    // Combine streams within this shard
                    let combined = match combinator {
                        StreamCombinator::Merge => {
                            // Merge streams concurrently
                            Box::pin(stream::select_all(shard_streams))
                                as Pin<Box<dyn Stream<Item = _> + Send>>
                        }
                        StreamCombinator::Chain => {
                            // Chain streams sequentially
                            Box::pin(stream::iter(shard_streams).flatten())
                                as Pin<Box<dyn Stream<Item = _> + Send>>
                        }
                    };

                    // Process the combined stream
                    let processor = TransactionProcessor::new(mgr, store);
                    let success = Self::process_shard_stream(combined, processor, policy).await;

                    ShardResult {
                        shard_id,
                        streams_processed: stream_count,
                        success,
                    }
                })
            })
            .collect();

        // Await all tasks
        let mut shard_results = Vec::new();
        for handle in handles {
            shard_results.push(handle.await.unwrap_or(ShardResult {
                shard_id: 0,
                streams_processed: 0,
                success: false,
            }));
        }

        ProcessorResults {
            shard_results,
            total_streams: num_streams,
        }
    }

    /// Process a single shard's stream
    async fn process_shard_stream<S>(
        mut stream: S,
        mut processor: TransactionProcessor<A, M, T>,
        policy: P,
    ) -> bool
    where
        S: Stream<Item = Result<Transaction<A>, IoError>> + Unpin,
    {
        while let Some(result) = stream.next().await {
            match result {
                Ok(transaction) => {
                    if let Err(e) = processor.process_transaction(transaction)
                        && !policy.handle_engine_error(e) {
                        return false;
                    }
                }
                Err(e) => {
                    if !policy.handle_io_error(e) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Get reference to account manager
    pub fn account_manager(&self) -> &M {
        &self.account_manager
    }
}

/// Results from processing streams across multiple shards
#[derive(Debug)]
pub struct ProcessorResults {
    pub shard_results: Vec<ShardResult>,
    pub total_streams: usize,
}

/// Result from processing a single shard
#[derive(Debug)]
pub struct ShardResult {
    pub shard_id: usize,
    pub streams_processed: usize,
    pub success: bool,
}

impl ProcessorResults {
    /// Check if all shards processed successfully
    pub fn all_succeeded(&self) -> bool {
        self.shard_results.iter().all(|r| r.success)
    }

    /// Get total number of shards
    pub fn total_shards(&self) -> usize {
        self.shard_results.len()
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
    async fn processes_single_stream() {
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let store = Arc::new(ConcurrentTransactionStore::new());

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

        let results = StreamProcessor::new(account_manager.clone(), store, SilentSkip)
            .add_stream(stream::iter(transactions))
            .process()
            .await;

        assert!(results.all_succeeded());
        assert_eq!(results.total_streams, 1);

        let entry1 = account_manager.entry(1).unwrap();
        assert_eq!(entry1.read().available(), FixedPoint::from_raw(10_000));

        let entry2 = account_manager.entry(2).unwrap();
        assert_eq!(entry2.read().available(), FixedPoint::from_raw(20_000));
    }

    #[tokio::test]
    async fn processes_multiple_streams_merged() {
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let store = Arc::new(ConcurrentTransactionStore::new());

        let stream1 = stream::iter(vec![
            Ok(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            }),
        ]);

        let stream2 = stream::iter(vec![
            Ok(Transaction::Deposit {
                client_id: 2,
                tx_id: 2,
                amount: FixedPoint::from_raw(20_000),
            }),
        ]);

        let results = StreamProcessor::new(account_manager.clone(), store, SilentSkip)
            .with_stream_combinator(StreamCombinator::Merge)
            .add_stream(stream1)
            .add_stream(stream2)
            .process()
            .await;

        assert!(results.all_succeeded());
        assert_eq!(results.total_streams, 2);

        let entry1 = account_manager.entry(1).unwrap();
        assert_eq!(entry1.read().available(), FixedPoint::from_raw(10_000));

        let entry2 = account_manager.entry(2).unwrap();
        assert_eq!(entry2.read().available(), FixedPoint::from_raw(20_000));
    }

    #[tokio::test]
    async fn processes_multiple_streams_chained() {
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let store = Arc::new(ConcurrentTransactionStore::new());

        let stream1 = stream::iter(vec![
            Ok(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            }),
        ]);

        let stream2 = stream::iter(vec![
            Ok(Transaction::Deposit {
                client_id: 1,
                tx_id: 2,
                amount: FixedPoint::from_raw(5_000),
            }),
        ]);

        let results = StreamProcessor::new(account_manager.clone(), store, SilentSkip)
            .with_stream_combinator(StreamCombinator::Chain)
            .add_stream(stream1)
            .add_stream(stream2)
            .process()
            .await;

        assert!(results.all_succeeded());

        let entry1 = account_manager.entry(1).unwrap();
        assert_eq!(entry1.read().available(), FixedPoint::from_raw(15_000));
    }

    #[tokio::test]
    async fn processes_with_multiple_shards() {
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let store = Arc::new(ConcurrentTransactionStore::new());

        let stream1 = stream::iter(vec![
            Ok(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            }),
        ]);

        let stream2 = stream::iter(vec![
            Ok(Transaction::Deposit {
                client_id: 2,
                tx_id: 2,
                amount: FixedPoint::from_raw(20_000),
            }),
        ]);

        let results = StreamProcessor::new(account_manager.clone(), store, SilentSkip)
            .with_shards(2)
            .add_stream(stream1)
            .add_stream(stream2)
            .process()
            .await;

        assert!(results.all_succeeded());
        assert_eq!(results.total_shards(), 2);

        let entry1 = account_manager.entry(1).unwrap();
        assert_eq!(entry1.read().available(), FixedPoint::from_raw(10_000));

        let entry2 = account_manager.entry(2).unwrap();
        assert_eq!(entry2.read().available(), FixedPoint::from_raw(20_000));
    }

    #[tokio::test]
    async fn skip_errors_continues_on_io_error() {
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let store = Arc::new(ConcurrentTransactionStore::new());

        let transactions = vec![
            Ok(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            }),
            Err(IoError::InvalidTransactionType("invalid".to_string())),
            Ok(Transaction::Deposit {
                client_id: 2,
                tx_id: 2,
                amount: FixedPoint::from_raw(20_000),
            }),
        ];

        let results = StreamProcessor::new(account_manager.clone(), store, SkipErrors)
            .add_stream(stream::iter(transactions))
            .process()
            .await;

        assert!(results.all_succeeded());

        let entry1 = account_manager.entry(1).unwrap();
        assert_eq!(entry1.read().available(), FixedPoint::from_raw(10_000));

        let entry2 = account_manager.entry(2).unwrap();
        assert_eq!(entry2.read().available(), FixedPoint::from_raw(20_000));
    }

    #[tokio::test]
    async fn abort_on_error_stops_on_io_error() {
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let store = Arc::new(ConcurrentTransactionStore::new());

        let transactions = vec![
            Ok(Transaction::Deposit {
                client_id: 1,
                tx_id: 1,
                amount: FixedPoint::from_raw(10_000),
            }),
            Err(IoError::InvalidTransactionType("invalid".to_string())),
            Ok(Transaction::Deposit {
                client_id: 2,
                tx_id: 2,
                amount: FixedPoint::from_raw(20_000),
            }),
        ];

        let results = StreamProcessor::new(account_manager.clone(), store, AbortOnError)
            .add_stream(stream::iter(transactions))
            .process()
            .await;

        assert!(!results.all_succeeded());

        // First transaction should be processed
        let entry1 = account_manager.entry(1).unwrap();
        assert_eq!(entry1.read().available(), FixedPoint::from_raw(10_000));

        // Second transaction should NOT be processed
        let entry2 = account_manager.entry(2).unwrap();
        assert_eq!(entry2.read().available(), FixedPoint::zero());
    }

    #[tokio::test]
    async fn skip_errors_continues_on_engine_error() {
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let store = Arc::new(ConcurrentTransactionStore::new());

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

        let results = StreamProcessor::new(account_manager.clone(), store, SkipErrors)
            .add_stream(stream::iter(transactions))
            .process()
            .await;

        assert!(results.all_succeeded());

        // First deposit should succeed
        let entry1 = account_manager.entry(1).unwrap();
        assert_eq!(entry1.read().available(), FixedPoint::from_raw(10_000));

        // Third deposit should succeed despite second transaction failing
        let entry2 = account_manager.entry(2).unwrap();
        assert_eq!(entry2.read().available(), FixedPoint::from_raw(5_000));
    }

    #[tokio::test]
    async fn abort_on_error_stops_on_engine_error() {
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let store = Arc::new(ConcurrentTransactionStore::new());

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

        let results = StreamProcessor::new(account_manager.clone(), store, AbortOnError)
            .add_stream(stream::iter(transactions))
            .process()
            .await;

        assert!(!results.all_succeeded());

        // First deposit should succeed
        let entry1 = account_manager.entry(1).unwrap();
        assert_eq!(entry1.read().available(), FixedPoint::from_raw(10_000));

        // Third deposit should NOT be processed (aborted after engine error)
        let entry2 = account_manager.entry(2).unwrap();
        assert_eq!(entry2.read().available(), FixedPoint::zero());
    }

    #[tokio::test]
    async fn processes_empty_stream() {
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let store = Arc::new(ConcurrentTransactionStore::new());

        let transactions: Vec<Result<Transaction<FixedPoint>, IoError>> = vec![];

        let results = StreamProcessor::new(account_manager, store, SilentSkip)
            .add_stream(stream::iter(transactions))
            .process()
            .await;

        assert!(results.all_succeeded());
    }

    #[tokio::test]
    async fn handles_no_streams() {
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let store = Arc::new(ConcurrentTransactionStore::new());

        let results = StreamProcessor::new(account_manager, store, SilentSkip)
            .process()
            .await;

        assert_eq!(results.total_streams, 0);
        assert_eq!(results.total_shards(), 0);
    }
}
