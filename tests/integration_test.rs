use futures::io::Cursor;
use pay::prelude::*;

/// Helper to process CSV data and return the output as a string
async fn process_csv(input: &str) -> String {
    // Create CSV stream with owned data
    let reader = Cursor::new(input.to_string().into_bytes());
    let tx_stream = CsvTransactionStream::<FixedPoint>::new(reader);

    // Create storage and engine
    let account_manager = ConcurrentAccountManager::<FixedPoint>::new();
    let store = ConcurrentTransactionStore::new();
    let processor = TransactionProcessor::new(account_manager, store);

    // Create processing session with permissive error policy
    let mut session = ProcessingSession::new(processor, SilentSkip);

    // Process transactions
    session.process_stream(tx_stream).await;

    // Write snapshot to buffer
    let mut output = Vec::new();
    write_snapshot(session.account_manager(), &mut output)
        .await
        .expect("Failed to write snapshot");

    String::from_utf8(output).expect("Invalid UTF-8 in output")
}

#[tokio::test]
async fn simple_deposits_and_withdrawals() {
    let input = "\
type,client,tx,amount
deposit,1,1,1.0
deposit,2,2,2.0
deposit,1,3,2.0
withdrawal,1,4,1.5
withdrawal,2,5,3.0
";

    let output = process_csv(input).await;

    // Parse output
    assert!(output.contains("client,available,held,total,locked"));

    // Client 1: deposits 1.0 + 2.0 = 3.0, withdraws 1.5 = 1.5 available
    assert!(output.contains("1,1.5000,0.0000,1.5000,false"));

    // Client 2: deposits 2.0, tries to withdraw 3.0 (should fail) = 2.0 available
    assert!(output.contains("2,2.0000,0.0000,2.0000,false"));
}

#[tokio::test]
async fn dispute_and_resolve() {
    let input = "\
type,client,tx,amount
deposit,1,1,10.0
deposit,1,2,5.0
dispute,1,1,
resolve,1,1,
";

    let output = process_csv(input).await;

    // After dispute and resolve: available = 5.0 + 10.0 = 15.0, held = 0
    assert!(output.contains("1,15.0000,0.0000,15.0000,false"));
}

#[tokio::test]
async fn dispute_and_chargeback() {
    let input = "\
type,client,tx,amount
deposit,1,1,10.0
deposit,1,2,5.0
dispute,1,1,
chargeback,1,1,
";

    let output = process_csv(input).await;

    // After chargeback: available = 5.0, held = 0, total = 5.0, locked = true
    assert!(output.contains("1,5.0000,0.0000,5.0000,true"));
}

#[tokio::test]
async fn multiple_clients() {
    let input = "\
type,client,tx,amount
deposit,1,1,100.0
deposit,2,2,200.0
deposit,3,3,300.0
withdrawal,2,4,50.0
";

    let output = process_csv(input).await;

    assert!(output.contains("1,100.0000,0.0000,100.0000,false"));
    assert!(output.contains("2,150.0000,0.0000,150.0000,false"));
    assert!(output.contains("3,300.0000,0.0000,300.0000,false"));
}

#[tokio::test]
async fn insufficient_funds_ignored() {
    let input = "\
type,client,tx,amount
deposit,1,1,50.0
withdrawal,1,2,100.0
deposit,1,3,25.0
";

    let output = process_csv(input).await;

    // Withdrawal should fail, so: 50.0 + 25.0 = 75.0
    assert!(output.contains("1,75.0000,0.0000,75.0000,false"));
}

#[tokio::test]
async fn dispute_nonexistent_transaction_ignored() {
    let input = "\
type,client,tx,amount
deposit,1,1,100.0
dispute,1,999,
";

    let output = process_csv(input).await;

    // Invalid dispute should be ignored
    assert!(output.contains("1,100.0000,0.0000,100.0000,false"));
}

#[tokio::test]
async fn decimal_precision() {
    let input = "\
type,client,tx,amount
deposit,1,1,1.2345
deposit,1,2,2.6789
";

    let output = process_csv(input).await;

    // Should preserve 4 decimal places: 1.2345 + 2.6789 = 3.9134
    assert!(output.contains("1,3.9134,0.0000,3.9134,false"));
}

#[tokio::test]
async fn empty_csv() {
    let input = "\
type,client,tx,amount
";

    let output = process_csv(input).await;

    // Should just have header
    assert_eq!(output.trim(), "client,available,held,total,locked");
}

#[tokio::test]
async fn transactions_on_locked_account() {
    let input = "\
type,client,tx,amount
deposit,1,1,100.0
dispute,1,1,
chargeback,1,1,
deposit,1,2,50.0
withdrawal,1,3,10.0
";

    let output = process_csv(input).await;

    // After chargeback, account is locked - subsequent transactions should be ignored
    // Available stays at 0 (chargeback removed the 100.0)
    assert!(output.contains("1,0.0000,0.0000,0.0000,true"));
}

#[tokio::test]
async fn held_funds_calculation() {
    let input = "\
type,client,tx,amount
deposit,1,1,50.0
deposit,1,2,30.0
dispute,1,1,
";

    let output = process_csv(input).await;

    // Total: 80.0, Held: 50.0, Available: 30.0
    assert!(output.contains("1,30.0000,50.0000,80.0000,false"));
}
