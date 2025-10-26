mod common;

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, BenchmarkId};
use pay::prelude::*;
use pay::domain::operations;

/// Benchmark account entry creation (cold cache)
fn bench_account_entry_cold(c: &mut Criterion) {
    let mut group = c.benchmark_group("account_entry_cold");

    for num_accounts in [100, 1_000, 10_000, 100_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_accounts),
            &num_accounts,
            |b, &num_accounts| {
                b.iter_batched(
                    || ConcurrentAccountManager::<FixedPoint>::new(),
                    |manager| {
                        // First access to each account (cold cache)
                        for i in 0..num_accounts {
                            black_box(manager.entry(i as u16).unwrap());
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark account entry access (hot cache - repeated access)
fn bench_account_entry_hot(c: &mut Criterion) {
    let mut group = c.benchmark_group("account_entry_hot");

    for num_accounts in [100, 1_000, 10_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_accounts),
            &num_accounts,
            |b, &num_accounts| {
                b.iter_batched(
                    || {
                        let manager = ConcurrentAccountManager::<FixedPoint>::new();
                        // Warm up the cache
                        for i in 0..num_accounts {
                            let _ = manager.entry(i as u16);
                        }
                        manager
                    },
                    |manager| {
                        // Hot access - repeatedly access same accounts
                        for _ in 0..100 {
                            for i in 0..num_accounts {
                                black_box(manager.entry(i as u16).unwrap());
                            }
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark account update operations
fn bench_account_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("account_update");

    for num_updates in [100, 1_000, 10_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_updates),
            &num_updates,
            |b, &num_updates| {
                b.iter_batched(
                    || ConcurrentAccountManager::<FixedPoint>::new(),
                    |manager| {
                        for _ in 0..num_updates {
                            let mut entry = manager.entry(1).unwrap();
                            black_box(
                                entry
                                    .try_update(|acc| {
                                        operations::apply_deposit(acc, FixedPoint::from_raw(10_000))
                                    })
                                    .unwrap(),
                            );
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark account read operations (no mutation)
fn bench_account_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("account_read");

    for num_accounts in [100, 1_000, 10_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_accounts),
            &num_accounts,
            |b, &num_accounts| {
                b.iter_batched(
                    || {
                        let manager = ConcurrentAccountManager::<FixedPoint>::new();
                        // Populate accounts
                        for i in 0..num_accounts {
                            let mut entry = manager.entry(i as u16).unwrap();
                            entry
                                .try_update(|acc| {
                                    operations::apply_deposit(acc, FixedPoint::from_raw(10_000))
                                })
                                .unwrap();
                        }
                        manager
                    },
                    |manager| {
                        // Read all accounts
                        for i in 0..num_accounts {
                            let entry = manager.entry(i as u16).unwrap();
                            black_box(entry.read());
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark transaction store insert operations
fn bench_transaction_store_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("transaction_store_insert");

    for num_transactions in [100, 1_000, 10_000, 100_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_transactions),
            &num_transactions,
            |b, &num_transactions| {
                b.iter_batched(
                    || ConcurrentTransactionStore::<FixedPoint>::new(),
                    |mut store| {
                        for i in 0..num_transactions {
                            let record = TransactionRecord {
                                client_id: (i % 1000) as u16,
                                amount: FixedPoint::from_raw(10_000),
                            };
                            black_box(store.insert(i as u32, record));
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark transaction store lookup operations
fn bench_transaction_store_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("transaction_store_get");

    for num_transactions in [100, 1_000, 10_000, 100_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_transactions),
            &num_transactions,
            |b, &num_transactions| {
                b.iter_batched(
                    || {
                        let mut store = ConcurrentTransactionStore::<FixedPoint>::new();
                        // Populate store
                        for i in 0..num_transactions {
                            let record = TransactionRecord {
                                client_id: (i % 1000) as u16,
                                amount: FixedPoint::from_raw(10_000),
                            };
                            store.insert(i as u32, record);
                        }
                        store
                    },
                    |store| {
                        // Lookup all transactions
                        for i in 0..num_transactions {
                            black_box(store.get(i as u32));
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark transaction store contains operations
fn bench_transaction_store_contains(c: &mut Criterion) {
    let mut group = c.benchmark_group("transaction_store_contains");

    for num_transactions in [100, 1_000, 10_000, 100_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_transactions),
            &num_transactions,
            |b, &num_transactions| {
                b.iter_batched(
                    || {
                        let mut store = ConcurrentTransactionStore::<FixedPoint>::new();
                        // Populate store
                        for i in 0..num_transactions {
                            let record = TransactionRecord {
                                client_id: (i % 1000) as u16,
                                amount: FixedPoint::from_raw(10_000),
                            };
                            store.insert(i as u32, record);
                        }
                        store
                    },
                    |store| {
                        // Check contains for all transactions
                        for i in 0..num_transactions {
                            black_box(store.contains(i as u32));
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark mixed account operations (read + update pattern)
fn bench_mixed_account_ops(c: &mut Criterion) {
    c.bench_function("mixed_account_ops", |b| {
        b.iter_batched(
            || {
                let manager = ConcurrentAccountManager::<FixedPoint>::new();
                // Populate with initial deposits
                for i in 0..100 {
                    let mut entry = manager.entry(i as u16).unwrap();
                    entry
                        .try_update(|acc| {
                            operations::apply_deposit(acc, FixedPoint::from_raw(100_000))
                        })
                        .unwrap();
                }
                manager
            },
            |manager| {
                // Mixed workload: 70% reads, 30% updates
                for i in 0..1_000 {
                    let client_id = (i % 100) as u16;
                    let entry = manager.entry(client_id).unwrap();

                    if i % 10 < 7 {
                        // Read
                        black_box(entry.read());
                    } else {
                        // Update
                        let mut entry = manager.entry(client_id).unwrap();
                        black_box(
                            entry
                                .try_update(|acc| {
                                    operations::apply_deposit(acc, FixedPoint::from_raw(10_000))
                                })
                                .ok(),
                        );
                    }
                }
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    bench_account_entry_cold,
    bench_account_entry_hot,
    bench_account_update,
    bench_account_read,
    bench_transaction_store_insert,
    bench_transaction_store_get,
    bench_transaction_store_contains,
    bench_mixed_account_ops,
);

criterion_main!(benches);
