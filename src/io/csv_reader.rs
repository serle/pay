use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};

use csv_async::AsyncReaderBuilder;
use futures::{Stream, StreamExt};
use futures::io::AsyncRead;
use tokio::fs::File;
use tokio_util::compat::TokioAsyncReadCompatExt;

use super::error::IoError;
use super::parse::RawTransactionRecord;
use crate::domain::{AmountType, Transaction};

/// Async stream of transactions from CSV input
pub struct CsvTransactionStream<A>
where
    A: AmountType + Unpin,
{
    inner: Pin<Box<dyn Stream<Item = Result<Transaction<A>, IoError>> + Send>>,
}

impl<A> CsvTransactionStream<A>
where
    A: AmountType + Unpin,
{
    /// Create a new transaction stream from an async reader
    pub fn new<R>(reader: R) -> Self
    where
        R: AsyncRead + Unpin + Send + 'static,
    {
        let csv_reader = AsyncReaderBuilder::new()
            .trim(csv_async::Trim::All)
            .flexible(true)
            .create_deserializer(reader);

        let stream = csv_reader
            .into_deserialize::<RawTransactionRecord>()
            .map(|result| {
                result
                    .map_err(IoError::from)
                    .and_then(|raw| raw.parse::<A>())
            });

        Self {
            inner: Box::pin(stream),
        }
    }

    /// Create a new transaction stream from a file path
    ///
    /// Opens the file asynchronously and creates a CSV stream.
    /// This is a convenience method that handles tokio-futures compatibility internally.
    ///
    /// # Example
    /// ```rust,ignore
    /// let stream = CsvTransactionStream::<FixedPoint>::from_file("transactions.csv").await?;
    /// ```
    pub async fn from_file(path: impl AsRef<Path>) -> Result<Self, IoError> {
        let file = File::open(path.as_ref()).await?;
        Ok(Self::new(file.compat()))
    }
}

impl<A> Stream for CsvTransactionStream<A>
where
    A: AmountType + Unpin,
{
    type Item = Result<Transaction<A>, IoError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::FixedPoint;
    use futures::StreamExt;
    use futures::io::Cursor;

    #[tokio::test]
    async fn reads_valid_csv_stream() {
        let csv_data = "\
type,client,tx,amount
deposit,1,1,1.0
deposit,2,2,2.0
withdrawal,1,3,0.5
dispute,1,1,
resolve,1,1,
";
        let reader = Cursor::new(csv_data.as_bytes());
        let mut stream = CsvTransactionStream::<FixedPoint>::new(reader);

        // First transaction: deposit
        let tx1 = stream.next().await.unwrap().unwrap();
        match tx1 {
            Transaction::Deposit {
                client_id,
                tx_id,
                amount,
            } => {
                assert_eq!(client_id, 1);
                assert_eq!(tx_id, 1);
                assert_eq!(amount, FixedPoint::from_raw(10_000));
            }
            _ => panic!("Expected Deposit"),
        }

        // Second transaction: deposit
        let tx2 = stream.next().await.unwrap().unwrap();
        match tx2 {
            Transaction::Deposit {
                client_id,
                tx_id,
                amount,
            } => {
                assert_eq!(client_id, 2);
                assert_eq!(tx_id, 2);
                assert_eq!(amount, FixedPoint::from_raw(20_000));
            }
            _ => panic!("Expected Deposit"),
        }

        // Third transaction: withdrawal
        let tx3 = stream.next().await.unwrap().unwrap();
        match tx3 {
            Transaction::Withdrawal {
                client_id,
                tx_id,
                amount,
            } => {
                assert_eq!(client_id, 1);
                assert_eq!(tx_id, 3);
                assert_eq!(amount, FixedPoint::from_raw(5_000));
            }
            _ => panic!("Expected Withdrawal"),
        }

        // Fourth transaction: dispute
        let tx4 = stream.next().await.unwrap().unwrap();
        assert!(matches!(
            tx4,
            Transaction::Dispute {
                client_id: 1,
                tx_id: 1
            }
        ));

        // Fifth transaction: resolve
        let tx5 = stream.next().await.unwrap().unwrap();
        assert!(matches!(
            tx5,
            Transaction::Resolve {
                client_id: 1,
                tx_id: 1
            }
        ));

        // End of stream
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn handles_whitespace() {
        let csv_data = "\
type,client,tx,amount
  deposit  ,  1  ,  1  ,  1.5000
";
        let reader = Cursor::new(csv_data.as_bytes());
        let mut stream = CsvTransactionStream::<FixedPoint>::new(reader);

        let tx = stream.next().await.unwrap().unwrap();
        match tx {
            Transaction::Deposit {
                client_id,
                tx_id,
                amount,
            } => {
                assert_eq!(client_id, 1);
                assert_eq!(tx_id, 1);
                assert_eq!(amount, FixedPoint::from_raw(15_000));
            }
            _ => panic!("Expected Deposit"),
        }
    }

    #[tokio::test]
    async fn handles_missing_amount_for_dispute() {
        let csv_data = "\
type,client,tx,amount
dispute,1,1,
";
        let reader = Cursor::new(csv_data.as_bytes());
        let mut stream = CsvTransactionStream::<FixedPoint>::new(reader);

        let tx = stream.next().await.unwrap().unwrap();
        assert!(matches!(tx, Transaction::Dispute { .. }));
    }

    #[tokio::test]
    async fn returns_error_for_invalid_transaction_type() {
        let csv_data = "\
type,client,tx,amount
invalid,1,1,1.0
";
        let reader = Cursor::new(csv_data.as_bytes());
        let mut stream = CsvTransactionStream::<FixedPoint>::new(reader);

        let result = stream.next().await.unwrap();
        assert!(matches!(result, Err(IoError::InvalidTransactionType(_))));
    }

    #[tokio::test]
    async fn returns_error_for_missing_required_amount() {
        let csv_data = "\
type,client,tx,amount
deposit,1,1,
";
        let reader = Cursor::new(csv_data.as_bytes());
        let mut stream = CsvTransactionStream::<FixedPoint>::new(reader);

        let result = stream.next().await.unwrap();
        assert!(matches!(result, Err(IoError::MissingField(_))));
    }

    #[tokio::test]
    async fn returns_error_for_invalid_amount() {
        let csv_data = "\
type,client,tx,amount
deposit,1,1,not_a_number
";
        let reader = Cursor::new(csv_data.as_bytes());
        let mut stream = CsvTransactionStream::<FixedPoint>::new(reader);

        let result = stream.next().await.unwrap();
        assert!(matches!(result, Err(IoError::InvalidAmount(_))));
    }

    #[tokio::test]
    async fn handles_empty_csv() {
        let csv_data = "\
type,client,tx,amount
";
        let reader = Cursor::new(csv_data.as_bytes());
        let mut stream = CsvTransactionStream::<FixedPoint>::new(reader);

        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn handles_all_transaction_types() {
        let csv_data = "\
type,client,tx,amount
deposit,1,1,1.0
withdrawal,1,2,0.5
dispute,1,1,
resolve,1,1,
chargeback,1,1,
";
        let reader = Cursor::new(csv_data.as_bytes());
        let stream = CsvTransactionStream::<FixedPoint>::new(reader);

        // Collect all transactions
        let transactions: Vec<_> = stream.collect().await;
        assert_eq!(transactions.len(), 5);
        assert!(transactions.iter().all(|r| r.is_ok()));
    }
}
