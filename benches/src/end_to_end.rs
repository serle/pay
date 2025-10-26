mod common;

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, BenchmarkId};
use pay::prelude::*;
use common::generate_csv_dataset;
use tokio::runtime::Runtime;
use futures::io::Cursor;

/// Benchmark complete CSV pipeline with different dataset sizes
fn bench_csv_pipeline_dataset_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("csv_pipeline_sizes");
    let runtime = Runtime::new().unwrap();

    for (size_name, num_transactions, num_clients) in [
        ("small_1k", 1_000, 100),
        ("medium_10k", 10_000, 1_000),
        ("large_100k", 100_000, 10_000),
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(size_name),
            &(num_transactions, num_clients),
            |b, &(num_transactions, num_clients)| {
                b.to_async(&runtime).iter_batched(
                    || {
                        // Generate CSV data
                        let csv_data = generate_csv_dataset(
                            num_transactions,
                            num_clients,
                            0.6,  // 60% deposits
                            0.3,  // 30% withdrawals
                            0.05, // 5% disputes
                        );
                        csv_data
                    },
                    |csv_data| async move {
                        let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
                        let transaction_store = ConcurrentTransactionStore::<FixedPoint>::new();
                        let processor = TransactionProcessor::new(account_manager, transaction_store);

                        let input = Cursor::new(csv_data);
                        let stream = CsvTransactionStream::<_, FixedPoint>::new(input);

                        let mut session = ProcessingSession::new(processor, SkipErrors);
                        black_box(session.process_stream(stream).await);

                        // Write snapshot
                        let mut output = Vec::new();
                        write_snapshot(session.account_manager(), &mut output)
                            .await
                            .unwrap();
                        black_box(output);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark with different client distributions
fn bench_csv_client_distributions(c: &mut Criterion) {
    let mut group = c.benchmark_group("csv_client_distributions");
    let runtime = Runtime::new().unwrap();

    let num_transactions = 10_000;

    for (dist_name, num_clients) in [
        ("single_client_worst_case", 1),
        ("few_clients_100", 100),
        ("many_clients_1000", 1_000),
        ("very_many_clients_10000", 10_000),
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(dist_name),
            &num_clients,
            |b, &num_clients| {
                b.to_async(&runtime).iter_batched(
                    || {
                        generate_csv_dataset(
                            num_transactions,
                            num_clients,
                            0.6,  // 60% deposits
                            0.3,  // 30% withdrawals
                            0.05, // 5% disputes
                        )
                    },
                    |csv_data| async move {
                        let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
                        let transaction_store = ConcurrentTransactionStore::<FixedPoint>::new();
                        let processor = TransactionProcessor::new(account_manager, transaction_store);

                        let input = Cursor::new(csv_data);
                        let stream = CsvTransactionStream::<_, FixedPoint>::new(input);

                        let mut session = ProcessingSession::new(processor, SkipErrors);
                        black_box(session.process_stream(stream).await);

                        let mut output = Vec::new();
                        write_snapshot(session.account_manager(), &mut output)
                            .await
                            .unwrap();
                        black_box(output);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark with different transaction patterns
fn bench_csv_transaction_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("csv_transaction_patterns");
    let runtime = Runtime::new().unwrap();

    let num_transactions = 10_000;
    let num_clients = 1_000;

    for (pattern_name, deposit_ratio, withdrawal_ratio, dispute_ratio) in [
        ("deposit_heavy_90", 0.9, 0.05, 0.02),
        ("balanced_50_30_10", 0.5, 0.3, 0.1),
        ("withdrawal_heavy_60", 0.3, 0.6, 0.05),
        ("dispute_heavy_30", 0.5, 0.1, 0.3),
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(pattern_name),
            &(deposit_ratio, withdrawal_ratio, dispute_ratio),
            |b, &(deposit_ratio, withdrawal_ratio, dispute_ratio)| {
                b.to_async(&runtime).iter_batched(
                    || {
                        generate_csv_dataset(
                            num_transactions,
                            num_clients,
                            deposit_ratio,
                            withdrawal_ratio,
                            dispute_ratio,
                        )
                    },
                    |csv_data| async move {
                        let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
                        let transaction_store = ConcurrentTransactionStore::<FixedPoint>::new();
                        let processor = TransactionProcessor::new(account_manager, transaction_store);

                        let input = Cursor::new(csv_data);
                        let stream = CsvTransactionStream::<_, FixedPoint>::new(input);

                        let mut session = ProcessingSession::new(processor, SkipErrors);
                        black_box(session.process_stream(stream).await);

                        let mut output = Vec::new();
                        write_snapshot(session.account_manager(), &mut output)
                            .await
                            .unwrap();
                        black_box(output);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark snapshot generation with varying account counts
fn bench_snapshot_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot_generation");
    let runtime = Runtime::new().unwrap();

    for num_accounts in [100, 1_000, 10_000, 100_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_accounts),
            &num_accounts,
            |b, &num_accounts| {
                b.to_async(&runtime).iter_batched(
                    || {
                        let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
                        let transaction_store = ConcurrentTransactionStore::<FixedPoint>::new();
                        let mut processor = TransactionProcessor::new(account_manager, transaction_store);

                        // Populate accounts
                        for i in 0..num_accounts {
                            processor
                                .process_transaction(Transaction::Deposit {
                                    client_id: i as u16,
                                    tx_id: i as u32,
                                    amount: FixedPoint::from_raw(10_000),
                                })
                                .unwrap();
                        }

                        processor
                    },
                    |processor| async move {
                        let mut output = Vec::new();
                        write_snapshot(processor.account_manager(), &mut output)
                            .await
                            .unwrap();
                        black_box(output);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark error handling overhead
fn bench_error_handling_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_handling_overhead");
    let runtime = Runtime::new().unwrap();

    let num_transactions = 10_000;
    let num_clients = 1_000;

    group.bench_function("skip_errors_policy", |b| {
        b.to_async(&runtime).iter_batched(
            || {
                // Generate dataset with many errors (insufficient funds)
                let csv_data = generate_csv_dataset(
                    num_transactions,
                    num_clients,
                    0.2,  // 20% deposits
                    0.7,  // 70% withdrawals (most will fail)
                    0.05, // 5% disputes
                );
                csv_data
            },
            |csv_data| async move {
                let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
                let transaction_store = ConcurrentTransactionStore::<FixedPoint>::new();
                let processor = TransactionProcessor::new(account_manager, transaction_store);

                let input = Cursor::new(csv_data.into_bytes());
                let stream = CsvTransactionStream::<_, FixedPoint>::new(input);

                let mut session = ProcessingSession::new(processor, SkipErrors);
                black_box(session.process_stream(stream).await);
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("silent_skip_policy", |b| {
        b.to_async(&runtime).iter_batched(
            || {
                let csv_data = generate_csv_dataset(
                    num_transactions,
                    num_clients,
                    0.2,  // 20% deposits
                    0.7,  // 70% withdrawals (most will fail)
                    0.05, // 5% disputes
                );
                csv_data
            },
            |csv_data| async move {
                let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
                let transaction_store = ConcurrentTransactionStore::<FixedPoint>::new();
                let processor = TransactionProcessor::new(account_manager, transaction_store);

                let input = Cursor::new(csv_data.into_bytes());
                let stream = CsvTransactionStream::<_, FixedPoint>::new(input);

                let mut session = ProcessingSession::new(processor, SilentSkip);
                black_box(session.process_stream(stream).await);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark parsing overhead vs processing overhead
fn bench_parsing_vs_processing(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();
    let num_transactions = 10_000;
    let num_clients = 1_000;

    c.bench_function("parsing_only", |b| {
        b.to_async(&runtime).iter_batched(
            || {
                generate_csv_dataset(num_transactions, num_clients, 0.6, 0.3, 0.05)
            },
            |csv_data| async move {
                let input = Cursor::new(csv_data.into_bytes());
                let stream = CsvTransactionStream::<_, FixedPoint>::new(input);

                // Just parse, don't process
                use futures::StreamExt;
                let mut count = 0;
                let mut pinned_stream = Box::pin(stream);
                while let Some(result) = pinned_stream.next().await {
                    if let Ok(tx) = result {
                        black_box(tx);
                        count += 1;
                    }
                }
                black_box(count);
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("processing_only", |b| {
        b.iter_batched(
            || {
                // Pre-parse transactions
                let processor = common::setup_processor();
                let transactions: Vec<_> = (0..num_transactions)
                    .map(|i| Transaction::Deposit {
                        client_id: ((i % num_clients as usize) + 1) as u16,
                        tx_id: i as u32,
                        amount: FixedPoint::from_raw(10_000),
                    })
                    .collect();
                (processor, transactions)
            },
            |(mut processor, transactions)| {
                for tx in transactions {
                    black_box(processor.process_transaction(tx).ok());
                }
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    bench_csv_pipeline_dataset_sizes,
    bench_csv_client_distributions,
    bench_csv_transaction_patterns,
    bench_snapshot_generation,
    bench_error_handling_overhead,
    bench_parsing_vs_processing,
);

criterion_main!(benches);
