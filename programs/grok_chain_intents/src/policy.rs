//! Pure policy checks shared by pay / swap / deploy / call / pump_buy / pump_sell / pump_create.
//! Unit-testable without a validator. Handlers still run these before CPI.

use anchor_lang::prelude::*;

use crate::constants::{
    MAX_SPONSOR_LAMPORTS, PUMP_AMM_BREAKING_FEE_RECIPIENTS, PUMP_AMM_BUY_ACCOUNT_COUNT,
    PUMP_AMM_BUY_ACCOUNT_COUNT_CASHBACK, PUMP_AMM_BUY_EXACT_QUOTE_IN_DATA_LEN,
    PUMP_AMM_BUY_EXACT_QUOTE_IN_DISC, PUMP_AMM_PROGRAM_ID, PUMP_AMM_PROTOCOL_FEE_RECIPIENTS,
    PUMP_AMM_RESERVED_FEE_RECIPIENTS, PUMP_AMM_SELL_ACCOUNT_COUNT,
    PUMP_AMM_SELL_DATA_LEN, PUMP_AMM_SELL_DISC,
    PUMP_CREATE_NAME_MAX, PUMP_CREATE_SYMBOL_MAX, PUMP_CREATE_URI_MAX,
    PUMP_CREATE_V2_ACCOUNT_COUNT, PUMP_CREATE_V2_ACCOUNT_COUNT_WITH_QUOTE, PUMP_CREATE_V2_DISC,
    PUMP_CREATE_V2_IX_DATA_MIN, PUMP_PROGRAM_ID, PUMP_TRADE_IX_DATA_LEN,
};
use crate::errors::IntentsError;

use anchor_lang::solana_program::system_program;

/// `check_grant` amount for deploy (and the call/deploy path). Always 0.
pub const DEPLOY_CHECK_GRANT_AMOUNT: u64 = 0;

/// `check_grant` amount for `pump_sell`. Always 0.
///
/// The grant cap is SOL *spent*. A sell spends base tokens and *receives* quote
/// (native SOL on SOL-paired coins). There is no SOL budget to consume.
/// `check_grant(0)` still enforces allowlist / expiry / revocation.
/// Residual: pump may debit ~0.0018 SOL from the trader to init
/// `user_volume_accumulator` on first trade; that is not counted here.
pub const PUMP_SELL_CHECK_GRANT_AMOUNT: u64 = 0;

/// `check_grant` amount for `pump_amm_sell`. Always 0.
///
/// Same honesty as bonding-curve `pump_sell`. Sell spends base tokens and
/// RECEIVES quote. Grant is a SOL-spent cap. Do not check_grant(min_quote)
/// or check_grant(base_amount_in) — tokens != SOL spent; min_quote is SOL received.
pub const PUMP_AMM_SELL_CHECK_GRANT_AMOUNT: u64 = 0;

/// Errors unless `amount == 0`. Guards against a dishonest non-zero grant.
pub fn require_pump_amm_sell_check_grant_amount(amount: u64) -> Result<()> {
    require!(
        amount == 0,
        IntentsError::PumpAmmSellCheckGrantAmountMustBeZero
    );
    Ok(())
}

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

/// Inner program must be the official pump.fun id. Anything else fails.
pub fn require_pump_program(id: &Pubkey) -> Result<()> {
    require_keys_eq!(*id, PUMP_PROGRAM_ID, IntentsError::PumpProgramMismatch);
    Ok(())
}

/// Agent signs INTENTS. Relayer is the outer fee payer. Agent is never either.
pub fn require_agent_not_fee_payer(agent: &Pubkey, fee_payer: Option<&Pubkey>) -> Result<()> {
    if let Some(fp) = fee_payer {
        require!(agent != fp, IntentsError::AgentCannotFeePay);
    }
    Ok(())
}

/// Unused by handlers. Error 21 is reserved. Vault is never pump user.
pub fn require_pump_user_is_vault(user: &Pubkey, vault: &Pubkey) -> Result<()> {
    require_keys_eq!(*user, *vault, IntentsError::PumpUserMustBeVault);
    Ok(())
}

