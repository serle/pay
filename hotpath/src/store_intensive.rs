use pay::prelude::*;

/// Transaction store intensive profiling (single-threaded)
///
/// Profiles heavy transaction store usage without concurrency overhead.
/// 50% of operations are lookups (disputes/resolves/chargebacks).
///
/// Run with: cargo run --release --bin hotpath_store_intensive --features profiling
#[hotpath::main]
fn main() {
    println!("=== Transaction Store Intensive Hotpath Profile ===");
    println!("Workload: 1M transactions, 50% require transaction store lookups");
    println!("Configuration: Single-threaded, isolates store performance from concurrency");
    println!();

    // Create processor
    let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
    let transaction_store = ConcurrentTransactionStore::<FixedPoint>::new();
    let mut processor = TransactionProcessor::new(account_manager, transaction_store);

    // Configuration
    let num_transactions = 1_000_000;
    let num_clients = 1_000;

    println!("Starting profiled execution...");
    println!();

    // Run profiled workload
    run_workload(&mut processor, num_transactions, num_clients);

    println!();
    println!("Profiling complete. Results show transaction store overhead in isolation.");
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
    let mut disputed_txs = Vec::new();

    for i in 0..num_transactions {
        let client_id = (i % num_clients as usize) as u16 + 1;
        let tx_type = i % 10;

        match tx_type {
            0..=2 => {
                // Deposit (30%)
                process_deposit(processor, client_id, tx_id, i, &mut deposited_txs);
                tx_id += 1;
            }
            3 => {
                // Withdrawal (10%)
                if !deposited_txs.is_empty() {
                    process_withdrawal(processor, client_id, tx_id, i);
                    tx_id += 1;
                }
            }
            4..=5 => {
                // Dispute (20%) - transaction store lookup
                if let Some(&(dep_client, dep_tx)) = deposited_txs.get(i % deposited_txs.len()) {
                    if dep_client == client_id {
                        process_dispute(processor, dep_client, dep_tx);
                        disputed_txs.push((dep_client, dep_tx));
                    }
                }
            }
            6..=7 => {
                // Resolve (20%) - transaction store lookup
                if !disputed_txs.is_empty() {
                    if let Some(&(disp_client, disp_tx)) = disputed_txs.get(i % disputed_txs.len()) {
                        process_resolve(processor, disp_client, disp_tx);
                    }
                }
            }
            8..=9 => {
                // Chargeback (20%) - transaction store lookup
                if !disputed_txs.is_empty() {
                    if let Some(&(disp_client, disp_tx)) = disputed_txs.get(i % disputed_txs.len()) {
                        process_chargeback(processor, disp_client, disp_tx);
                    }
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

#[hotpath::measure]
fn process_resolve(
    processor: &mut TransactionProcessor<
        FixedPoint,
        ConcurrentAccountManager<FixedPoint>,
        ConcurrentTransactionStore<FixedPoint>,
    >,
    client_id: u16,
    tx_id: u32,
) {
    let _ = processor.process_transaction(Transaction::Resolve { client_id, tx_id });
}

#[hotpath::measure]
fn process_chargeback(
    processor: &mut TransactionProcessor<
        FixedPoint,
        ConcurrentAccountManager<FixedPoint>,
        ConcurrentTransactionStore<FixedPoint>,
    >,
    client_id: u16,
    tx_id: u32,
) {
    let _ = processor.process_transaction(Transaction::Chargeback { client_id, tx_id });
}
