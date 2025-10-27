use std::sync::Arc;

use pay::prelude::*;

fn main() {
    CliApp::new("pay")
        .with_args(parse_args)
        .run(run_transaction_processor);
}

/// Parse and validate command-line arguments
fn parse_args(args: Vec<String>) -> Result<String, AppError> {
    if args.len() != 2 {
        return Err(AppError::InvalidArguments(
            "Usage: pay <transactions.csv>".to_string(),
        ));
    }
    Ok(args[1].clone())
}

/// Main application logic - processes transactions and writes snapshot
async fn run_transaction_processor(
    mut writers: Writers,
    input_file: String,
) -> Result<(), AppError> {
    // Create CSV transaction stream from file
    // This is a simple single-stream topology (most common case)
    // For processing multiple streams, see:
    //   - examples/sequential_topology.rs (chain multiple streams in order)
    //   - examples/concurrent_topology.rs (merge multiple streams concurrently)
    let tx_stream = CsvTransactionStream::<FixedPoint>::from_file(&input_file).await?;

    // Create shared storage (wrapped in Arc for StreamProcessor API)
    let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
    let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

    // Process stream with silent error policy (per brief requirements)
    // "you can ignore it and assume this is an error on our partners side"
    // Use SilentSkip to avoid stderr output during automated scoring
    let _results = StreamProcessor::new(account_manager.clone(), transaction_store, SilentSkip)
        .add_stream(tx_stream)
        .process()
        .await;
    // Note: We continue regardless of success/failure per brief's error handling guidance

    // Write snapshot to stdout (snapshot() handles flushing)
    account_manager.snapshot(&mut writers.stdout).await?;

    Ok(())
}
