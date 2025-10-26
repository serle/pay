use pay::prelude::*;
use std::sync::Arc;
use tokio::runtime::Builder;

/// High-contention hotpath profiling with Zipf distribution
///
/// Profiles realistic access patterns where 20% of clients get 80% of traffic.
/// This scenario stresses account-level locking and contention.
///
/// Run with: cargo run --release --bin hotpath_high_contention --features profiling
#[hotpath::main]
fn main() {
    println!("=== High Contention Hotpath Profile (Zipf Distribution) ===");
    println!("Workload: 1M transactions with 80/20 access pattern");
    println!("Configuration: 100 concurrent streams, 20% of clients get 80% of traffic");
    println!();

    // Create runtime with 8 threads
    let runtime = Builder::new_multi_thread()
        .worker_threads(8)
        .build()
        .unwrap();

    println!("Starting profiled execution...");
    println!();

    // Run profiled workload
    runtime.block_on(async {
        run_zipf_workload().await;
    });

    println!();
    println!("Profiling complete. Results show contention overhead with realistic access patterns.");
}

#[hotpath::measure]
async fn run_zipf_workload() {
    let num_streams = 100;
    let transactions_per_stream = 10_000;

    // Zipf distribution: 20 hot clients, 100 total clients
    let hot_clients = 20u16;
    let total_clients = 100u16;

    // Shared state
    let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
    let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

    println!("Spawning {} concurrent streams with zipf distribution...", num_streams);

    // Spawn concurrent tasks
    let mut handles = Vec::new();

    for stream_id in 0..num_streams {
        let acc_mgr = Arc::clone(&account_manager);
        let tx_store = Arc::clone(&transaction_store);

        let handle = tokio::spawn(async move {
            process_stream(
                stream_id,
                transactions_per_stream,
                acc_mgr,
                tx_store,
                hot_clients,
                total_clients,
            )
            .await
        });

        handles.push(handle);
    }

    // Wait for all streams to complete
    println!(
        "Processing {} total transactions...",
        num_streams * transactions_per_stream
    );

    for (i, handle) in handles.into_iter().enumerate() {
        handle.await.unwrap();

        if (i + 1) % 10 == 0 {
            println!("Completed {} / {} streams", i + 1, num_streams);
        }
    }

    println!("All streams completed");
}

#[hotpath::measure]
async fn process_stream(
    stream_id: usize,
    num_transactions: usize,
    account_manager: Arc<ConcurrentAccountManager<FixedPoint>>,
    transaction_store: Arc<ConcurrentTransactionStore<FixedPoint>>,
    hot_clients: u16,
    total_clients: u16,
) {
    let mut processor = TransactionProcessor::new(
        Arc::clone(&account_manager),
        Arc::clone(&transaction_store),
    );

    let base_tx_id = (stream_id * num_transactions) as u32;
    let mut tx_id = base_tx_id;
    let mut deposited_txs = Vec::new();

    for i in 0..num_transactions {
        // Zipf distribution: 80% chance to hit hot clients (20% of total)
        let client_id = if i % 5 < 4 {
            // Hot clients (80% of traffic)
            (i % hot_clients as usize) as u16 + 1
        } else {
            // Cold clients (20% of traffic)
            hot_clients + ((i % (total_clients - hot_clients) as usize) as u16) + 1
        };

        let tx_type = i % 10;

        match tx_type {
            0..=5 => {
                // Deposit (60%)
                process_deposit(&mut processor, client_id, tx_id, i, &mut deposited_txs);
                tx_id += 1;
            }
            6..=8 => {
                // Withdrawal (30%)
                if !deposited_txs.is_empty() {
                    process_withdrawal(&mut processor, client_id, tx_id, i);
                    tx_id += 1;
                }
            }
            9 => {
                // Dispute (10%)
                if let Some(&(dep_client, dep_tx)) = deposited_txs.get(i % deposited_txs.len()) {
                    if dep_client == client_id {
                        process_dispute(&mut processor, client_id, dep_tx);
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}

#[hotpath::measure]
fn process_deposit(
    processor: &mut TransactionProcessor<
        FixedPoint,
        Arc<ConcurrentAccountManager<FixedPoint>>,
        Arc<ConcurrentTransactionStore<FixedPoint>>,
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
        Arc<ConcurrentAccountManager<FixedPoint>>,
        Arc<ConcurrentTransactionStore<FixedPoint>>,
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
        Arc<ConcurrentAccountManager<FixedPoint>>,
        Arc<ConcurrentTransactionStore<FixedPoint>>,
    >,
    client_id: u16,
    tx_id: u32,
) {
    let _ = processor.process_transaction(Transaction::Dispute { client_id, tx_id });
}
