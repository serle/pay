mod common;

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, BenchmarkId};
use pay::prelude::*;
use std::sync::Arc;
use tokio::runtime::{Builder, Runtime};

/// Compare single-threaded vs multi-threaded Tokio runtime
fn bench_runtime_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime_threads");

    // Test with 1, 4, 8, 16, 32, 64 threads
    for num_threads in [1, 4, 8, 16, 32, 64] {
        let runtime = Builder::new_multi_thread()
            .worker_threads(num_threads)
            .build()
            .unwrap();

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}threads_100streams", num_threads)),
            &num_threads,
            |b, _| {
                b.to_async(&runtime).iter_batched(
                    || {
                        let num_streams = 100;
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

/// Compare single-threaded runtime (current_thread) vs multi-threaded
fn bench_single_vs_multi(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime_type");

    // Single-threaded runtime
    let single_thread_runtime = Builder::new_current_thread()
        .build()
        .unwrap();

    group.bench_function("current_thread_100streams", |b| {
        b.to_async(&single_thread_runtime).iter_batched(
            || {
                let num_streams = 100;
                let transactions_per_stream = 100;
                let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
                let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

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
    });

    // Multi-threaded runtime (default)
    let multi_thread_runtime = Runtime::new().unwrap();

    group.bench_function("multi_thread_default_100streams", |b| {
        b.to_async(&multi_thread_runtime).iter_batched(
            || {
                let num_streams = 100;
                let transactions_per_stream = 100;
                let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
                let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

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
    });

    group.finish();
}

criterion_group!(benches, bench_runtime_comparison, bench_single_vs_multi);
criterion_main!(benches);