/// remaining[13] must be the system-owned pump-trader PDA.
pub fn require_pump_user_is_trader(user: &Pubkey, trader: &Pubkey) -> Result<()> {
    require_keys_eq!(*user, *trader, IntentsError::PumpUserMustBeTrader);
    Ok(())
}

/// Vault is never pump user (system transfer needs a system-owned from).
pub fn require_pump_user_not_vault(user: &Pubkey, vault: &Pubkey) -> Result<()> {
    require!(*user != *vault, IntentsError::PumpUserMustBeTrader);
    Ok(())
}

pub fn require_pump_trader_system_owned(owner: &Pubkey) -> Result<()> {
    require_keys_eq!(
        *owner,
        system_program::ID,
        IntentsError::PumpTraderNotSystemOwned
    );
    Ok(())
}

pub fn require_pump_account_count(got: usize, expected: usize) -> Result<()> {
    require!(got == expected, IntentsError::PumpAccountCountMismatch);
    Ok(())
}

/// Official buy_v2 / sell_v2 payload is 24 bytes. Empty data is forbidden.
pub fn require_nonempty_pump_data(data: &[u8]) -> Result<()> {
    require!(!data.is_empty(), IntentsError::PumpEmptyDataForbidden);
    require!(
        data.len() == PUMP_TRADE_IX_DATA_LEN,
        IntentsError::PumpEmptyDataForbidden
    );
    Ok(())
}

pub fn require_pump_buy_amounts(amount: u64, max_sol_cost: u64) -> Result<()> {
    require!(amount > 0, IntentsError::ZeroAmount);
    require!(max_sol_cost > 0, IntentsError::ZeroAmount);
    Ok(())
}

/// `min_sol_output` may be 0 (accept any quote). `amount` must be > 0.
pub fn require_pump_sell_amounts(amount: u64, _min_sol_output: u64) -> Result<()> {
    require!(amount > 0, IntentsError::ZeroAmount);
    Ok(())
}


pub fn require_pump_create_strings(name: &str, symbol: &str, uri: &str) -> Result<()> {
    require!(
        name.chars().count() <= PUMP_CREATE_NAME_MAX,
        IntentsError::PumpCreateNameTooLong
    );
    require!(
        symbol.chars().count() <= PUMP_CREATE_SYMBOL_MAX,
        IntentsError::PumpCreateSymbolTooLong
    );
    require!(
        uri.chars().count() <= PUMP_CREATE_URI_MAX,
        IntentsError::PumpCreateUriTooLong
    );
    Ok(())
}

pub fn require_pump_create_amounts(max_sol_cost: u64) -> Result<()> {
    require!(max_sol_cost > 0, IntentsError::ZeroAmount);
    Ok(())
}

/// remaining[0] mint must be a signer (client Token-2022 keypair).
pub fn require_pump_mint_is_signer(is_signer: bool) -> Result<()> {
    require!(is_signer, IntentsError::PumpMintMustBeSigner);
    Ok(())
}

pub fn require_pump_create_account_count(got: usize) -> Result<()> {
    require!(
        got == PUMP_CREATE_V2_ACCOUNT_COUNT || got == PUMP_CREATE_V2_ACCOUNT_COUNT_WITH_QUOTE,
        IntentsError::PumpAccountCountMismatch
    );
    Ok(())
}

/// Official create_v2 payload starts with create_v2 disc. Empty data is forbidden.
pub fn require_nonempty_pump_create_data(data: &[u8]) -> Result<()> {
    require!(!data.is_empty(), IntentsError::PumpEmptyDataForbidden);
    require!(
        data.len() >= PUMP_CREATE_V2_IX_DATA_MIN,
        IntentsError::PumpEmptyDataForbidden
    );
    require!(
        data.len() >= 8 && data[..8] == PUMP_CREATE_V2_DISC,
        IntentsError::PumpEmptyDataForbidden
    );
    Ok(())
}


