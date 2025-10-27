//! Example: Sequential Stream Topology
//!
//! This example demonstrates processing multiple CSV files in sequence using
//! StreamCombiner.chain(). Streams are processed one after another in the
//! order they are added.
//!
//! Use case: When order matters
//! - Main transactions file, followed by corrections, then adjustments
//! - Historical data processing (oldest to newest)
//! - Any scenario where later streams depend on earlier ones being processed first
//!
//! Usage:
//!   cargo run --example sequential_topology -- transactions.csv corrections.csv adjustments.csv
//!
//! Or create test files:
//!   echo -e "type,client,tx,amount\ndeposit,1,1,100.0" > /tmp/tx1.csv
//!   echo -e "type,client,tx,amount\ndeposit,1,2,50.0" > /tmp/tx2.csv
//!   echo -e "type,client,tx,amount\nwithdrawal,1,3,25.0" > /tmp/tx3.csv
//!   cargo run --example sequential_topology -- /tmp/tx1.csv /tmp/tx2.csv /tmp/tx3.csv

use std::env;
use std::sync::Arc;
use tokio::fs::File;
use tokio_util::compat::TokioAsyncReadCompatExt;

use pay::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file1.csv> [file2.csv] [file3.csv] ...", args[0]);
        eprintln!();
        eprintln!("Example with test data:");
        eprintln!("  echo -e \"type,client,tx,amount\\ndeposit,1,1,100.0\" > /tmp/tx1.csv");
        eprintln!("  echo -e \"type,client,tx,amount\\ndeposit,1,2,50.0\" > /tmp/tx2.csv");
        eprintln!("  echo -e \"type,client,tx,amount\\nwithdrawal,1,3,25.0\" > /tmp/tx3.csv");
        eprintln!("  {} /tmp/tx1.csv /tmp/tx2.csv /tmp/tx3.csv", args[0]);
        std::process::exit(1);
    }

    let input_files = &args[1..];

    eprintln!("=== Sequential Stream Topology Example ===");
    eprintln!("Processing {} files in sequence:", input_files.len());
    for (i, path) in input_files.iter().enumerate() {
        eprintln!("  {}. {}", i + 1, path);
    }
    eprintln!();

    // Create shared storage
    let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
    let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

    // Build sequential topology using StreamProcessor
    let mut processor = StreamProcessor::new(account_manager.clone(), transaction_store, SkipErrors);

    for input_path in input_files {
        // Open file
        let file = File::open(input_path).await.map_err(|e| {
            format!("Failed to open {}: {}", input_path, e)
        })?;

        // Convert tokio AsyncRead to futures AsyncRead
        let compat_file = file.compat();

        // Create CSV transaction stream
        let csv_stream = CsvTransactionStream::<FixedPoint>::new(compat_file);

        // Add to processor
        processor = processor.add_stream(csv_stream);
    }

    // Chain streams together (sequential processing with single shard)
    eprintln!("Topology: Sequential chain (streams processed in order)");
    eprintln!("  → File 1 processes completely");
    eprintln!("  → Then File 2 processes completely");
    eprintln!("  → Then File 3 processes completely");
    eprintln!("  → etc.");
    eprintln!();

    // Process the sequential topology
    eprintln!("Processing transactions...");
    let results = processor
        .with_stream_combinator(StreamCombinator::Chain)
        .process()
        .await;

    if results.all_succeeded() {
        eprintln!("✓ All streams processed successfully");
    } else {
        eprintln!("⚠ Processing had errors");
    }
    eprintln!();

    // Write snapshot to stdout
    eprintln!("Account snapshot:");
    eprintln!("=================");
    write_snapshot(&*account_manager, &mut tokio::io::stdout()).await?;

    Ok(())
}
