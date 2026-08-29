//! Shared grant-gated intent mouth: one check_grant, optional sponsor, vault debit.
//! Agent never fee-pays and never is the SOL source.
//! Never invoke_signed with vault seeds. The pump adapter (`pump.rs`)
//! invoke_signeds only trader seeds for the hardcoded pump.fun program
//! (buy_v2 / sell_v2 / create_v2), never for an arbitrary program or
//! arbitrary remaining-account blob.

use anchor_lang::prelude::*;

use grok_chain_core::Grant;

use crate::errors::IntentsError;
use crate::events::Sponsored;
use crate::instructions::try_debit_program_owned;
use crate::policy;
use crate::state::{Paymaster, SpendVault};

pub fn precheck_sponsor<'info>(
    sponsor_lamports: u64,
    paymaster: &Option<Account<'info, Paymaster>>,
    fee_payer: &Option<Signer<'info>>,
) -> Result<()> {
    policy::require_sponsor_accounts(
        sponsor_lamports,
        paymaster.is_some() && fee_payer.is_some(),
    )
}

/// Optional paymaster → relayer reimbursement. Not a CORE vault debit.
/// Must run after a successful check_grant. Abort on any check.
pub fn reimburse_sponsor<'info>(
    grant: &Account<'info, Grant>,
    paymaster: &Option<Account<'info, Paymaster>>,
    fee_payer: &Option<Signer<'info>>,
    sponsor_lamports: u64,
) -> Result<()> {
    if sponsor_lamports == 0 {
        return Ok(());
    }
    let paymaster = paymaster
        .as_ref()
        .ok_or(error!(IntentsError::SponsorAccountsRequired))?;
    let fee_payer = fee_payer
        .as_ref()
        .ok_or(error!(IntentsError::SponsorAccountsRequired))?;

    require!(grant.sponsor_eligible, IntentsError::NotSponsorEligible);
    require!(!paymaster.paused, IntentsError::PaymasterPaused);
    require!(
        fee_payer.key() == paymaster.relayer,
        IntentsError::RelayerMismatch
    );

    let min_pm = Rent::get()?.minimum_balance(Paymaster::SPACE);
    require!(
        try_debit_program_owned(
            &paymaster.to_account_info(),
            &fee_payer.to_account_info(),
            sponsor_lamports,
            min_pm,
        )?,
        IntentsError::InsufficientPaymaster
    );

    emit!(Sponsored {
        paymaster: paymaster.key(),
        relayer: fee_payer.key(),
        sponsor_lamports,
        grant: grant.key(),
        generation: grant.generation,
    });
    Ok(())
}

/// Debit SpendVault → dest. Leaves rent-exempt minimum.
pub fn debit_spend_vault<'info>(
    spend_vault: &Account<'info, SpendVault>,
    dest: &AccountInfo<'info>,
    amount_lamports: u64,
) -> Result<()> {
    let min_vault = Rent::get()?.minimum_balance(SpendVault::SPACE);
    require!(
        try_debit_program_owned(
            &spend_vault.to_account_info(),
            dest,
            amount_lamports,
            min_vault,
        )?,
        IntentsError::InsufficientSpendVault
    );
    Ok(())
}

/// Trader is a 0-byte system-owned PDA. Must exist (rent-exempt) and stay system-owned.
pub fn require_pump_trader_ready(trader: &AccountInfo) -> Result<()> {
    require!(
        *trader.owner == anchor_lang::solana_program::system_program::ID,
        IntentsError::PumpTraderNotSystemOwned
    );
    require!(trader.data_is_empty(), IntentsError::PumpTraderNotSystemOwned);
    let min = Rent::get()?.minimum_balance(crate::PUMP_TRADER_SPACE);
    require!(
        trader.lamports() >= min,
        IntentsError::PumpTraderNotInitialized
    );
    Ok(())
}

/// remaining_accounts must not include our program-owned vaults.
pub fn reject_protected_remaining<'info>(
    remaining: &[AccountInfo<'info>],
    spend_vault: &Pubkey,
    paymaster: Option<&Pubkey>,
) -> Result<()> {
    for acc in remaining {
        require!(
            acc.key() != *spend_vault,
            IntentsError::ProtectedAccountInRemaining
        );
        if let Some(pm) = paymaster {
            require!(
                acc.key() != *pm,
                IntentsError::ProtectedAccountInRemaining
            );
        }
    }
    Ok(())
}

/// remaining_accounts must not include the paymaster. Pump `user` is the
/// system-owned pump-trader PDA (remaining[13]), not SpendVault.
pub fn reject_paymaster_in_remaining<'info>(
    remaining: &[AccountInfo<'info>],
    paymaster: Option<&Pubkey>,
) -> Result<()> {
    if let Some(pm) = paymaster {
        for acc in remaining {
            require!(acc.key() != *pm, IntentsError::ProtectedAccountInRemaining);
        }
    }
    Ok(())
}

