mod common;

use std::sync::Arc;
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, BenchmarkId};
use pay::prelude::*;
use common::generate_csv_dataset;
use tokio::runtime::Runtime;
use futures::io::Cursor;

/// Benchmark comparing Chain vs Merge stream combining strategies
///
/// Tests the same workload with:
/// - StreamCombinator::Chain - Sequential stream processing
/// - StreamCombinator::Merge - Concurrent stream processing
fn bench_chain_vs_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("topology_chain_vs_merge");
    let runtime = Runtime::new().unwrap();

    let num_streams = 4;
    let transactions_per_stream = 2_500; // 10K total transactions
    let clients_per_stream = 250; // 1K total clients

    for (combinator_name, combinator) in [
        ("chain_sequential", StreamCombinator::Chain),
        ("merge_concurrent", StreamCombinator::Merge),
    ] {
        let setup = || {
            let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
            let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

            // Generate multiple CSV datasets
            let datasets: Vec<_> = (0..num_streams)
                .map(|_| {
                    generate_csv_dataset(
                        transactions_per_stream,
                        clients_per_stream,
                        0.6,  // 60% deposits
                        0.3,  // 30% withdrawals
                        0.05, // 5% disputes
                    )
                })
                .collect();

            (account_manager, transaction_store, datasets)
        };

        let bench = |(account_manager, transaction_store, datasets): (Arc<ConcurrentAccountManager<FixedPoint>>, Arc<ConcurrentTransactionStore<FixedPoint>>, Vec<String>)| async move {
            let mut processor = StreamProcessor::new(
                account_manager.clone(),
                transaction_store,
                SilentSkip,
            );

            // Add all streams
            for csv_data in datasets {
                let input = Cursor::new(csv_data);
                let stream = CsvTransactionStream::<FixedPoint>::new(input);
                processor = processor.add_stream(stream);
            }

            // Configure stream combinator
            let results = processor
                .with_stream_combinator(combinator)
                .process()
                .await;

            black_box(results);
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(combinator_name),
            &combinator_name,
            |b, _| {
                b.to_async(&runtime).iter_batched(setup, bench, BatchSize::SmallInput);
            },
        );
    }

    group.finish();
}

/// Benchmark comparing different shard counts
///
/// Tests parallel processing with varying degrees of parallelism:
/// - 1 shard (serial)
/// - 2 shards
/// - 4 shards
/// - 8 shards
fn bench_shard_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("topology_shard_scaling");
    let runtime = Runtime::new().unwrap();

    let num_streams = 8;
    let transactions_per_stream = 1_250; // 10K total transactions
    let clients_per_stream = 125; // 1K total clients

    for num_shards in [1, 2, 4, 8] {
        let setup = || {
            let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
            let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

            // Generate multiple CSV datasets with disjoint client IDs to avoid contention
            let datasets: Vec<_> = (0..num_streams)
                .map(|stream_id| {
                    // Generate data with offset client IDs to minimize contention
                    let mut csv_data = String::from("type,client,tx,amount\n");
                    let client_offset = stream_id * clients_per_stream;
                    let tx_offset = stream_id * transactions_per_stream;

                    for i in 0..transactions_per_stream {
                        let client_id = client_offset + (i % clients_per_stream) + 1;
                        let tx_id = tx_offset + i;
                        let tx_type = i % 10;

                        let line = if tx_type < 6 {
                            format!("deposit,{},{},100.0\n", client_id, tx_id)
                        } else if tx_type < 9 {
                            format!("withdrawal,{},{},50.0\n", client_id, tx_id)
                        } else {
                            format!("dispute,{},{},\n", client_id, tx_id / 2)
                        };

                        csv_data.push_str(&line);
                    }

                    csv_data
                })
                .collect();

            (account_manager, transaction_store, datasets)
        };

        let bench = |(account_manager, transaction_store, datasets): (Arc<ConcurrentAccountManager<FixedPoint>>, Arc<ConcurrentTransactionStore<FixedPoint>>, Vec<String>)| async move {
            let mut processor = StreamProcessor::new(
                account_manager.clone(),
                transaction_store,
                SilentSkip,
            );

            // Add all streams
            for csv_data in datasets {
                let input = Cursor::new(csv_data);
                let stream = CsvTransactionStream::<FixedPoint>::new(input);
                processor = processor.add_stream(stream);
            }

            // Configure shards and merge strategy
            let results = processor
                .with_shards(num_shards)
                .with_stream_combinator(StreamCombinator::Merge)
                .process()
                .await;

            black_box(results);
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(num_shards),
            &num_shards,
            |b, _| {
                b.to_async(&runtime).iter_batched(setup, bench, BatchSize::SmallInput);
            },
        );
    }

    group.finish();
}

