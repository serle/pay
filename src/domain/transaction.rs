use super::amount::AmountType;

/// Transaction types with separate variants for type safety
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transaction<A: AmountType> {
    Deposit {
        client_id: u16,
        tx_id: u32,
        amount: A,
    },
    Withdrawal {
        client_id: u16,
        tx_id: u32,
        amount: A,
    },
    Dispute {
        client_id: u16,
        tx_id: u32,
    },
    Resolve {
        client_id: u16,
        tx_id: u32,
    },
    Chargeback {
        client_id: u16,
        tx_id: u32,
    },
}

impl<A: AmountType> Transaction<A> {
    /// Get the client ID for this transaction
    pub fn client_id(&self) -> u16 {
        match self {
            Self::Deposit { client_id, .. } => *client_id,
            Self::Withdrawal { client_id, .. } => *client_id,
            Self::Dispute { client_id, .. } => *client_id,
            Self::Resolve { client_id, .. } => *client_id,
            Self::Chargeback { client_id, .. } => *client_id,
        }
    }

    /// Get the transaction ID
    pub fn tx_id(&self) -> u32 {
        match self {
            Self::Deposit { tx_id, .. } => *tx_id,
            Self::Withdrawal { tx_id, .. } => *tx_id,
            Self::Dispute { tx_id, .. } => *tx_id,
            Self::Resolve { tx_id, .. } => *tx_id,
            Self::Chargeback { tx_id, .. } => *tx_id,
        }
    }
}

/// Immutable record of a transaction (for dispute resolution)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionRecord<A: AmountType> {
    pub client_id: u16,
    pub amount: A,
}

impl<A: AmountType> TransactionRecord<A> {
    /// Create a new transaction record
    pub fn new(client_id: u16, amount: A) -> Self {
        Self {
            client_id,
            amount,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::amount::FixedPoint;

    #[test]
    fn deposit_has_amount() {
        let tx = Transaction::Deposit {
            client_id: 1,
            tx_id: 100,
            amount: FixedPoint::from_raw(10_000),
        };

        assert_eq!(tx.client_id(), 1);
        assert_eq!(tx.tx_id(), 100);
    }

    #[test]
    fn withdrawal_has_amount() {
        let tx = Transaction::Withdrawal {
            client_id: 2,
            tx_id: 200,
            amount: FixedPoint::from_raw(5_000),
        };

        assert_eq!(tx.client_id(), 2);
        assert_eq!(tx.tx_id(), 200);
    }

    #[test]
    fn dispute_no_amount() {
        let tx = Transaction::<FixedPoint>::Dispute {
            client_id: 1,
            tx_id: 100,
        };

        assert_eq!(tx.client_id(), 1);
        assert_eq!(tx.tx_id(), 100);
    }

    #[test]
    fn resolve_no_amount() {
        let tx = Transaction::<FixedPoint>::Resolve {
            client_id: 1,
            tx_id: 100,
        };

        assert_eq!(tx.client_id(), 1);
        assert_eq!(tx.tx_id(), 100);
    }

    #[test]
    fn chargeback_no_amount() {
        let tx = Transaction::<FixedPoint>::Chargeback {
            client_id: 1,
            tx_id: 100,
        };

        assert_eq!(tx.client_id(), 1);
        assert_eq!(tx.tx_id(), 100);
    }

    #[test]
    fn transaction_record_creation() {
        let record = TransactionRecord::new(1, FixedPoint::from_raw(10_000));

        assert_eq!(record.client_id, 1);
        assert_eq!(record.amount, FixedPoint::from_raw(10_000));
    }

    #[test]
    fn transaction_record_is_immutable_and_clonable() {
        let record = TransactionRecord::new(1, FixedPoint::from_raw(10_000));
        let cloned = record.clone();

        assert_eq!(record, cloned);
        assert_eq!(cloned.client_id, 1);
        assert_eq!(cloned.amount, FixedPoint::from_raw(10_000));
    }

    #[test]
    fn transaction_variants_are_distinct() {
        let deposit = Transaction::Deposit {
            client_id: 1,
            tx_id: 1,
            amount: FixedPoint::from_raw(1000),
        };

        let withdrawal = Transaction::Withdrawal {
            client_id: 1,
            tx_id: 1,
            amount: FixedPoint::from_raw(1000),
        };

        assert_ne!(deposit, withdrawal);
    }
}