/// Inner program must be the official PumpSwap AMM id.
pub fn require_pump_amm_program(id: &Pubkey) -> Result<()> {
    require_keys_eq!(*id, PUMP_AMM_PROGRAM_ID, IntentsError::PumpAmmProgramMismatch);
    Ok(())
}

pub fn require_pump_amm_account_count(got: usize) -> Result<()> {
    require!(
        got == PUMP_AMM_BUY_ACCOUNT_COUNT || got == PUMP_AMM_BUY_ACCOUNT_COUNT_CASHBACK,
        IntentsError::PumpAmmAccountCountMismatch
    );
    Ok(())
}

pub fn require_nonempty_pump_amm_data(data: &[u8]) -> Result<()> {
    require!(!data.is_empty(), IntentsError::PumpEmptyDataForbidden);
    require!(
        data.len() == PUMP_AMM_BUY_EXACT_QUOTE_IN_DATA_LEN,
        IntentsError::PumpEmptyDataForbidden
    );
    require!(
        data[..8] == PUMP_AMM_BUY_EXACT_QUOTE_IN_DISC,
        IntentsError::PumpEmptyDataForbidden
    );
    Ok(())
}

pub fn require_pump_amm_buy_amounts(
    spendable_quote_in: u64,
    min_base_amount_out: u64,
    max_sol_cost: u64,
) -> Result<()> {
    require!(spendable_quote_in > 0, IntentsError::ZeroAmount);
    require!(min_base_amount_out > 0, IntentsError::ZeroAmount);
    require!(max_sol_cost > 0, IntentsError::ZeroAmount);
    require!(
        max_sol_cost >= spendable_quote_in,
        IntentsError::MinOutNotMet
    );
    Ok(())
}

pub fn require_pump_amm_sell_account_count(got: usize) -> Result<()> {
    require!(
        got == PUMP_AMM_SELL_ACCOUNT_COUNT,
        IntentsError::PumpAmmSellAccountCountMismatch
    );
    Ok(())
}

pub fn require_nonempty_pump_amm_sell_data(data: &[u8]) -> Result<()> {
    require!(!data.is_empty(), IntentsError::PumpEmptyDataForbidden);
    require!(
        data.len() == PUMP_AMM_SELL_DATA_LEN,
        IntentsError::PumpEmptyDataForbidden
    );
    require!(
        data[..8] == PUMP_AMM_SELL_DISC,
        IntentsError::PumpEmptyDataForbidden
    );
    Ok(())
}

/// `min_quote_amount_out` may be 0 (accept any quote). Same as bonding-curve
/// `require_pump_sell_amounts`. `base_amount_in` must be > 0.
pub fn require_pump_amm_sell_amounts(
    base_amount_in: u64,
    _min_quote_amount_out: u64,
) -> Result<()> {
    require!(base_amount_in > 0, IntentsError::ZeroAmount);
    Ok(())
}

pub fn require_pump_amm_protocol_fee_recipient(id: &Pubkey) -> Result<()> {
    let ok = PUMP_AMM_PROTOCOL_FEE_RECIPIENTS.iter().any(|p| p == id)
        || PUMP_AMM_RESERVED_FEE_RECIPIENTS.iter().any(|p| p == id);
    require!(ok, IntentsError::PumpAmmFeeRecipientInvalid);
    Ok(())
}

