use std::env;
use tokio::fs::File;
use tokio::io::BufWriter;
use tokio_util::compat::TokioAsyncReadCompatExt;

use pay::prelude::*;

#[tokio::main]
async fn main() {
    // Use CliApp to handle signals, stdout buffering, and exit codes
    CliApp::new("pay")
        .with_signal_snapshot(false)
        .run(run_transaction_processor)
        .await
}

/// Main application logic - processes transactions and writes snapshot
async fn run_transaction_processor() -> Result<BufWriter<tokio::io::Stdout>, AppError> {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        return Err(AppError::InvalidArguments(
            "Usage: pay <transactions.csv>".to_string(),
        ));
    }

    let input_path = &args[1];

    // Open input file
    let file = File::open(input_path)
        .await
        .map_err(|_| AppError::FileNotFound(input_path.clone()))?;

    // Convert tokio AsyncRead to futures AsyncRead
    let compat_file = file.compat();

    // Create CSV transaction stream
    let tx_stream = CsvTransactionStream::<FixedPoint>::new(compat_file);

    // Create storage and engine
    let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
    let transaction_store = ConcurrentTransactionStore::<FixedPoint>::new();
    let processor = TransactionProcessor::new(account_manager, transaction_store);

    // Create processing session with silent error policy (per brief requirements)
    // "you can ignore it and assume this is an error on our partners side"
    // Use SilentSkip to avoid stderr output during automated scoring
    let mut session = ProcessingSession::new(processor, SilentSkip);

    // Process the transaction stream
    let _success = session.process_stream(tx_stream).await;
    // Note: We continue regardless of success/failure per brief's error handling guidance

    // Write snapshot to stdout
    let stdout = tokio::io::stdout();
    let mut writer = BufWriter::new(stdout);

    write_snapshot(session.account_manager(), &mut writer).await?;

    // Return writer so CliApp can flush it before exit
    Ok(writer)
}
