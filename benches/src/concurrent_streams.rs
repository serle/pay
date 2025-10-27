mod common;

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, BenchmarkId};
use pay::prelude::*;
use std::sync::Arc;
use tokio::runtime::Runtime;

// NOTE: This benchmark tests PROCESSOR-LEVEL PARALLELISM, not stream topology composition.
// It spawns multiple independent TransactionProcessor instances in separate tokio tasks
// to measure concurrent processing throughput and contention characteristics.
//
// For stream topology composition examples, see:
// - examples/sequential_topology.rs - Sequential stream processing
// - examples/concurrent_topology.rs - Concurrent stream merging

/// Type alias for benchmark setup data to reduce type complexity
type BenchSetup = (
    Arc<ConcurrentAccountManager<FixedPoint>>,
    Arc<ConcurrentTransactionStore<FixedPoint>>,
    Vec<Vec<Transaction<FixedPoint>>>
);

/// Benchmark parallel processor scaling with varying number of concurrent tasks
///
/// Tests how the system scales when processing transactions in parallel across
/// multiple independent processors. Each processor runs in its own tokio task
/// with disjoint client IDs (low contention scenario).
fn bench_concurrent_streams_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_streams_scaling");
    let runtime = Runtime::new().unwrap();

    // Test scaling from 1 to 10,000 streams
    for num_streams in [1, 10, 100, 1_000, 10_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_streams),
            &num_streams,
            |b, &num_streams| {
                b.to_async(&runtime).iter_batched(
                    || {
                        // Each stream will process 100 transactions
                        let transactions_per_stream = 100;
                        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
                        let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

                        // Create streams with disjoint client IDs (low contention)
                        let streams: Vec<_> = (0..num_streams)
                            .map(|stream_id| {
                                let client_id = stream_id as u16 + 1;
                                let start_tx_id = (stream_id * transactions_per_stream) as u32;

                                let transactions: Vec<_> = (0..transactions_per_stream)
                                    .map(|i| Transaction::Deposit {
                                        client_id,
                                        tx_id: start_tx_id + i as u32,
                                        amount: FixedPoint::from_raw(10_000),
                                    })
                                    .collect();

                                transactions
                            })
                            .collect();

                        (account_manager, transaction_store, streams)
                    },
                    |(account_manager, transaction_store, streams)| async move {
                        // Process all streams concurrently
                        let handles: Vec<_> = streams
                            .into_iter()
                            .map(|transactions| {
                                let acc_mgr = Arc::clone(&account_manager);
                                let tx_store = Arc::clone(&transaction_store);

                                tokio::spawn(async move {
                                    let mut processor = TransactionProcessor::new(
                                        Arc::clone(&acc_mgr),
                                        Arc::clone(&tx_store),
                                    );

                                    for tx in transactions {
                                        black_box(processor.process_transaction(tx).ok());
                                    }
                                })
                            })
                            .collect();

                        for handle in handles {
                            handle.await.unwrap();
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark high contention scenario (all processors access same account)
///
/// Stress test: Multiple parallel processors all updating the same client account.
/// Measures maximum lock contention and wait times in the DashMap-based account manager.
fn bench_high_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("high_contention");
    let runtime = Runtime::new().unwrap();

    for num_streams in [10, 100, 1_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_streams),
            &num_streams,
            |b, &num_streams| {
                b.to_async(&runtime).iter_batched(
                    || {
                        let transactions_per_stream = 100;
                        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
                        let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

                        // All streams access CLIENT 1 (maximum contention)
                        let streams: Vec<_> = (0..num_streams)
                            .map(|stream_id| {
                                let start_tx_id = (stream_id * transactions_per_stream) as u32;

                                let transactions: Vec<_> = (0..transactions_per_stream)
                                    .map(|i| Transaction::Deposit {
                                        client_id: 1,  // Same client for all streams!
                                        tx_id: start_tx_id + i as u32,
                                        amount: FixedPoint::from_raw(10_000),
                                    })
                                    .collect();

                                transactions
                            })
                            .collect();

                        (account_manager, transaction_store, streams)
                    },
                    |(account_manager, transaction_store, streams)| async move {
                        let handles: Vec<_> = streams
                            .into_iter()
                            .map(|transactions| {
                                let acc_mgr = Arc::clone(&account_manager);
                                let tx_store = Arc::clone(&transaction_store);

                                tokio::spawn(async move {
                                    let mut processor = TransactionProcessor::new(
                                        Arc::clone(&acc_mgr),
                                        Arc::clone(&tx_store),
                                    );

                                    for tx in transactions {
                                        black_box(processor.process_transaction(tx).ok());
                                    }
                                })
                            })
                            .collect();

                        for handle in handles {
                            handle.await.unwrap();
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark low contention scenario (each processor has disjoint accounts)
///
/// Ideal case: Each parallel processor operates on completely separate client accounts.
/// Measures overhead of parallelism without lock contention.
fn bench_low_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("low_contention");
    let runtime = Runtime::new().unwrap();

    for num_streams in [10, 100, 1_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_streams),
            &num_streams,
            |b, &num_streams| {
                b.to_async(&runtime).iter_batched(
                    || {
                        let transactions_per_stream = 100;
                        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
                        let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

                        // Each stream has completely disjoint client IDs
                        let streams: Vec<_> = (0..num_streams)
                            .map(|stream_id| {
                                // Use modulo to prevent u16 overflow while keeping disjoint ranges
                                let base_client = ((stream_id * 100) % 60000) as u16;
                                let start_tx_id = (stream_id * transactions_per_stream) as u32;

                                let transactions: Vec<_> = (0..transactions_per_stream)
                                    .map(|i| Transaction::Deposit {
                                        client_id: base_client + (i % 100) as u16,
                                        tx_id: start_tx_id + i as u32,
                                        amount: FixedPoint::from_raw(10_000),
                                    })
                                    .collect();

                                transactions
                            })
                            .collect();

                        (account_manager, transaction_store, streams)
                    },
                    |(account_manager, transaction_store, streams)| async move {
                        let handles: Vec<_> = streams
                            .into_iter()
                            .map(|transactions| {
                                let acc_mgr = Arc::clone(&account_manager);
                                let tx_store = Arc::clone(&transaction_store);

                                tokio::spawn(async move {
                                    let mut processor = TransactionProcessor::new(
                                        Arc::clone(&acc_mgr),
                                        Arc::clone(&tx_store),
                                    );

                                    for tx in transactions {
                                        black_box(processor.process_transaction(tx).ok());
                                    }
                                })
                            })
                            .collect();

                        for handle in handles {
                            handle.await.unwrap();
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark error policy impact under parallel processor load
///
/// Tests error handling overhead when multiple parallel processors
/// encounter failures (insufficient funds). Uses StreamProcessor with
/// SkipErrors policy.
fn bench_error_policy_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_policy_concurrent");
    let runtime = Runtime::new().unwrap();

    let num_streams = 100;
    let transactions_per_stream = 100;

    let setup = || {
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

        // Create streams with some invalid transactions (insufficient funds)
        let streams: Vec<_> = (0..num_streams)
            .map(|stream_id| {
                let client_id = stream_id as u16 + 1;
                let start_tx_id = (stream_id * transactions_per_stream) as u32;

                let mut transactions = vec![];
                // First deposit
                transactions.push(Transaction::Deposit {
                    client_id,
                    tx_id: start_tx_id,
                    amount: FixedPoint::from_raw(10_000),
                });

                // Then many withdrawals (most will fail due to insufficient funds)
                for i in 1..transactions_per_stream {
                    transactions.push(Transaction::Withdrawal {
                        client_id,
                        tx_id: start_tx_id + i as u32,
                        amount: FixedPoint::from_raw(5_000),
                    });
                }

                transactions
            })
            .collect();

        (account_manager, transaction_store, streams)
    };

    let bench = |(account_manager, transaction_store, streams): BenchSetup| async move {
        let handles: Vec<_> = streams
            .into_iter()
            .map(|transactions| {
                let acc_mgr = Arc::clone(&account_manager);
                let tx_store = Arc::clone(&transaction_store);

                tokio::spawn(async move {
                    let stream = futures::stream::iter(
                        transactions.into_iter().map(Ok::<_, IoError>)
                    );

                    StreamProcessor::new(acc_mgr, tx_store, SkipErrors)
                        .add_stream(stream)
                        .process()
                        .await
                        .all_succeeded()
                })
            })
            .collect();

        for handle in handles {
            handle.await.unwrap();
        }
    };

    group.bench_function("skip_errors", |b| {
        b.to_async(&runtime).iter_batched(setup, bench, BatchSize::SmallInput);
    });

    group.finish();
}

/// Benchmark with zipf distribution (realistic access pattern)
///
/// Simulates real-world scenario where 20% of clients receive 80% of traffic.
/// Tests contention patterns under realistic workload distribution across
/// multiple parallel processors.
fn bench_zipf_distribution(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let setup = || {
        let num_streams = 100;
        let transactions_per_stream = 100;
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

        // Zipf-like distribution: 20% of clients get 80% of traffic
        let hot_clients = 20u16;
        let total_clients = 100u16;

        let streams: Vec<_> = (0..num_streams)
            .map(|stream_id| {
                let start_tx_id = (stream_id * transactions_per_stream) as u32;

                let transactions: Vec<_> = (0..transactions_per_stream)
                    .map(|i| {
                        // 80% chance to hit hot clients
                        let client_id = if i % 5 < 4 {
                            (i % hot_clients as usize) as u16 + 1
                        } else {
                            hot_clients + ((i % (total_clients - hot_clients) as usize) as u16) + 1
                        };

                        Transaction::Deposit {
                            client_id,
                            tx_id: start_tx_id + i as u32,
                            amount: FixedPoint::from_raw(10_000),
                        }
                    })
                    .collect();

                transactions
            })
            .collect();

        (account_manager, transaction_store, streams)
    };

    let bench = |(account_manager, transaction_store, streams): BenchSetup| async move {
        let handles: Vec<_> = streams
            .into_iter()
            .map(|transactions| {
                let acc_mgr = Arc::clone(&account_manager);
                let tx_store = Arc::clone(&transaction_store);

                tokio::spawn(async move {
                    let mut processor = TransactionProcessor::new(
                        Arc::clone(&acc_mgr),
                        Arc::clone(&tx_store),
                    );

                    for tx in transactions {
                        black_box(processor.process_transaction(tx).ok());
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.await.unwrap();
        }
    };

    c.bench_function("zipf_distribution_100_streams", |b| {
        b.to_async(&runtime).iter_batched(setup, bench, BatchSize::SmallInput);
    });
}

criterion_group!(
    benches,
    bench_concurrent_streams_scaling,
    bench_high_contention,
    bench_low_contention,
    bench_error_policy_concurrent,
    bench_zipf_distribution,
);

criterion_main!(benches);
