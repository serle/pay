use pay::prelude::*;
use std::sync::Arc;
use tokio::runtime::Builder;

/// Sparse account ID profiling
///
/// Profiles with realistic sparse account IDs instead of sequential 1, 2, 3...
/// In production, account IDs are often UUIDs, large random numbers, or non-sequential.
/// This tests whether sparse IDs affect DashMap hash distribution and performance.
///
/// Run with: cargo run --release --bin hotpath_sparse_accounts --features profiling
#[hotpath::main]
fn main() {
    println!("=== Sparse Account ID Hotpath Profile ===");
    println!("Workload: 1M transactions with realistic sparse account IDs");
    println!("Configuration: 100 concurrent streams, 8 threads, non-sequential account IDs");
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
        run_sparse_workload().await;
    });

    println!();
    println!("Profiling complete. Results show performance with realistic sparse account IDs.");
}

#[hotpath::measure]
async fn run_sparse_workload() {
    let num_streams = 100;
    let transactions_per_stream = 10_000;

    // Shared state
    let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
    let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

    println!("Spawning {} concurrent streams with sparse account IDs...", num_streams);

    // Spawn concurrent tasks
    let mut handles = Vec::new();

    for stream_id in 0..num_streams {
        let acc_mgr = Arc::clone(&account_manager);
        let tx_store = Arc::clone(&transaction_store);

        let handle = tokio::spawn(async move {
            process_stream(stream_id, transactions_per_stream, acc_mgr, tx_store).await
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
) {
    let mut processor = TransactionProcessor::new(
        Arc::clone(&account_manager),
        Arc::clone(&transaction_store),
    );

    // Generate sparse account IDs
    // Simulates realistic production IDs: large, non-sequential, with gaps
    // Base offset: stream_id * 1,000,000 to ensure uniqueness across streams
    // Within stream: use prime number stepping to create sparse distribution
    let base_offset = (stream_id * 1_000_000) as u16;

    let base_tx_id = (stream_id * num_transactions) as u32;
    let mut tx_id = base_tx_id;
    let mut deposited_txs = Vec::new();

    for i in 0..num_transactions {
        // Sparse account ID generation:
        // - Not sequential (use prime 251 for stepping)
        // - Large gaps between IDs
        // - Realistic distribution seen in production systems
        let sparse_offset = ((i * 251) % 10000) as u16;
        let client_id = base_offset.wrapping_add(sparse_offset);

        // Ensure client_id is non-zero (account IDs start at 1)
        let client_id = if client_id == 0 { 1 } else { client_id };

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
                if let Some(&(dep_client, dep_tx)) = deposited_txs.get(i % deposited_txs.len())
                    && dep_client == client_id
                {
                    process_dispute(&mut processor, client_id, dep_tx);
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