pub fn require_pump_amm_breaking_fee_recipient(id: &Pubkey) -> Result<()> {
    require!(
        PUMP_AMM_BREAKING_FEE_RECIPIENTS.iter().any(|p| p == id),
        IntentsError::PumpAmmFeeRecipientInvalid
    );
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

    #[test]
    fn target_not_pump_id_fails() {
        assert!(require_pump_program(&PUMP_PROGRAM_ID).is_ok());
        assert!(require_pump_program(&crate::ID).is_err());
        assert!(require_pump_program(&Pubkey::default()).is_err());
    }

    #[test]
    fn agent_cannot_be_fee_payer() {
        let agent = Pubkey::new_unique();
        let relayer = Pubkey::new_unique();
        assert!(require_agent_not_fee_payer(&agent, None).is_ok());
        assert!(require_agent_not_fee_payer(&agent, Some(&relayer)).is_ok());
        assert!(require_agent_not_fee_payer(&agent, Some(&agent)).is_err());
    }

    #[test]
    fn pump_buy_empty_data_forbidden_and_amounts() {
        assert!(require_nonempty_pump_data(&[]).is_err());
        assert!(require_nonempty_pump_data(&[1, 2, 3]).is_err());
        let good = crate::constants::encode_pump_buy_v2(1, 2);
        assert_eq!(good.len(), 24);
        assert!(require_nonempty_pump_data(&good).is_ok());
        assert!(require_pump_buy_amounts(0, 1).is_err());
        assert!(require_pump_buy_amounts(1, 0).is_err());
        assert!(require_pump_buy_amounts(1, 1).is_ok());
        assert!(require_pump_sell_amounts(0, 0).is_err());
        assert!(require_pump_sell_amounts(1, 0).is_ok());
        assert_eq!(PUMP_SELL_CHECK_GRANT_AMOUNT, 0);
    }

    #[test]
    fn pump_user_must_be_vault_and_count() {
        // Error 21 stays in the enum. Handlers do not call this.
        let vault = Pubkey::new_unique();
        let other = Pubkey::new_unique();
        assert!(require_pump_user_is_vault(&vault, &vault).is_ok());
        assert!(require_pump_user_is_vault(&other, &vault).is_err());
        assert!(require_pump_account_count(27, 27).is_ok());
        assert!(require_pump_account_count(26, 27).is_err());
        assert!(require_pump_account_count(26, 26).is_ok());
    }

    #[test]
    fn pump_user_must_be_trader_vault_as_user_fails() {
        let vault = Pubkey::new_unique();
        let trader = Pubkey::new_unique();
        assert!(require_pump_user_is_trader(&trader, &trader).is_ok());
        assert!(require_pump_user_is_trader(&vault, &trader).is_err());
        assert!(require_pump_user_not_vault(&trader, &vault).is_ok());
        assert!(require_pump_user_not_vault(&vault, &vault).is_err());
        assert!(require_pump_trader_system_owned(&system_program::ID).is_ok());
        assert!(require_pump_trader_system_owned(&crate::ID).is_err());
        assert!(require_pump_trader_system_owned(&Pubkey::new_unique()).is_err());
    }

    #[test]
    fn pump_create_strings_mint_signer_and_data() {
        assert!(require_pump_create_strings("n", "s", "u").is_ok());
        assert!(require_pump_create_strings(&"n".repeat(32), &"s".repeat(13), &"u".repeat(200)).is_ok());
        assert!(require_pump_create_strings(&"n".repeat(33), "s", "u").is_err());
        assert!(require_pump_create_strings("n", &"s".repeat(14), "u").is_err());
        assert!(require_pump_create_strings("n", "s", &"u".repeat(201)).is_err());
        assert!(require_pump_create_amounts(0).is_err());
        assert!(require_pump_create_amounts(1).is_ok());
        assert!(require_pump_mint_is_signer(true).is_ok());
        assert!(require_pump_mint_is_signer(false).is_err());
        assert!(require_pump_create_account_count(16).is_ok());
        assert!(require_pump_create_account_count(19).is_ok());
        assert!(require_pump_create_account_count(17).is_err());
        assert!(require_pump_create_account_count(27).is_err());
        let creator = Pubkey::new_unique();
        let good = crate::constants::encode_pump_create_v2("N", "S", "uri", &creator, false, false);
        assert!(require_nonempty_pump_create_data(&good).is_ok());
        assert!(require_nonempty_pump_create_data(&[]).is_err());
        assert!(require_nonempty_pump_create_data(&[1, 2, 3]).is_err());
        assert_eq!(&good[..8], &PUMP_CREATE_V2_DISC);
    }

    #[test]
    fn pump_amm_program_count_amounts_and_fees() {
        assert!(require_pump_amm_program(&PUMP_AMM_PROGRAM_ID).is_ok());
        assert!(require_pump_amm_program(&PUMP_PROGRAM_ID).is_err());
        assert!(require_pump_amm_program(&crate::ID).is_err());
        assert!(require_pump_amm_account_count(26).is_ok());
        assert!(require_pump_amm_account_count(27).is_ok());
        assert!(require_pump_amm_account_count(23).is_err());
        assert!(require_pump_amm_account_count(25).is_err());
        assert!(require_pump_amm_buy_amounts(0, 1, 1).is_err());
        assert!(require_pump_amm_buy_amounts(1, 0, 1).is_err());
        assert!(require_pump_amm_buy_amounts(2, 1, 1).is_err());
        assert!(require_pump_amm_buy_amounts(1, 1, 1).is_ok());
        assert!(require_pump_amm_buy_amounts(1, 1, 2).is_ok());
        let good = crate::constants::encode_pump_amm_buy_exact_quote_in(100, 1, true);
        assert_eq!(good.len(), 25);
        assert!(require_nonempty_pump_amm_data(&good).is_ok());
        assert!(require_nonempty_pump_amm_data(&[]).is_err());
        assert!(require_pump_amm_protocol_fee_recipient(&PUMP_AMM_PROTOCOL_FEE_RECIPIENTS[5]).is_ok());
        assert!(require_pump_amm_protocol_fee_recipient(&PUMP_AMM_RESERVED_FEE_RECIPIENTS[0]).is_ok());
        assert!(require_pump_amm_protocol_fee_recipient(&Pubkey::new_unique()).is_err());
        assert!(require_pump_amm_breaking_fee_recipient(&PUMP_AMM_BREAKING_FEE_RECIPIENTS[0]).is_ok());
        assert!(require_pump_amm_breaking_fee_recipient(&Pubkey::new_unique()).is_err());
        // sell: official 24 only. Buy 26/27 and cashback-shaped 26 fail (volume accs would shift fee_config).
        assert!(require_pump_amm_sell_account_count(24).is_ok());
        assert!(require_pump_amm_sell_account_count(26).is_err());
        assert!(require_pump_amm_sell_account_count(23).is_err());
        assert!(require_pump_amm_sell_account_count(25).is_err());
        assert!(require_pump_amm_sell_account_count(27).is_err());
        assert!(require_pump_amm_sell_amounts(0, 1).is_err());
        assert!(require_pump_amm_sell_amounts(1, 0).is_ok());
        assert!(require_pump_amm_sell_amounts(1, 1).is_ok());
        let sell = crate::constants::encode_pump_amm_sell(100, 1);
        assert_eq!(sell.len(), 24);
        assert_eq!(&sell[..8], &PUMP_AMM_SELL_DISC);
        assert!(require_nonempty_pump_amm_sell_data(&sell).is_ok());
        assert!(require_nonempty_pump_amm_sell_data(&[]).is_err());
        assert!(require_nonempty_pump_amm_sell_data(&good).is_err()); // buy 25-byte payload
        assert_eq!(PUMP_SELL_CHECK_GRANT_AMOUNT, 0);
        assert_eq!(PUMP_AMM_SELL_CHECK_GRANT_AMOUNT, 0);
        assert!(require_pump_amm_sell_check_grant_amount(0).is_ok());
        assert!(require_pump_amm_sell_check_grant_amount(1).is_err());
        assert!(require_pump_amm_sell_check_grant_amount(PUMP_AMM_SELL_CHECK_GRANT_AMOUNT).is_ok());
    }
}

/// Trader must already hold spendable + 0-byte rent.
/// pump_buy / pump_create / pump_amm_buy must not raw-debit vault→trader
/// in the same ix (named vault + remaining user, or raw debit + later
/// system/pump CPI, is UnbalancedInstruction). Root fund_pump_trader first.
pub fn require_pump_trader_prefunded(
    trader_lamports: u64,
    spendable_quote_in: u64,
    rent0: u64,
) -> Result<()> {
    let need = spendable_quote_in
        .checked_add(rent0)
        .ok_or(error!(IntentsError::LamportOverflow))?;
    require!(
        trader_lamports >= need,
        IntentsError::PumpTraderUnderfunded
    );
    Ok(())
}
