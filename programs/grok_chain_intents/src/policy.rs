//! Pure policy checks shared by pay / swap / deploy / call.
//! Unit-testable without a validator. Handlers still run these before CPI.

use anchor_lang::prelude::*;

use crate::constants::MAX_SPONSOR_LAMPORTS;
use crate::errors::IntentsError;

/// `check_grant` amount for deploy (and the call/deploy path). Always 0.
pub const DEPLOY_CHECK_GRANT_AMOUNT: u64 = 0;

pub fn require_pay_amount(amount_lamports: u64) -> Result<()> {
    require!(amount_lamports > 0, IntentsError::ZeroPayAmount);
    Ok(())
}

pub fn require_swap_amounts(amount_in_lamports: u64, min_out_lamports: u64) -> Result<()> {
    require!(amount_in_lamports > 0, IntentsError::ZeroAmount);
    require!(
        amount_in_lamports >= min_out_lamports,
        IntentsError::MinOutNotMet
    );
    Ok(())
}

/// call/deploy: amount 0 is valid. No extra check on amount.
pub fn require_sponsor_cap(sponsor_lamports: u64) -> Result<()> {
    if sponsor_lamports > 0 {
        require!(
            sponsor_lamports <= MAX_SPONSOR_LAMPORTS,
            IntentsError::SponsorCapExceeded
        );
    }
    Ok(())
}

pub fn require_sponsor_accounts(sponsor_lamports: u64, present: bool) -> Result<()> {
    require_sponsor_cap(sponsor_lamports)?;
    if sponsor_lamports > 0 {
        require!(present, IntentsError::SponsorAccountsRequired);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pay_zero_fails() {
        assert!(require_pay_amount(0).is_err());
        assert!(require_pay_amount(1).is_ok());
    }

    #[test]
    fn swap_zero_fails_and_min_out_is_enforced() {
        assert!(require_swap_amounts(0, 0).is_err());
        assert!(require_swap_amounts(0, 1).is_err());
        assert!(require_swap_amounts(5, 6).is_err());
        assert!(require_swap_amounts(5, 5).is_ok());
        assert!(require_swap_amounts(6, 5).is_ok());
    }

    #[test]
    fn sponsor_cap_and_accounts() {
        assert!(require_sponsor_cap(0).is_ok());
        assert!(require_sponsor_cap(MAX_SPONSOR_LAMPORTS).is_ok());
        assert!(require_sponsor_cap(MAX_SPONSOR_LAMPORTS + 1).is_err());
        assert!(require_sponsor_accounts(0, false).is_ok());
        assert!(require_sponsor_accounts(1, false).is_err());
        assert!(require_sponsor_accounts(1, true).is_ok());
    }

    #[test]
    fn deploy_check_grant_amount_is_zero() {
        assert_eq!(DEPLOY_CHECK_GRANT_AMOUNT, 0);
    }
}
