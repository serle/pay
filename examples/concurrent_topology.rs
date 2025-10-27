//! Example: Concurrent Stream Topology
//!
//! This example demonstrates processing multiple CSV files concurrently using
//! StreamCombiner.merge(). All streams are polled concurrently and transactions
//! are processed in the order they become available across all streams.
//!
//! Use case: When order doesn't matter
//! - Multiple independent data sources (different regions, branches, etc.)
//! - Maximizing throughput when processing multiple files
//! - Real-time processing of multiple concurrent data feeds
//!
//! Note: Unlike sequential processing, transactions from all files are interleaved.
//! The final result is the same, but the processing order is non-deterministic.
//!
//! Usage:
//!   cargo run --example concurrent_topology -- source_a.csv source_b.csv source_c.csv
//!
//! Or create test files:
//!   echo -e "type,client,tx,amount\ndeposit,1,1,100.0\ndeposit,1,2,50.0" > /tmp/source_a.csv
//!   echo -e "type,client,tx,amount\ndeposit,2,3,200.0\ndeposit,2,4,75.0" > /tmp/source_b.csv
//!   echo -e "type,client,tx,amount\ndeposit,3,5,150.0\ndeposit,3,6,25.0" > /tmp/source_c.csv
//!   cargo run --example concurrent_topology -- /tmp/source_a.csv /tmp/source_b.csv /tmp/source_c.csv

use std::env;
use std::sync::Arc;

use pay::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file1.csv> [file2.csv] [file3.csv] ...", args[0]);
        eprintln!();
        eprintln!("Example with test data (independent sources):");
        eprintln!("  echo -e \"type,client,tx,amount\\ndeposit,1,1,100.0\\ndeposit,1,2,50.0\" > /tmp/source_a.csv");
        eprintln!("  echo -e \"type,client,tx,amount\\ndeposit,2,3,200.0\\ndeposit,2,4,75.0\" > /tmp/source_b.csv");
        eprintln!("  echo -e \"type,client,tx,amount\\ndeposit,3,5,150.0\\ndeposit,3,6,25.0\" > /tmp/source_c.csv");
        eprintln!("  {} /tmp/source_a.csv /tmp/source_b.csv /tmp/source_c.csv", args[0]);
        std::process::exit(1);
    }

    let input_files = &args[1..];

    eprintln!("=== Concurrent Stream Topology Example ===");
    eprintln!("Processing {} files concurrently:", input_files.len());
    for (i, path) in input_files.iter().enumerate() {
        eprintln!("  {}. {}", i + 1, path);
    }
    eprintln!();

    // Create shared storage
    let account_manager = Arc::new(ConcurrentAccountManager::<FixedPoint>::new());
    let transaction_store = Arc::new(ConcurrentTransactionStore::<FixedPoint>::new());

    // Build concurrent topology using StreamProcessor
    let mut processor = StreamProcessor::new(account_manager.clone(), transaction_store, SkipErrors);

    for input_path in input_files {
        // Create CSV transaction stream from file
        let csv_stream = CsvTransactionStream::<FixedPoint>::from_file(input_path)
            .await
            .map_err(|e| format!("Failed to open {}: {}", input_path, e))?;

        // Add to processor
        processor = processor.add_stream(csv_stream);
    }

    // Merge streams together (concurrent processing with single shard)
    eprintln!("Topology: Concurrent merge (streams processed simultaneously)");
    eprintln!("  → All files are read concurrently");
    eprintln!("  → Transactions processed in arrival order");
    eprintln!("  → Maximizes throughput");
    eprintln!("  → Order is non-deterministic but results are consistent");
    eprintln!();

    // Process the concurrent topology
    eprintln!("Processing transactions concurrently...");
    let results = processor
        .with_stream_combinator(StreamCombinator::Merge)
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
