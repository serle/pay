mod common;

fn main() {
    println!("Generating benchmark fixture datasets...");

    match common::create_standard_fixtures() {
        Ok(()) => println!("Successfully generated all fixture datasets in benches/fixtures/"),
        Err(e) => eprintln!("Error generating fixtures: {}", e),
    }

    println!("\nGenerated files:");
    println!("  - small_dataset.csv (1K transactions, 100 clients)");
    println!("  - medium_dataset.csv (100K transactions, 1K clients)");
    println!("  - large_dataset.csv (1M transactions, 10K clients)");
    println!("  - high_contention.csv (10K transactions, 1 client)");
    println!("  - dispute_heavy.csv (50K transactions, 500 clients)");
}
