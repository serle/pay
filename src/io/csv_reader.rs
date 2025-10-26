use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use csv_async::AsyncReaderBuilder;
use futures::Stream;
use futures::io::AsyncRead;

use super::error::IoError;
use super::parse::RawTransactionRecord;
use crate::domain::{AmountType, Transaction};

/// Async stream of transactions from CSV input
pub struct CsvTransactionStream<R, A>
where
    R: AsyncRead + Unpin + Send + 'static,
    A: AmountType + Unpin,
{
    records: csv_async::DeserializeRecordsIntoStream<'static, R, RawTransactionRecord>,
    _phantom: PhantomData<A>,
}

impl<R, A> CsvTransactionStream<R, A>
where
    R: AsyncRead + Unpin + Send + 'static,
    A: AmountType + Unpin,
{
    /// Create a new transaction stream from an async reader
    pub fn new(reader: R) -> Self {
        let csv_reader = AsyncReaderBuilder::new()
            .trim(csv_async::Trim::All)
            .flexible(true)
            .create_deserializer(reader);

        let records = csv_reader.into_deserialize::<RawTransactionRecord>();

        Self {
            records,
            _phantom: PhantomData,
        }
    }
}

impl<R, A> Stream for CsvTransactionStream<R, A>
where
    R: AsyncRead + Unpin + Send + 'static,
    A: AmountType + Unpin,
{
    type Item = Result<Transaction<A>, IoError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match Pin::new(&mut this.records).poll_next(cx) {
            Poll::Ready(Some(Ok(raw_record))) => {
                let result = raw_record.parse::<A>();
                Poll::Ready(Some(result))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(IoError::from(e)))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
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
        let mut stream = CsvTransactionStream::<_, FixedPoint>::new(reader);

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
        let mut stream = CsvTransactionStream::<_, FixedPoint>::new(reader);

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
        let mut stream = CsvTransactionStream::<_, FixedPoint>::new(reader);

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
        let mut stream = CsvTransactionStream::<_, FixedPoint>::new(reader);

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
        let mut stream = CsvTransactionStream::<_, FixedPoint>::new(reader);

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
        let mut stream = CsvTransactionStream::<_, FixedPoint>::new(reader);

        let result = stream.next().await.unwrap();
        assert!(matches!(result, Err(IoError::InvalidAmount(_))));
    }

    #[tokio::test]
    async fn handles_empty_csv() {
        let csv_data = "\
type,client,tx,amount
";
        let reader = Cursor::new(csv_data.as_bytes());
        let mut stream = CsvTransactionStream::<_, FixedPoint>::new(reader);

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
        let stream = CsvTransactionStream::<_, FixedPoint>::new(reader);

        // Collect all transactions
        let transactions: Vec<_> = stream.collect().await;
        assert_eq!(transactions.len(), 5);
        assert!(transactions.iter().all(|r| r.is_ok()));
    }
}
