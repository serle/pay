mod common;

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, BenchmarkId};
use pay::prelude::*;
use common::{setup_processor, create_deposit_batch, create_mixed_batch};

/// Benchmark deposit transaction processing throughput
fn bench_deposit_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("deposit_throughput");

    for count in [100, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter_batched(
                || (setup_processor(), create_deposit_batch(1, count, 1)),
                |(mut processor, transactions)| {
                    for tx in transactions {
                        black_box(processor.process_transaction(tx).ok());
                    }
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark withdrawal transaction processing throughput
fn bench_withdrawal_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("withdrawal_throughput");

    for count in [100, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter_batched(
                || {
                    let mut processor = setup_processor();
                    // First deposit funds
                    for i in 0..count {
                        processor.process_transaction(Transaction::Deposit {
                            client_id: 1,
                            tx_id: i as u32,
                            amount: FixedPoint::from_raw(100_000),
                        }).unwrap();
                    }

                    // Create withdrawal transactions
                    let withdrawals: Vec<_> = (count..count * 2)
                        .map(|i| Transaction::Withdrawal {
                            client_id: 1,
                            tx_id: i as u32,
                            amount: FixedPoint::from_raw(10_000),
                        })
                        .collect();

                    (processor, withdrawals)
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

    group.finish();
}

/// Benchmark dispute workflow (dispute → resolve)
fn bench_dispute_workflow(c: &mut Criterion) {
    let mut group = c.benchmark_group("dispute_workflow");

    for count in [100, 1_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter_batched(
                || {
                    let mut processor = setup_processor();
                    // First deposit funds to create transactions that can be disputed
                    for i in 0..count {
                        processor.process_transaction(Transaction::Deposit {
                            client_id: 1,
                            tx_id: i as u32,
                            amount: FixedPoint::from_raw(10_000),
                        }).unwrap();
                    }

                    // Create dispute + resolve pairs
                    let mut workflow = Vec::with_capacity(count * 2);
                    for i in 0..count {
                        workflow.push(Transaction::Dispute {
                            client_id: 1,
                            tx_id: i as u32,
                        });
                        workflow.push(Transaction::Resolve {
                            client_id: 1,
                            tx_id: i as u32,
                        });
                    }

                    (processor, workflow)
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

    group.finish();
}

/// Benchmark chargeback workflow (dispute → chargeback)
fn bench_chargeback_workflow(c: &mut Criterion) {
    let mut group = c.benchmark_group("chargeback_workflow");

    for count in [100, 1_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter_batched(
                || {
                    let mut processor = setup_processor();
                    // Create separate accounts for each chargeback to avoid locking
                    for i in 0..count {
                        processor.process_transaction(Transaction::Deposit {
                            client_id: (i + 1) as u16,
                            tx_id: i as u32,
                            amount: FixedPoint::from_raw(10_000),
                        }).unwrap();
                    }

                    // Create dispute + chargeback pairs
                    let mut workflow = Vec::with_capacity(count * 2);
                    for i in 0..count {
                        workflow.push(Transaction::Dispute {
                            client_id: (i + 1) as u16,
                            tx_id: i as u32,
                        });
                        workflow.push(Transaction::Chargeback {
                            client_id: (i + 1) as u16,
                            tx_id: i as u32,
                        });
                    }

                    (processor, workflow)
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

    group.finish();
}

/// Benchmark mixed transaction workload (realistic ratio)
fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");

    // Test with different numbers of clients to understand contention impact
    for (count, num_clients) in [(1_000, 10), (10_000, 100), (100_000, 1_000)] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_txs_{}_clients", count, num_clients)),
            &(count, num_clients),
            |b, &(count, num_clients)| {
                b.iter_batched(
                    || (setup_processor(), create_mixed_batch(1, count, num_clients)),
                    |(mut processor, transactions)| {
                        for tx in transactions {
                            black_box(processor.process_transaction(tx).ok());
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark account locking overhead
fn bench_locked_account_overhead(c: &mut Criterion) {
    c.bench_function("locked_account_rejection", |b| {
        b.iter_batched(
            || {
                let mut processor = setup_processor();
                // Create account, deposit, dispute, chargeback (locks account)
                processor.process_transaction(Transaction::Deposit {
                    client_id: 1,
                    tx_id: 1,
                    amount: FixedPoint::from_raw(10_000),
                }).unwrap();
                processor.process_transaction(Transaction::Dispute {
                    client_id: 1,
                    tx_id: 1,
                }).unwrap();
                processor.process_transaction(Transaction::Chargeback {
                    client_id: 1,
                    tx_id: 1,
                }).unwrap();

                // Try to deposit to locked account
                processor
            },
            |mut processor| {
                for i in 0..1_000 {
                    black_box(processor.process_transaction(Transaction::Deposit {
                        client_id: 1,
                        tx_id: (i + 100) as u32,
                        amount: FixedPoint::from_raw(10_000),
                    }).ok());
                }
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    bench_deposit_throughput,
    bench_withdrawal_throughput,
    bench_dispute_workflow,
    bench_chargeback_workflow,
    bench_mixed_workload,
    bench_locked_account_overhead,
);

criterion_main!(benches);
