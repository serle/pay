use tokio::io::AsyncWrite;

use super::error::IoError;
use crate::domain::AmountType;
use crate::storage::ClientAccountManager;

/// Write account snapshots to CSV format
pub async fn write_snapshot<A, M, W>(account_manager: &M, writer: W) -> Result<(), IoError>
where
    A: AmountType,
    M: ClientAccountManager<A>,
    W: AsyncWrite + Unpin + Send,
{
    account_manager.snapshot(writer).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FixedPoint, operations};
    use crate::storage::{ClientAccountEntry, ConcurrentAccountManager};

    #[tokio::test]
    async fn writes_empty_snapshot() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();
        let mut output = Vec::new();

        write_snapshot(&manager, &mut output).await.unwrap();

        let result = String::from_utf8(output).unwrap();
        assert_eq!(result, "client,available,held,total,locked\n");
    }

    #[tokio::test]
    async fn writes_single_account() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();

        // Create account with deposit
        {
            let mut entry = manager.entry(1).unwrap();
            entry
                .try_update(|acc| operations::apply_deposit(acc, FixedPoint::from_raw(15_000)))
                .unwrap();
        }

        let mut output = Vec::new();
        write_snapshot(&manager, &mut output).await.unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("client,available,held,total,locked"));
        assert!(result.contains("1,1.5000,0.0000,1.5000,false"));
    }

    #[tokio::test]
    async fn writes_multiple_accounts() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();

        // Create multiple accounts
        for i in 1..=3 {
            let mut entry = manager.entry(i).unwrap();
            entry
                .try_update(|acc| {
                    operations::apply_deposit(acc, FixedPoint::from_raw(i as i64 * 10_000))
                })
                .unwrap();
        }

        let mut output = Vec::new();
        write_snapshot(&manager, &mut output).await.unwrap();

        let result = String::from_utf8(output).unwrap();

        // Should have header + 3 accounts
        assert_eq!(result.lines().count(), 4);
        assert!(result.contains("1,1.0000,0.0000,1.0000,false"));
        assert!(result.contains("2,2.0000,0.0000,2.0000,false"));
        assert!(result.contains("3,3.0000,0.0000,3.0000,false"));
    }

    #[tokio::test]
    async fn writes_account_with_held_funds() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();

        // Create account and dispute
        {
            let mut entry = manager.entry(1).unwrap();
            entry
                .try_update(|acc| operations::apply_deposit(acc, FixedPoint::from_raw(10_000)))
                .unwrap();
            entry
                .try_update(|acc| operations::apply_dispute(acc, 1, FixedPoint::from_raw(5_000)))
                .unwrap();
        }

        let mut output = Vec::new();
        write_snapshot(&manager, &mut output).await.unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("1,0.5000,0.5000,1.0000,false"));
    }

    #[tokio::test]
    async fn writes_locked_account() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();

        // Create account and perform chargeback
        {
            let mut entry = manager.entry(1).unwrap();
            entry
                .try_update(|acc| operations::apply_deposit(acc, FixedPoint::from_raw(10_000)))
                .unwrap();
            entry
                .try_update(|acc| operations::apply_dispute(acc, 1, FixedPoint::from_raw(10_000)))
                .unwrap();
            entry
                .try_update(|acc| operations::apply_chargeback(acc, 1, FixedPoint::from_raw(10_000)))
                .unwrap();
        }

        let mut output = Vec::new();
        write_snapshot(&manager, &mut output).await.unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("1,0.0000,0.0000,0.0000,true"));
    }

    #[tokio::test]
    async fn decimal_precision_preserved() {
        let manager = ConcurrentAccountManager::<FixedPoint>::new();

        // Test various decimal amounts
        {
            let mut entry = manager.entry(1).unwrap();
            entry
                .try_update(|acc| operations::apply_deposit(acc, FixedPoint::from_raw(12_345))) // 1.2345
                .unwrap();
        }

        let mut output = Vec::new();
        write_snapshot(&manager, &mut output).await.unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("1.2345"));
    }
}
