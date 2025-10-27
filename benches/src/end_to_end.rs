mod common;

use std::sync::Arc;
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, BenchmarkId};
use pay::prelude::*;
use common::generate_csv_dataset;
use tokio::runtime::Runtime;
use futures::io::Cursor;

/// Type alias for processor with standard storage backends
type Processor = (
    TransactionProcessor<FixedPoint, ConcurrentAccountManager<FixedPoint>, ConcurrentTransactionStore<FixedPoint>>,
    Vec<Transaction<FixedPoint>>
);

/// Benchmark complete CSV pipeline with different dataset sizes
fn bench_csv_pipeline_dataset_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("csv_pipeline_sizes");
    let runtime = Runtime::new().unwrap();

    for (size_name, num_transactions, num_clients) in [
        ("small_1k", 1_000, 100),
        ("medium_10k", 10_000, 1_000),
        ("large_100k", 100_000, 10_000),
    ] {
        let setup = || {
            // Generate CSV data
            generate_csv_dataset(
                num_transactions,
                num_clients,
                0.6,  // 60% deposits
                0.3,  // 30% withdrawals
                0.05, // 5% disputes
            )
        };

        let bench = |csv_data| async move {
            let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
            let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

            let input = Cursor::new(csv_data);
            let stream = CsvTransactionStream::<FixedPoint>::new(input);

            let results = StreamProcessor::new(account_manager.clone(), transaction_store, SkipErrors)
                .add_stream(stream)
                .process()
                .await;
            black_box(results);

            // Write snapshot
            let mut output = Vec::new();
            write_snapshot(&*account_manager, &mut output)
                .await
                .unwrap();
            black_box(output);
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(size_name),
            &(num_transactions, num_clients),
            |b, _| {
                b.to_async(&runtime).iter_batched(setup, bench, BatchSize::SmallInput);
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
        let setup = || {
            generate_csv_dataset(
                num_transactions,
                num_clients,
                0.6,  // 60% deposits
                0.3,  // 30% withdrawals
                0.05, // 5% disputes
            )
        };

        let bench = |csv_data| async move {
            let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
            let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

            let input = Cursor::new(csv_data);
            let stream = CsvTransactionStream::<FixedPoint>::new(input);

            let results = StreamProcessor::new(account_manager.clone(), transaction_store, SkipErrors)
                .add_stream(stream)
                .process()
                .await;
            black_box(results);

            let mut output = Vec::new();
            write_snapshot(&*account_manager, &mut output)
                .await
                .unwrap();
            black_box(output);
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(dist_name),
            &num_clients,
            |b, _| {
                b.to_async(&runtime).iter_batched(setup, bench, BatchSize::SmallInput);
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
        let setup = || {
            generate_csv_dataset(
                num_transactions,
                num_clients,
                deposit_ratio,
                withdrawal_ratio,
                dispute_ratio,
            )
        };

        let bench = |csv_data| async move {
            let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
            let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

            let input = Cursor::new(csv_data);
            let stream = CsvTransactionStream::<FixedPoint>::new(input);

            let results = StreamProcessor::new(account_manager.clone(), transaction_store, SkipErrors)
                .add_stream(stream)
                .process()
                .await;
            black_box(results);

            let mut output = Vec::new();
            write_snapshot(&*account_manager, &mut output)
                .await
                .unwrap();
            black_box(output);
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(pattern_name),
            &(deposit_ratio, withdrawal_ratio, dispute_ratio),
            |b, _| {
                b.to_async(&runtime).iter_batched(setup, bench, BatchSize::SmallInput);
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
        let setup = || {
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
        };

        let bench = |processor: TransactionProcessor<FixedPoint, ConcurrentAccountManager<FixedPoint>, ConcurrentTransactionStore<FixedPoint>>| async move {
            let mut output = Vec::new();
            write_snapshot(processor.account_manager(), &mut output)
                .await
                .unwrap();
            black_box(output);
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(num_accounts),
            &num_accounts,
            |b, _| {
                b.to_async(&runtime).iter_batched(setup, bench, BatchSize::SmallInput);
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

    // Benchmark with SkipErrors policy
    let setup_skip = || {
        // Generate dataset with many errors (insufficient funds)
        generate_csv_dataset(
            num_transactions,
            num_clients,
            0.2,  // 20% deposits
            0.7,  // 70% withdrawals (most will fail)
            0.05, // 5% disputes
        )
    };

    let bench_skip = |csv_data: String| async move {
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

        let input = Cursor::new(csv_data.into_bytes());
        let stream = CsvTransactionStream::<FixedPoint>::new(input);

        let results = StreamProcessor::new(account_manager, transaction_store, SkipErrors)
            .add_stream(stream)
            .process()
            .await;
        black_box(results);
    };

    group.bench_function("skip_errors_policy", |b| {
        b.to_async(&runtime).iter_batched(setup_skip, bench_skip, BatchSize::SmallInput);
    });

    // Benchmark with SilentSkip policy
    let setup_silent = || {
        generate_csv_dataset(
            num_transactions,
            num_clients,
            0.2,  // 20% deposits
            0.7,  // 70% withdrawals (most will fail)
            0.05, // 5% disputes
        )
    };

    let bench_silent = |csv_data: String| async move {
        let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
        let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

        let input = Cursor::new(csv_data.into_bytes());
        let stream = CsvTransactionStream::<FixedPoint>::new(input);

        let results = StreamProcessor::new(account_manager, transaction_store, SilentSkip)
            .add_stream(stream)
            .process()
            .await;
        black_box(results);
    };

    group.bench_function("silent_skip_policy", |b| {
        b.to_async(&runtime).iter_batched(setup_silent, bench_silent, BatchSize::SmallInput);
    });

    group.finish();
}

/// Benchmark parsing overhead vs processing overhead
fn bench_parsing_vs_processing(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();
    let num_transactions = 10_000;
    let num_clients = 1_000;

    let setup_parsing = || {
        generate_csv_dataset(num_transactions, num_clients, 0.6, 0.3, 0.05)
    };

    let bench_parsing = |csv_data: String| async move {
        let input = Cursor::new(csv_data.into_bytes());
        let stream = CsvTransactionStream::<FixedPoint>::new(input);

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
    };

    c.bench_function("parsing_only", |b| {
        b.to_async(&runtime).iter_batched(setup_parsing, bench_parsing, BatchSize::SmallInput);
    });

    let setup_processing = || {
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
    };

    let bench_processing = |(mut processor, transactions): Processor| {
        for tx in transactions {
            black_box(processor.process_transaction(tx).ok());
        }
    };

    c.bench_function("processing_only", |b| {
        b.iter_batched(setup_processing, bench_processing, BatchSize::SmallInput);
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
