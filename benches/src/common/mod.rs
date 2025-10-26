use pay::prelude::*;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Generate a CSV dataset with the specified parameters
pub fn generate_csv_dataset(
    num_transactions: usize,
    num_clients: u16,
    deposit_ratio: f64,
    withdrawal_ratio: f64,
    dispute_ratio: f64,
) -> String {
    let mut csv = String::from("type,client,tx,amount\n");
    let mut tx_counter = 1u32;
    let mut deposited_txs = Vec::new();

    for i in 0..num_transactions {
        let client_id = ((i % num_clients as usize) + 1) as u16;
        let rand_val = (i as f64 / num_transactions as f64);

        if rand_val < deposit_ratio {
            // Deposit
            let amount = format!("{}.{:04}", (i % 1000) + 1, i % 10000);
            csv.push_str(&format!("deposit,{},{},{}\n", client_id, tx_counter, amount));
            deposited_txs.push((client_id, tx_counter));
            tx_counter += 1;
        } else if rand_val < deposit_ratio + withdrawal_ratio {
            // Withdrawal (only if we have deposits for this client)
            if !deposited_txs.is_empty() {
                let amount = format!("{}.{:04}", (i % 100) + 1, i % 10000);
                csv.push_str(&format!("withdrawal,{},{},{}\n", client_id, tx_counter, amount));
                tx_counter += 1;
            }
        } else if rand_val < deposit_ratio + withdrawal_ratio + dispute_ratio {
            // Dispute (reference a previous transaction)
            if let Some(&(dep_client, dep_tx)) = deposited_txs.get(i % deposited_txs.len()) {
                if dep_client == client_id {
                    csv.push_str(&format!("dispute,{},{},\n", client_id, dep_tx));
                }
            }
        }
    }

    csv
}

/// Generate CSV dataset and write to file
pub fn generate_csv_file<P: AsRef<Path>>(
    path: P,
    num_transactions: usize,
    num_clients: u16,
    deposit_ratio: f64,
    withdrawal_ratio: f64,
    dispute_ratio: f64,
) -> std::io::Result<()> {
    let csv = generate_csv_dataset(
        num_transactions,
        num_clients,
        deposit_ratio,
        withdrawal_ratio,
        dispute_ratio,
    );
    let mut file = File::create(path)?;
    file.write_all(csv.as_bytes())?;
    Ok(())
}

/// Create standard fixture datasets
pub fn create_standard_fixtures() -> std::io::Result<()> {
    // Small dataset: 1K transactions, 100 clients, balanced workload
    generate_csv_file(
        "benches/fixtures/small_dataset.csv",
        1_000,
        100,
        0.5,   // 50% deposits
        0.3,   // 30% withdrawals
        0.1,   // 10% disputes
    )?;

    // Medium dataset: 100K transactions, 1K clients, deposit-heavy
    generate_csv_file(
        "benches/fixtures/medium_dataset.csv",
        100_000,
        1_000,
        0.7,   // 70% deposits
        0.2,   // 20% withdrawals
        0.05,  // 5% disputes
    )?;

    // Large dataset: 1M transactions, 10K clients, withdrawal-heavy
    generate_csv_file(
        "benches/fixtures/large_dataset.csv",
        1_000_000,
        10_000,
        0.4,   // 40% deposits
        0.5,   // 50% withdrawals
        0.05,  // 5% disputes
    )?;

    // Contention dataset: 10K transactions, single client (worst case)
    generate_csv_file(
        "benches/fixtures/high_contention.csv",
        10_000,
        1,     // Single client - maximum contention
        0.6,   // 60% deposits
        0.3,   // 30% withdrawals
        0.05,  // 5% disputes
    )?;

    // Dispute-heavy dataset: stress test for dispute resolution
    generate_csv_file(
        "benches/fixtures/dispute_heavy.csv",
        50_000,
        500,
        0.5,   // 50% deposits
        0.1,   // 10% withdrawals
        0.3,   // 30% disputes
    )?;

    Ok(())
}

/// Setup helper for creating a processor with both account manager and transaction store
pub fn setup_processor() -> TransactionProcessor<FixedPoint, ConcurrentAccountManager<FixedPoint>, ConcurrentTransactionStore<FixedPoint>> {
    let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
    let transaction_store = ConcurrentTransactionStore::<FixedPoint>::new();
    TransactionProcessor::new(account_manager, transaction_store)
}

/// Create a batch of deposit transactions for testing
pub fn create_deposit_batch(start_tx_id: u32, count: usize, client_id: u16) -> Vec<Transaction<FixedPoint>> {
    (0..count)
        .map(|i| Transaction::Deposit {
            client_id,
            tx_id: start_tx_id + i as u32,
            amount: FixedPoint::from_raw(10_000), // 1.0000
        })
        .collect()
}

/// Create a batch of transactions with mixed types
pub fn create_mixed_batch(
    start_tx_id: u32,
    count: usize,
    num_clients: u16,
) -> Vec<Transaction<FixedPoint>> {
    let mut transactions = Vec::with_capacity(count);
    let mut tx_id = start_tx_id;

    for i in 0..count {
        let client_id = ((i % num_clients as usize) + 1) as u16;
        let tx_type = i % 10;

        let tx = match tx_type {
            0..=6 => Transaction::Deposit {
                client_id,
                tx_id,
                amount: FixedPoint::from_raw(((i % 1000) + 1) as i64 * 10_000),
            },
            7..=9 => Transaction::Withdrawal {
                client_id,
                tx_id,
                amount: FixedPoint::from_raw(((i % 100) + 1) as i64 * 10_000),
            },
            _ => unreachable!(),
        };

        transactions.push(tx);
        tx_id += 1;
    }

    transactions
}
