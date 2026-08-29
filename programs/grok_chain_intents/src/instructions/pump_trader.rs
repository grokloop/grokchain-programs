//! Root-only pump-trader PDA: 0-byte, System Program owned.
//!
//! Created with system_instruction::create_account + invoke_signed trader
//! seeds. No #[account] data. SpendVault stays INTENTS-owned (73 bytes);
//! pump.fun buy_v2 system-transfers FROM user, which requires a
//! system-owned from. invoke_signed does not change owner.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    program::invoke_signed, system_instruction, system_program,
};

use grok_chain_core::{GrokAccount, SEED_GROK_ACCOUNT};

use crate::constants::{PUMP_TRADER_SPACE, SEED_PUMP_TRADER, SEED_SPEND_VAULT};
use crate::errors::IntentsError;
use crate::events::{PumpTraderFunded, PumpTraderInitialized};
use crate::instructions::common;
use crate::state::SpendVault;

pub fn init(ctx: Context<InitPumpTrader>) -> Result<()> {
    let trader = &ctx.accounts.pump_trader;
    require!(
        trader.lamports() == 0 && trader.data_is_empty(),
        IntentsError::PumpTraderAlreadyExists
    );

    let grok = ctx.accounts.grok_account.key();
    let bump = [ctx.bumps.pump_trader];
    let trader_signer_seeds: &[&[u8]] = &[SEED_PUMP_TRADER, grok.as_ref(), bump.as_ref()];
    let rent = Rent::get()?.minimum_balance(PUMP_TRADER_SPACE);

    invoke_signed(
        &system_instruction::create_account(
            &ctx.accounts.root.key(),
            &trader.key(),
            rent,
            PUMP_TRADER_SPACE as u64,
            &system_program::ID,
        ),
        &[
            ctx.accounts.root.to_account_info(),
            trader.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        &[trader_signer_seeds],
    )?;

    emit!(PumpTraderInitialized {
        pump_trader: trader.key(),
        grok_account: grok,
        root: ctx.accounts.root.key(),
    });
    Ok(())
}

/// Root: try_debit SpendVault → trader. No grant. Root can also
/// system-transfer to the trader anytime (it is system-owned).
pub fn fund(ctx: Context<FundPumpTrader>, lamports: u64) -> Result<()> {
    require!(lamports > 0, IntentsError::ZeroAmount);
    common::require_pump_trader_ready(&ctx.accounts.pump_trader.to_account_info())?;
    common::debit_spend_vault(
        &ctx.accounts.spend_vault,
        &ctx.accounts.pump_trader.to_account_info(),
        lamports,
    )?;
    emit!(PumpTraderFunded {
        pump_trader: ctx.accounts.pump_trader.key(),
        spend_vault: ctx.accounts.spend_vault.key(),
        grok_account: ctx.accounts.grok_account.key(),
        root: ctx.accounts.root.key(),
        lamports,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct InitPumpTrader<'info> {
    #[account(mut)]
    pub root: Signer<'info>,
    #[account(
        seeds = [SEED_GROK_ACCOUNT, grok_account.root.as_ref()],
        bump = grok_account.bump,
        seeds::program = grok_chain_core::ID,
        constraint = grok_account.root == root.key() @ IntentsError::UnauthorizedRoot,
    )]
    pub grok_account: Account<'info, GrokAccount>,
    /// CHECK: 0-byte system-owned PDA. No #[account] data. Created here.
    #[account(
        mut,
        seeds = [SEED_PUMP_TRADER, grok_account.key().as_ref()],
        bump,
    )]
    pub pump_trader: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FundPumpTrader<'info> {
    #[account(mut)]
    pub root: Signer<'info>,
    #[account(
        seeds = [SEED_GROK_ACCOUNT, grok_account.root.as_ref()],
        bump = grok_account.bump,
        seeds::program = grok_chain_core::ID,
        constraint = grok_account.root == root.key() @ IntentsError::UnauthorizedRoot,
    )]
    pub grok_account: Account<'info, GrokAccount>,
    #[account(
        mut,
        seeds = [SEED_SPEND_VAULT, grok_account.key().as_ref()],
        bump = spend_vault.bump,
        constraint = spend_vault.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = spend_vault.root == root.key() @ IntentsError::UnauthorizedRoot,
    )]
    pub spend_vault: Account<'info, SpendVault>,
    /// CHECK: 0-byte system-owned trader PDA. Must already exist.
    #[account(
        mut,
        seeds = [SEED_PUMP_TRADER, grok_account.key().as_ref()],
        bump,
    )]
    pub pump_trader: UncheckedAccount<'info>,
}
