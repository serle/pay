use std::collections::HashSet;

use super::amount::AmountType;

/// Client account with private fields enforcing invariants
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientAccount<A: AmountType> {
    client_id: u16,
    available: A,
    held: A,
    locked: bool,
    disputed_transactions: HashSet<u32>,
}

impl<A: AmountType> ClientAccount<A> {
    /// Create a new account with zero balance
    pub fn new(client_id: u16) -> Self {
        Self {
            client_id,
            available: A::zero(),
            held: A::zero(),
            locked: false,
            disputed_transactions: HashSet::new(),
        }
    }

    /// Get the client ID
    pub fn client_id(&self) -> u16 {
        self.client_id
    }

    /// Get available funds
    pub fn available(&self) -> A {
        self.available
    }

    /// Get held funds
    pub fn held(&self) -> A {
        self.held
    }

    /// Get total funds (derived: available + held)
    pub fn total(&self) -> A {
        self.available + self.held
    }

    /// Check if account is locked
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Check if a transaction is disputed
    pub fn is_disputed(&self, tx_id: u32) -> bool {
        self.disputed_transactions.contains(&tx_id)
    }

    /// Get the number of disputed transactions
    pub fn disputed_count(&self) -> usize {
        self.disputed_transactions.len()
    }

    // Internal mutation methods for use by operations module
    pub(crate) fn set_available(&mut self, amount: A) {
        self.available = amount;
    }

    pub(crate) fn set_held(&mut self, amount: A) {
        self.held = amount;
    }

    pub(crate) fn lock(&mut self) {
        self.locked = true;
    }

    pub(crate) fn add_disputed(&mut self, tx_id: u32) -> bool {
        self.disputed_transactions.insert(tx_id)
    }

    pub(crate) fn remove_disputed(&mut self, tx_id: u32) -> bool {
        self.disputed_transactions.remove(&tx_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::amount::FixedPoint;

    #[test]
    fn new_account_has_zero_balance() {
        let account = ClientAccount::<FixedPoint>::new(1);

        assert_eq!(account.client_id(), 1);
        assert_eq!(account.available(), FixedPoint::zero());
        assert_eq!(account.held(), FixedPoint::zero());
        assert_eq!(account.total(), FixedPoint::zero());
        assert!(!account.is_locked());
    }

    #[test]
    fn total_equals_available_plus_held() {
        let mut account = ClientAccount::<FixedPoint>::new(1);
        account.set_available(FixedPoint::from_raw(10_000));
        account.set_held(FixedPoint::from_raw(5_000));

        assert_eq!(account.total(), FixedPoint::from_raw(15_000));
    }

    #[test]
    fn getters_return_correct_values() {
        let mut account = ClientAccount::<FixedPoint>::new(42);
        account.set_available(FixedPoint::from_raw(1_000));
        account.set_held(FixedPoint::from_raw(500));
        account.lock();

        assert_eq!(account.client_id(), 42);
        assert_eq!(account.available(), FixedPoint::from_raw(1_000));
        assert_eq!(account.held(), FixedPoint::from_raw(500));
        assert_eq!(account.total(), FixedPoint::from_raw(1_500));
        assert!(account.is_locked());
    }

    #[test]
    fn account_can_be_cloned() {
        let account = ClientAccount::<FixedPoint>::new(1);
        let cloned = account.clone();

        assert_eq!(account, cloned);
    }

    #[test]
    fn lock_sets_locked_flag() {
        let mut account = ClientAccount::<FixedPoint>::new(1);
        assert!(!account.is_locked());

        account.lock();
        assert!(account.is_locked());
    }

    #[test]
    fn new_account_has_no_disputes() {
        let account = ClientAccount::<FixedPoint>::new(1);
        assert!(!account.is_disputed(100));
        assert_eq!(account.disputed_count(), 0);
    }

    #[test]
    fn add_disputed_tracks_transaction() {
        let mut account = ClientAccount::<FixedPoint>::new(1);

        assert!(account.add_disputed(100));
        assert!(account.is_disputed(100));
        assert_eq!(account.disputed_count(), 1);
    }

    #[test]
    fn add_disputed_returns_false_if_already_disputed() {
        let mut account = ClientAccount::<FixedPoint>::new(1);

        account.add_disputed(100);
        assert!(!account.add_disputed(100)); // Already disputed
        assert_eq!(account.disputed_count(), 1); // Still only one
    }

    #[test]
    fn remove_disputed_removes_transaction() {
        let mut account = ClientAccount::<FixedPoint>::new(1);

        account.add_disputed(100);
        assert!(account.is_disputed(100));

        assert!(account.remove_disputed(100));
        assert!(!account.is_disputed(100));
        assert_eq!(account.disputed_count(), 0);
    }

    #[test]
    fn remove_disputed_returns_false_if_not_disputed() {
        let mut account = ClientAccount::<FixedPoint>::new(1);

        assert!(!account.remove_disputed(999)); // Never disputed
    }

    #[test]
    fn account_can_track_multiple_disputes() {
        let mut account = ClientAccount::<FixedPoint>::new(1);

        account.add_disputed(1);
        account.add_disputed(2);
        account.add_disputed(3);

        assert!(account.is_disputed(1));
        assert!(account.is_disputed(2));
        assert!(account.is_disputed(3));
        assert!(!account.is_disputed(4));
        assert_eq!(account.disputed_count(), 3);
    }

    #[test]
    fn dispute_cycle() {
        let mut account = ClientAccount::<FixedPoint>::new(1);

        // Not disputed initially
        assert!(!account.is_disputed(1));

        // Add dispute
        account.add_disputed(1);
        assert!(account.is_disputed(1));

        // Remove dispute
        account.remove_disputed(1);
        assert!(!account.is_disputed(1));

        // Can dispute again
        account.add_disputed(1);
        assert!(account.is_disputed(1));
    }
}
