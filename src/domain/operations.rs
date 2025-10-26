use super::account::ClientAccount;
use super::amount::AmountType;
use super::error::DomainError;

/// Apply a deposit to an account
pub fn apply_deposit<A: AmountType>(
    account: &mut ClientAccount<A>,
    amount: A,
) -> Result<(), DomainError> {
    // Validate amount is positive
    if amount <= A::zero() {
        return Err(DomainError::InvalidAmount);
    }

    // Check account is not locked
    if account.is_locked() {
        return Err(DomainError::AccountLocked);
    }

    // Add to available with overflow check
    let new_available = account
        .available()
        .checked_add(amount)
        .ok_or(DomainError::Overflow)?;

    account.set_available(new_available);
    Ok(())
}

/// Apply a withdrawal from an account
pub fn apply_withdrawal<A: AmountType>(
    account: &mut ClientAccount<A>,
    amount: A,
) -> Result<(), DomainError> {
    // Validate amount is positive
    if amount <= A::zero() {
        return Err(DomainError::InvalidAmount);
    }

    // Check account is not locked
    if account.is_locked() {
        return Err(DomainError::AccountLocked);
    }

    // Check sufficient funds
    if account.available() < amount {
        return Err(DomainError::InsufficientFunds);
    }

    // Subtract from available with underflow check
    let new_available = account
        .available()
        .checked_sub(amount)
        .ok_or(DomainError::Overflow)?;

    account.set_available(new_available);
    Ok(())
}

/// Apply a dispute to an account (move funds from available to held)
pub fn apply_dispute<A: AmountType>(
    account: &mut ClientAccount<A>,
    tx_id: u32,
    amount: A,
) -> Result<(), DomainError> {
    // Check account is not locked
    if account.is_locked() {
        return Err(DomainError::AccountLocked);
    }

    // Check not already disputed
    if account.is_disputed(tx_id) {
        return Err(DomainError::AlreadyDisputed);
    }

    // Check sufficient available funds
    if account.available() < amount {
        return Err(DomainError::InsufficientFunds);
    }

    // Move from available to held
    let new_available = account
        .available()
        .checked_sub(amount)
        .ok_or(DomainError::Overflow)?;

    let new_held = account
        .held()
        .checked_add(amount)
        .ok_or(DomainError::Overflow)?;

    account.set_available(new_available);
    account.set_held(new_held);
    account.add_disputed(tx_id);

    Ok(())
}

/// Apply a resolve to an account (move funds from held back to available)
pub fn apply_resolve<A: AmountType>(
    account: &mut ClientAccount<A>,
    tx_id: u32,
    amount: A,
) -> Result<(), DomainError> {
    // Check account is not locked
    if account.is_locked() {
        return Err(DomainError::AccountLocked);
    }

    // Check transaction is disputed
    if !account.is_disputed(tx_id) {
        return Err(DomainError::NotDisputed);
    }

    // Check sufficient held funds
    if account.held() < amount {
        return Err(DomainError::InsufficientFunds);
    }

    // Move from held to available
    let new_held = account
        .held()
        .checked_sub(amount)
        .ok_or(DomainError::Overflow)?;

    let new_available = account
        .available()
        .checked_add(amount)
        .ok_or(DomainError::Overflow)?;

    account.set_held(new_held);
    account.set_available(new_available);
    account.remove_disputed(tx_id);

    Ok(())
}

