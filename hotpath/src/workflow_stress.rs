use pay::prelude::*;
use std::sync::Arc;
use tokio::runtime::Builder;

/// Workflow-intensive hotpath profiling
///
/// Profiles complete dispute workflows: deposit → dispute → resolve/chargeback.
/// This scenario heavily stresses the transaction store with lookups.
///
/// Run with: cargo run --release --bin hotpath_workflow_stress --features profiling
#[hotpath::main]
fn main() {
    println!("=== Workflow Stress Hotpath Profile ===");
    println!("Workload: 500K transactions with 40% full workflows (dispute → resolve/chargeback)");
    println!("Configuration: 100 concurrent streams, stresses transaction store lookups");
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
        run_workflow_workload().await;
    });

    println!();
    println!("Profiling complete. Results show transaction store overhead under heavy workflow load.");
}

#[hotpath::measure]
async fn run_workflow_workload() {
    let num_streams = 100;
    let transactions_per_stream = 5_000;

    // Shared state
    let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
    let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

    println!("Spawning {} concurrent streams...", num_streams);

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

    let base_client_id = (stream_id * 100) as u16;
    let base_tx_id = (stream_id * num_transactions) as u32;

    let mut tx_id = base_tx_id;
    let mut deposited_txs = Vec::new();
    let mut disputed_txs = Vec::new();

    for i in 0..num_transactions {
        let client_id = base_client_id + (i % 100) as u16;
        let tx_type = i % 10;

        match tx_type {
            0..=3 => {
                // Deposit (40%)
                process_deposit(&mut processor, client_id, tx_id, i, &mut deposited_txs);
                tx_id += 1;
            }
            4..=5 => {
                // Withdrawal (20%)
                if !deposited_txs.is_empty() {
                    process_withdrawal(&mut processor, client_id, tx_id, i);
                    tx_id += 1;
                }
            }
            6..=7 => {
                // Dispute (20%) - creates disputed transactions
                if let Some(&(dep_client, dep_tx)) = deposited_txs.get(i % deposited_txs.len())
                    && dep_client == client_id
                {
                    process_dispute(&mut processor, dep_client, dep_tx);
                    disputed_txs.push((dep_client, dep_tx));
                }
            }
            8 => {
                // Resolve (10%) - completes half of disputed transactions
                if !disputed_txs.is_empty()
                    && let Some(&(disp_client, disp_tx)) = disputed_txs.get(i % disputed_txs.len())
                {
                    process_resolve(&mut processor, disp_client, disp_tx);
                }
            }
            9 => {
                // Chargeback (10%) - completes other half, locks accounts
                if !disputed_txs.is_empty()
                    && let Some(&(disp_client, disp_tx)) = disputed_txs.get(i % disputed_txs.len())
                {
                    process_chargeback(&mut processor, disp_client, disp_tx);
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

#[hotpath::measure]
fn process_resolve(
    processor: &mut TransactionProcessor<
        FixedPoint,
        Arc<ConcurrentAccountManager<FixedPoint>>,
        Arc<ConcurrentTransactionStore<FixedPoint>>,
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
        Arc<ConcurrentAccountManager<FixedPoint>>,
        Arc<ConcurrentTransactionStore<FixedPoint>>,
    >,
    client_id: u16,
    tx_id: u32,
) {
    let _ = processor.process_transaction(Transaction::Chargeback { client_id, tx_id });
}