/// Benchmark comparing RoundRobin vs Sequential shard assignment
fn bench_shard_assignment_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("topology_shard_assignment");
    let runtime = Runtime::new().unwrap();

    let num_shards = 4;
    let num_streams = 16;
    let transactions_per_stream = 625; // 10K total transactions

    // Benchmark RoundRobin
    let setup_rr = || {
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

        let datasets: Vec<_> = (0..num_streams)
            .map(|stream_id| {
                let mut csv_data = String::from("type,client,tx,amount\n");
                let client_offset = stream_id * 100;
                let tx_offset = stream_id * transactions_per_stream;

                for i in 0..transactions_per_stream {
                    let client_id = client_offset + (i % 100) + 1;
                    let tx_id = tx_offset + i;
                    csv_data.push_str(&format!("deposit,{},{},100.0\n", client_id, tx_id));
                }

                csv_data
            })
            .collect();

        (account_manager, transaction_store, datasets)
    };

    let bench_rr = |(account_manager, transaction_store, datasets): (Arc<ConcurrentAccountManager<FixedPoint>>, Arc<ConcurrentTransactionStore<FixedPoint>>, Vec<String>)| async move {
        let mut processor = StreamProcessor::new(
            account_manager.clone(),
            transaction_store,
            SilentSkip,
        );

        for csv_data in datasets {
            let input = Cursor::new(csv_data);
            let stream = CsvTransactionStream::<FixedPoint>::new(input);
            processor = processor.add_stream(stream);
        }

        let results = processor
            .with_shards(num_shards)
            .with_shard_assignment(ShardAssignment::RoundRobin)
            .with_stream_combinator(StreamCombinator::Merge)
            .process()
            .await;

        black_box(results);
    };

    group.bench_function("round_robin", |b| {
        b.to_async(&runtime).iter_batched(setup_rr, bench_rr, BatchSize::SmallInput);
    });

    // Benchmark Sequential
    let setup_seq = || {
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

        let datasets: Vec<_> = (0..num_streams)
            .map(|stream_id| {
                let mut csv_data = String::from("type,client,tx,amount\n");
                let client_offset = stream_id * 100;
                let tx_offset = stream_id * transactions_per_stream;

                for i in 0..transactions_per_stream {
                    let client_id = client_offset + (i % 100) + 1;
                    let tx_id = tx_offset + i;
                    csv_data.push_str(&format!("deposit,{},{},100.0\n", client_id, tx_id));
                }

                csv_data
            })
            .collect();

        (account_manager, transaction_store, datasets)
    };

    let bench_seq = |(account_manager, transaction_store, datasets): (Arc<ConcurrentAccountManager<FixedPoint>>, Arc<ConcurrentTransactionStore<FixedPoint>>, Vec<String>)| async move {
        let mut processor = StreamProcessor::new(
            account_manager.clone(),
            transaction_store,
            SilentSkip,
        );

        for csv_data in datasets {
            let input = Cursor::new(csv_data);
            let stream = CsvTransactionStream::<FixedPoint>::new(input);
            processor = processor.add_stream(stream);
        }

        let results = processor
            .with_shards(num_shards)
            .with_shard_assignment(ShardAssignment::Sequential)
            .with_stream_combinator(StreamCombinator::Merge)
            .process()
            .await;

        black_box(results);
    };

    group.bench_function("sequential", |b| {
        b.to_async(&runtime).iter_batched(setup_seq, bench_seq, BatchSize::SmallInput);
    });

    group.finish();
}

/// Benchmark best case vs worst case topologies
///
/// Best case: Many shards + disjoint clients + merge = maximum parallelism
/// Worst case: Single shard + chain = serial processing
fn bench_best_vs_worst_topology(c: &mut Criterion) {
    let mut group = c.benchmark_group("topology_best_vs_worst");
    let runtime = Runtime::new().unwrap();

    let num_streams = 8;
    let transactions_per_stream = 1_250; // 10K total

    for (config_name, num_shards, combinator) in [
        ("worst_serial", 1, StreamCombinator::Chain),
        ("medium_parallel", 4, StreamCombinator::Merge),
        ("best_parallel", 8, StreamCombinator::Merge),
    ] {
        let setup = || {
            let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
            let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

            // Generate datasets with disjoint client IDs
            let datasets: Vec<_> = (0..num_streams)
                .map(|stream_id| {
                    let mut csv_data = String::from("type,client,tx,amount\n");
                    let client_offset = stream_id * 200;
                    let tx_offset = stream_id * transactions_per_stream;

                    for i in 0..transactions_per_stream {
                        let client_id = client_offset + (i % 200) + 1;
                        let tx_id = tx_offset + i;
                        csv_data.push_str(&format!("deposit,{},{},100.0\n", client_id, tx_id));
                    }

                    csv_data
                })
                .collect();

            (account_manager, transaction_store, datasets)
        };

        let bench = |(account_manager, transaction_store, datasets): (Arc<ConcurrentAccountManager<FixedPoint>>, Arc<ConcurrentTransactionStore<FixedPoint>>, Vec<String>)| async move {
            let mut processor = StreamProcessor::new(
                account_manager.clone(),
                transaction_store,
                SilentSkip,
            );

            // Add all streams
            for csv_data in datasets {
                let input = Cursor::new(csv_data);
                let stream = CsvTransactionStream::<FixedPoint>::new(input);
                processor = processor.add_stream(stream);
            }

            // Configure topology
            let results = processor
                .with_shards(num_shards)
                .with_stream_combinator(combinator)
                .process()
                .await;

            black_box(results);
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(config_name),
            &config_name,
            |b, _| {
                b.to_async(&runtime).iter_batched(setup, bench, BatchSize::SmallInput);
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_chain_vs_merge,
    bench_shard_scaling,
    bench_shard_assignment_strategies,
    bench_best_vs_worst_topology,
);

criterion_main!(benches);
