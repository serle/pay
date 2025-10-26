use serde::Deserialize;

use super::error::IoError;
use crate::domain::{AmountType, Transaction};

/// Raw CSV record as read from input
#[derive(Debug, Deserialize)]
pub struct RawTransactionRecord {
    #[serde(rename = "type")]
    pub tx_type: String,
    pub client: u16,
    pub tx: u32,
    pub amount: Option<String>,
}

impl RawTransactionRecord {
    /// Parse this raw record into a strongly-typed Transaction
    pub fn parse<A: AmountType>(self) -> Result<Transaction<A>, IoError> {
        let tx_type_lower = self.tx_type.trim().to_lowercase();

        match tx_type_lower.as_str() {
            "deposit" => {
                let amount_str = self.amount.ok_or_else(|| {
                    IoError::MissingField("amount required for deposit".to_string())
                })?;
                let amount = A::from_decimal_str(&amount_str)
                    .map_err(|_| IoError::InvalidAmount(amount_str))?;
                Ok(Transaction::Deposit {
                    client_id: self.client,
                    tx_id: self.tx,
                    amount,
                })
            }
            "withdrawal" => {
                let amount_str = self.amount.ok_or_else(|| {
                    IoError::MissingField("amount required for withdrawal".to_string())
                })?;
                let amount = A::from_decimal_str(&amount_str)
                    .map_err(|_| IoError::InvalidAmount(amount_str))?;
                Ok(Transaction::Withdrawal {
                    client_id: self.client,
                    tx_id: self.tx,
                    amount,
                })
            }
            "dispute" => Ok(Transaction::Dispute {
                client_id: self.client,
                tx_id: self.tx,
            }),
            "resolve" => Ok(Transaction::Resolve {
                client_id: self.client,
                tx_id: self.tx,
            }),
            "chargeback" => Ok(Transaction::Chargeback {
                client_id: self.client,
                tx_id: self.tx,
            }),
            _ => Err(IoError::InvalidTransactionType(self.tx_type)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::FixedPoint;

    #[test]
    fn parse_deposit() {
        let raw = RawTransactionRecord {
            tx_type: "deposit".to_string(),
            client: 1,
            tx: 100,
            amount: Some("1.5".to_string()),
        };

        let tx = raw.parse::<FixedPoint>().unwrap();
        match tx {
            Transaction::Deposit {
                client_id,
                tx_id,
                amount,
            } => {
                assert_eq!(client_id, 1);
                assert_eq!(tx_id, 100);
                assert_eq!(amount, FixedPoint::from_raw(15_000));
            }
            _ => panic!("Expected Deposit variant"),
        }
    }

    #[test]
    fn parse_withdrawal() {
        let raw = RawTransactionRecord {
            tx_type: "withdrawal".to_string(),
            client: 2,
            tx: 200,
            amount: Some("0.5000".to_string()),
        };

        let tx = raw.parse::<FixedPoint>().unwrap();
        match tx {
            Transaction::Withdrawal {
                client_id,
                tx_id,
                amount,
            } => {
                assert_eq!(client_id, 2);
                assert_eq!(tx_id, 200);
                assert_eq!(amount, FixedPoint::from_raw(5_000));
            }
            _ => panic!("Expected Withdrawal variant"),
        }
    }

    #[test]
    fn parse_dispute() {
        let raw = RawTransactionRecord {
            tx_type: "dispute".to_string(),
            client: 1,
            tx: 100,
            amount: None,
        };

        let tx = raw.parse::<FixedPoint>().unwrap();
        match tx {
            Transaction::Dispute { client_id, tx_id } => {
                assert_eq!(client_id, 1);
                assert_eq!(tx_id, 100);
            }
            _ => panic!("Expected Dispute variant"),
        }
    }

    #[test]
    fn parse_resolve() {
        let raw = RawTransactionRecord {
            tx_type: "resolve".to_string(),
            client: 1,
            tx: 100,
            amount: None,
        };

        let tx = raw.parse::<FixedPoint>().unwrap();
        match tx {
            Transaction::Resolve { client_id, tx_id } => {
                assert_eq!(client_id, 1);
                assert_eq!(tx_id, 100);
            }
            _ => panic!("Expected Resolve variant"),
        }
    }

    #[test]
    fn parse_chargeback() {
        let raw = RawTransactionRecord {
            tx_type: "chargeback".to_string(),
            client: 1,
            tx: 100,
            amount: None,
        };

        let tx = raw.parse::<FixedPoint>().unwrap();
        match tx {
            Transaction::Chargeback { client_id, tx_id } => {
                assert_eq!(client_id, 1);
                assert_eq!(tx_id, 100);
            }
            _ => panic!("Expected Chargeback variant"),
        }
    }

    #[test]
    fn parse_case_insensitive() {
        let raw = RawTransactionRecord {
            tx_type: "DEPOSIT".to_string(),
            client: 1,
            tx: 100,
            amount: Some("1.0".to_string()),
        };

        let tx = raw.parse::<FixedPoint>().unwrap();
        assert!(matches!(tx, Transaction::Deposit { .. }));
    }

    #[test]
    fn parse_whitespace_trimmed() {
        let raw = RawTransactionRecord {
            tx_type: " deposit ".to_string(),
            client: 1,
            tx: 100,
            amount: Some("1.0".to_string()),
        };

        let tx = raw.parse::<FixedPoint>().unwrap();
        assert!(matches!(tx, Transaction::Deposit { .. }));
    }

    #[test]
    fn parse_invalid_transaction_type() {
        let raw = RawTransactionRecord {
            tx_type: "invalid".to_string(),
            client: 1,
            tx: 100,
            amount: None,
        };

        let result = raw.parse::<FixedPoint>();
        assert!(matches!(result, Err(IoError::InvalidTransactionType(_))));
    }

    #[test]
    fn parse_deposit_missing_amount() {
        let raw = RawTransactionRecord {
            tx_type: "deposit".to_string(),
            client: 1,
            tx: 100,
            amount: None,
        };

        let result = raw.parse::<FixedPoint>();
        assert!(matches!(result, Err(IoError::MissingField(_))));
    }

    #[test]
    fn parse_withdrawal_missing_amount() {
        let raw = RawTransactionRecord {
            tx_type: "withdrawal".to_string(),
            client: 1,
            tx: 100,
            amount: None,
        };

        let result = raw.parse::<FixedPoint>();
        assert!(matches!(result, Err(IoError::MissingField(_))));
    }

    #[test]
    fn parse_invalid_amount_format() {
        let raw = RawTransactionRecord {
            tx_type: "deposit".to_string(),
            client: 1,
            tx: 100,
            amount: Some("not_a_number".to_string()),
        };

        let result = raw.parse::<FixedPoint>();
        assert!(matches!(result, Err(IoError::InvalidAmount(_))));
    }

    #[test]
    fn parse_amount_too_many_decimals() {
        let raw = RawTransactionRecord {
            tx_type: "deposit".to_string(),
            client: 1,
            tx: 100,
            amount: Some("1.123456".to_string()),
        };

        let result = raw.parse::<FixedPoint>();
        assert!(matches!(result, Err(IoError::InvalidAmount(_))));
    }
}