/// Apply a chargeback to an account (remove held funds and lock account)
pub fn apply_chargeback<A: AmountType>(
    account: &mut ClientAccount<A>,
    tx_id: u32,
    amount: A,
) -> Result<(), DomainError> {
    // Check transaction is disputed
    if !account.is_disputed(tx_id) {
        return Err(DomainError::NotDisputed);
    }

    // Check sufficient held funds
    if account.held() < amount {
        return Err(DomainError::InsufficientFunds);
    }

    // Remove from held
    let new_held = account
        .held()
        .checked_sub(amount)
        .ok_or(DomainError::Overflow)?;

    account.set_held(new_held);
    account.lock();
    account.remove_disputed(tx_id);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::amount::FixedPoint;

    #[test]
    fn deposit_increases_available_and_total() {
        let mut account = ClientAccount::new(1);
        let amount = FixedPoint::from_raw(10_000);

        apply_deposit(&mut account, amount).unwrap();

        assert_eq!(account.available(), FixedPoint::from_raw(10_000));
        assert_eq!(account.total(), FixedPoint::from_raw(10_000));
    }

    #[test]
    fn deposit_zero_fails() {
        let mut account = ClientAccount::new(1);

        let result = apply_deposit(&mut account, FixedPoint::zero());
        assert_eq!(result, Err(DomainError::InvalidAmount));
    }

    #[test]
    fn deposit_negative_fails() {
        let mut account = ClientAccount::new(1);

        let result = apply_deposit(&mut account, FixedPoint::from_raw(-100));
        assert_eq!(result, Err(DomainError::InvalidAmount));
    }

    #[test]
    fn deposit_on_locked_account_fails() {
        let mut account = ClientAccount::new(1);
        account.lock();

        let result = apply_deposit(&mut account, FixedPoint::from_raw(1000));
        assert_eq!(result, Err(DomainError::AccountLocked));

        // Account unchanged
        assert_eq!(account.available(), FixedPoint::zero());
    }

    #[test]
    fn withdrawal_decreases_available_and_total() {
        let mut account = ClientAccount::new(1);
        account.set_available(FixedPoint::from_raw(10_000));

        apply_withdrawal(&mut account, FixedPoint::from_raw(3_000)).unwrap();

        assert_eq!(account.available(), FixedPoint::from_raw(7_000));
        assert_eq!(account.total(), FixedPoint::from_raw(7_000));
    }

    #[test]
    fn withdrawal_insufficient_funds_fails() {
        let mut account = ClientAccount::new(1);
        account.set_available(FixedPoint::from_raw(1_000));

        let result = apply_withdrawal(&mut account, FixedPoint::from_raw(2_000));
        assert_eq!(result, Err(DomainError::InsufficientFunds));

        // Account unchanged
        assert_eq!(account.available(), FixedPoint::from_raw(1_000));
    }

    #[test]
    fn withdrawal_zero_fails() {
        let mut account = ClientAccount::new(1);
        account.set_available(FixedPoint::from_raw(10_000));

        let result = apply_withdrawal(&mut account, FixedPoint::zero());
        assert_eq!(result, Err(DomainError::InvalidAmount));
    }

    #[test]
    fn withdrawal_on_locked_account_fails() {
        let mut account = ClientAccount::new(1);
        account.set_available(FixedPoint::from_raw(10_000));
        account.lock();

        let result = apply_withdrawal(&mut account, FixedPoint::from_raw(1_000));
        assert_eq!(result, Err(DomainError::AccountLocked));
    }

    #[test]
    fn dispute_moves_funds_to_held() {
        let mut account = ClientAccount::new(1);
        account.set_available(FixedPoint::from_raw(10_000));

        apply_dispute(&mut account, 1, FixedPoint::from_raw(3_000)).unwrap();

        assert_eq!(account.available(), FixedPoint::from_raw(7_000));
        assert_eq!(account.held(), FixedPoint::from_raw(3_000));
        assert_eq!(account.total(), FixedPoint::from_raw(10_000)); // Total unchanged
        assert!(account.is_disputed(1));
    }

    #[test]
    fn dispute_insufficient_available_fails() {
        let mut account = ClientAccount::new(1);
        account.set_available(FixedPoint::from_raw(1_000));

        let result = apply_dispute(&mut account, 1, FixedPoint::from_raw(2_000));
        assert_eq!(result, Err(DomainError::InsufficientFunds));

        // Account unchanged
        assert_eq!(account.available(), FixedPoint::from_raw(1_000));
        assert_eq!(account.held(), FixedPoint::zero());
        assert!(!account.is_disputed(1));
    }

    #[test]
    fn dispute_on_locked_account_fails() {
        let mut account = ClientAccount::new(1);
        account.set_available(FixedPoint::from_raw(10_000));
        account.lock();

        let result = apply_dispute(&mut account, 1, FixedPoint::from_raw(3_000));
        assert_eq!(result, Err(DomainError::AccountLocked));
    }

    #[test]
    fn dispute_same_transaction_twice_fails() {
        let mut account = ClientAccount::new(1);
        account.set_available(FixedPoint::from_raw(10_000));

        apply_dispute(&mut account, 1, FixedPoint::from_raw(1_000)).unwrap();

        let result = apply_dispute(&mut account, 1, FixedPoint::from_raw(1_000));
        assert_eq!(result, Err(DomainError::AlreadyDisputed));

        // Account state from first dispute unchanged
        assert_eq!(account.available(), FixedPoint::from_raw(9_000));
        assert_eq!(account.held(), FixedPoint::from_raw(1_000));
        assert_eq!(account.disputed_count(), 1);
    }

    #[test]
    fn resolve_releases_held_funds() {
        let mut account = ClientAccount::new(1);
        account.set_available(FixedPoint::from_raw(7_000));
        account.set_held(FixedPoint::from_raw(3_000));
        account.add_disputed(1); // Mark as disputed first

        apply_resolve(&mut account, 1, FixedPoint::from_raw(3_000)).unwrap();

        assert_eq!(account.available(), FixedPoint::from_raw(10_000));
        assert_eq!(account.held(), FixedPoint::zero());
        assert_eq!(account.total(), FixedPoint::from_raw(10_000)); // Total unchanged
        assert!(!account.is_disputed(1)); // No longer disputed
    }

    #[test]
    fn resolve_insufficient_held_fails() {
        let mut account = ClientAccount::new(1);
        account.set_available(FixedPoint::from_raw(10_000));
        account.set_held(FixedPoint::from_raw(1_000));
        account.add_disputed(1);

        let result = apply_resolve(&mut account, 1, FixedPoint::from_raw(2_000));
        assert_eq!(result, Err(DomainError::InsufficientFunds));
    }

    #[test]
    fn resolve_on_locked_account_fails() {
        let mut account = ClientAccount::new(1);
        account.set_held(FixedPoint::from_raw(3_000));
        account.add_disputed(1);
        account.lock();

        let result = apply_resolve(&mut account, 1, FixedPoint::from_raw(3_000));
        assert_eq!(result, Err(DomainError::AccountLocked));
    }

    #[test]
    fn resolve_non_disputed_transaction_fails() {
        let mut account = ClientAccount::new(1);
        account.set_held(FixedPoint::from_raw(1_000));

        let result = apply_resolve(&mut account, 99, FixedPoint::from_raw(1_000));
        assert_eq!(result, Err(DomainError::NotDisputed));
    }

    #[test]
    fn chargeback_removes_held_and_locks() {
        let mut account = ClientAccount::new(1);
        account.set_available(FixedPoint::from_raw(7_000));
        account.set_held(FixedPoint::from_raw(3_000));
        account.add_disputed(1); // Mark as disputed first

        apply_chargeback(&mut account, 1, FixedPoint::from_raw(3_000)).unwrap();

        assert_eq!(account.available(), FixedPoint::from_raw(7_000)); // Unchanged
        assert_eq!(account.held(), FixedPoint::zero());
        assert_eq!(account.total(), FixedPoint::from_raw(7_000)); // Reduced
        assert!(account.is_locked());
        assert!(!account.is_disputed(1)); // Dispute resolved by chargeback
    }

    #[test]
    fn chargeback_insufficient_held_fails() {
        let mut account = ClientAccount::new(1);
        account.set_held(FixedPoint::from_raw(1_000));
        account.add_disputed(1);

        let result = apply_chargeback(&mut account, 1, FixedPoint::from_raw(2_000));
        assert_eq!(result, Err(DomainError::InsufficientFunds));

        // Account unchanged
        assert!(!account.is_locked());
    }

    #[test]
    fn chargeback_non_disputed_transaction_fails() {
        let mut account = ClientAccount::new(1);
        account.set_held(FixedPoint::from_raw(1_000));

        let result = apply_chargeback(&mut account, 99, FixedPoint::from_raw(1_000));
        assert_eq!(result, Err(DomainError::NotDisputed));

        assert!(!account.is_locked());
    }

    #[test]
    fn locked_account_rejects_all_mutations() {
        let mut account = ClientAccount::new(1);
        account.set_available(FixedPoint::from_raw(10_000));
        account.lock();

        assert_eq!(
            apply_deposit(&mut account, FixedPoint::from_raw(1_000)),
            Err(DomainError::AccountLocked)
        );

        assert_eq!(
            apply_withdrawal(&mut account, FixedPoint::from_raw(1_000)),
            Err(DomainError::AccountLocked)
        );

        assert_eq!(
            apply_dispute(&mut account, 1, FixedPoint::from_raw(1_000)),
            Err(DomainError::AccountLocked)
        );

        account.set_held(FixedPoint::from_raw(1_000));
        account.add_disputed(1);

        assert_eq!(
            apply_resolve(&mut account, 1, FixedPoint::from_raw(1_000)),
            Err(DomainError::AccountLocked)
        );

        // Note: chargeback doesn't check locked status (can chargeback even if locked)
    }

    #[test]
    fn multiple_deposits_accumulate() {
        let mut account = ClientAccount::new(1);

        apply_deposit(&mut account, FixedPoint::from_raw(1_000)).unwrap();
        apply_deposit(&mut account, FixedPoint::from_raw(2_000)).unwrap();
        apply_deposit(&mut account, FixedPoint::from_raw(3_000)).unwrap();

        assert_eq!(account.total(), FixedPoint::from_raw(6_000));
    }

    #[test]
    fn full_dispute_resolve_cycle() {
        let mut account = ClientAccount::new(1);
        account.set_available(FixedPoint::from_raw(10_000));

        let initial_total = account.total();

        // Dispute
        apply_dispute(&mut account, 1, FixedPoint::from_raw(3_000)).unwrap();
        assert_eq!(account.available(), FixedPoint::from_raw(7_000));
        assert_eq!(account.held(), FixedPoint::from_raw(3_000));
        assert_eq!(account.total(), initial_total);
        assert!(account.is_disputed(1));

        // Resolve
        apply_resolve(&mut account, 1, FixedPoint::from_raw(3_000)).unwrap();
        assert_eq!(account.available(), FixedPoint::from_raw(10_000));
        assert_eq!(account.held(), FixedPoint::zero());
        assert_eq!(account.total(), initial_total);
        assert!(!account.is_disputed(1));
    }

    #[test]
    fn full_dispute_chargeback_cycle() {
        let mut account = ClientAccount::new(1);
        account.set_available(FixedPoint::from_raw(10_000));

        // Dispute
        apply_dispute(&mut account, 1, FixedPoint::from_raw(3_000)).unwrap();
        assert_eq!(account.total(), FixedPoint::from_raw(10_000));
        assert!(account.is_disputed(1));

        // Chargeback
        apply_chargeback(&mut account, 1, FixedPoint::from_raw(3_000)).unwrap();
        assert_eq!(account.total(), FixedPoint::from_raw(7_000)); // Total reduced
        assert!(account.is_locked());
        assert!(!account.is_disputed(1)); // Dispute cleared by chargeback
    }

    #[test]
    fn account_can_have_multiple_disputes() {
        let mut account = ClientAccount::new(1);
        account.set_available(FixedPoint::from_raw(10_000));

        // Dispute three different transactions
        apply_dispute(&mut account, 1, FixedPoint::from_raw(1_000)).unwrap();
        apply_dispute(&mut account, 2, FixedPoint::from_raw(2_000)).unwrap();
        apply_dispute(&mut account, 3, FixedPoint::from_raw(3_000)).unwrap();

        assert_eq!(account.available(), FixedPoint::from_raw(4_000));
        assert_eq!(account.held(), FixedPoint::from_raw(6_000));
        assert_eq!(account.disputed_count(), 3);
        assert!(account.is_disputed(1));
        assert!(account.is_disputed(2));
        assert!(account.is_disputed(3));
    }
}
