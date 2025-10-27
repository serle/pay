use pay::prelude::*;

/// Single-threaded hotpath profiling
///
/// Profiles pure transaction processing without concurrency overhead.
/// Shows where time is spent in domain logic, storage operations, and validation.
///
/// Run with: cargo run --release --bin hotpath_single_threaded --features profiling
#[hotpath::main]
fn main() {
    println!("=== Single-Threaded Hotpath Profile ===");
    println!("Workload: 1M transactions across 10K clients");
    println!("Configuration: No concurrency, pure synchronous processing");
    println!();

    // Create processor
    let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
    let transaction_store = ConcurrentTransactionStore::<FixedPoint>::new();
    let mut processor = TransactionProcessor::new(account_manager, transaction_store);

    // Configuration
    let num_transactions = 1_000_000;
    let num_clients = 10_000;

    println!("Starting profiled execution...");
    println!();

    // Run profiled workload
    run_workload(&mut processor, num_transactions, num_clients);

    println!();
    println!("Profiling complete. Results above show function-level breakdown.");
}

#[hotpath::measure]
fn run_workload(
    processor: &mut TransactionProcessor<
        FixedPoint,
        ConcurrentAccountManager<FixedPoint>,
        ConcurrentTransactionStore<FixedPoint>,
    >,
    num_transactions: usize,
    num_clients: u16,
) {
    let mut tx_id = 0u32;
    let mut deposited_txs = Vec::new();

    for i in 0..num_transactions {
        let client_id = (i % num_clients as usize) as u16 + 1;
        let tx_type = i % 10;

        match tx_type {
            0..=5 => {
                // Deposit (60%)
                process_deposit(processor, client_id, tx_id, i, &mut deposited_txs);
                tx_id += 1;
            }
            6..=8 => {
                // Withdrawal (30%)
                if !deposited_txs.is_empty() {
                    process_withdrawal(processor, client_id, tx_id, i);
                    tx_id += 1;
                }
            }
            9 => {
                // Dispute (10%)
                if let Some(&(dep_client, dep_tx)) = deposited_txs.get(i % deposited_txs.len())
                    && dep_client == client_id
                {
                    process_dispute(processor, client_id, dep_tx);
                }
            }
            _ => unreachable!(),
        }

        // Print progress
        if (i + 1) % 100_000 == 0 {
            println!("Processed {} / {} transactions", i + 1, num_transactions);
        }
    }
}

#[hotpath::measure]
fn process_deposit(
    processor: &mut TransactionProcessor<
        FixedPoint,
        ConcurrentAccountManager<FixedPoint>,
        ConcurrentTransactionStore<FixedPoint>,
    >,
    client_id: u16,
    tx_id: u32,
    i: usize,
    deposited_txs: &mut Vec<(u16, u32)>,
) {
    let amount = FixedPoint::from_raw(((i % 1000) + 1) as i64 * 10_000);
    let _ = processor.process_transaction(Transaction::Deposit {
        client_id,
        tx_id,
        amount,
    });
    deposited_txs.push((client_id, tx_id));
}

#[hotpath::measure]
fn process_withdrawal(
    processor: &mut TransactionProcessor<
        FixedPoint,
        ConcurrentAccountManager<FixedPoint>,
        ConcurrentTransactionStore<FixedPoint>,
    >,
    client_id: u16,
    tx_id: u32,
    i: usize,
) {
    let amount = FixedPoint::from_raw(((i % 100) + 1) as i64 * 10_000);
    let _ = processor.process_transaction(Transaction::Withdrawal {
        client_id,
        tx_id,
        amount,
    });
}

#[hotpath::measure]
fn process_dispute(
    processor: &mut TransactionProcessor<
        FixedPoint,
        ConcurrentAccountManager<FixedPoint>,
        ConcurrentTransactionStore<FixedPoint>,
    >,
    client_id: u16,
    tx_id: u32,
) {
    let _ = processor.process_transaction(Transaction::Dispute { client_id, tx_id });
}
