//! Root-only pump-trader PDA: 0-byte, System Program owned.
//!
//! Created with system_instruction::create_account + invoke_signed trader
//! seeds. No #[account] data. SpendVault stays INTENTS-owned (73 bytes);
//! pump.fun buy_v2 system-transfers FROM user, which requires a
//! system-owned from. invoke_signed does not change owner.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    system_instruction, system_program,
};

use grok_chain_core::{GrokAccount, SEED_GROK_ACCOUNT};

use crate::constants::{
    PUMP_TRADER_SPACE, SEED_PUMP_TRADER, SEED_SPEND_VAULT, TOKEN_2022_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
};
use crate::errors::IntentsError;
use crate::events::{PumpTraderFunded, PumpTraderInitialized, PumpTraderWithdrawn};
use crate::instructions::common;
use crate::state::SpendVault;

/// SPL Token / Token-2022 account: mint(32) + owner(32) + amount(u64).
const TOKEN_ACCOUNT_MIN_LEN: usize = 72;
/// SPL Token instruction 3 = Transfer (legacy + Token-2022).
const TOKEN_TRANSFER_IX: u8 = 3;

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

/// Root-only. Not grant-gated. Agent cannot call.
///
/// `lamports == 0` is a SOL no-op (token-only sweep is ok).
/// SOL uses invoke_signed system transfer trader → root. Trader is
/// system-owned, so do NOT use try_debit_program_owned.
/// Leaves rent-exempt minimum for PUMP_TRADER_SPACE (0). Does not close.
/// Token sweep via remaining_accounts as even pairs [from_ata, to_ata, ...].
pub fn withdraw<'info>(ctx: Context<'_, '_, '_, 'info, WithdrawPumpTrader<'info>>, lamports: u64) -> Result<()> {
    // Clone remaining first so named AccountInfos can be taken independently.
    let remaining: Vec<AccountInfo> = ctx.remaining_accounts.to_vec();
    require!(
        remaining.len() % 2 == 0,
        IntentsError::WithdrawRemainingAccountsOdd
    );

    common::require_pump_trader_ready(&ctx.accounts.pump_trader.to_account_info())?;

    let grok = ctx.accounts.grok_account.key();
    let bump = [ctx.bumps.pump_trader];
    let trader_signer_seeds: &[&[u8]] = &[SEED_PUMP_TRADER, grok.as_ref(), bump.as_ref()];
    let trader_key = ctx.accounts.pump_trader.key();
    let root_key = ctx.accounts.root.key();

    if lamports > 0 {
        let min = Rent::get()?.minimum_balance(PUMP_TRADER_SPACE);
        let after = ctx
            .accounts
            .pump_trader
            .lamports()
            .checked_sub(lamports)
            .ok_or(error!(IntentsError::InsufficientPumpTrader))?;
        require!(after >= min, IntentsError::InsufficientPumpTrader);
        invoke_signed(
            &system_instruction::transfer(&trader_key, &root_key, lamports),
            &[
                ctx.accounts.pump_trader.to_account_info(),
                ctx.accounts.root.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[trader_signer_seeds],
        )?;
    }

    let trader = ctx.accounts.pump_trader.to_account_info();
    let token_program = ctx.accounts.token_program.to_account_info();
    let token_2022_program = ctx.accounts.token_2022_program.to_account_info();
    for pair in remaining.chunks_exact(2) {
        sweep_token_pair(
            &pair[0],
            &pair[1],
            &trader,
            &trader_key,
            &root_key,
            &token_program,
            &token_2022_program,
            trader_signer_seeds,
        )?;
    }

    emit!(PumpTraderWithdrawn {
        pump_trader: trader_key,
        grok_account: grok,
        root: root_key,
        lamports,
    });
    Ok(())
}

fn token_account_fields(info: &AccountInfo) -> Result<(Pubkey, Pubkey, u64)> {
    require!(
        *info.owner == TOKEN_PROGRAM_ID || *info.owner == TOKEN_2022_PROGRAM_ID,
        IntentsError::InvalidTokenProgram
    );
    let data = info.try_borrow_data()?;
    require!(
        data.len() >= TOKEN_ACCOUNT_MIN_LEN,
        IntentsError::WithdrawTokenAccountInvalid
    );
    let mint = Pubkey::new_from_array(data[0..32].try_into().unwrap());
    let owner = Pubkey::new_from_array(data[32..64].try_into().unwrap());
    let amount = u64::from_le_bytes(data[64..72].try_into().unwrap());
    Ok((mint, owner, amount))
}

fn sweep_token_pair<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    trader: &AccountInfo<'info>,
    trader_key: &Pubkey,
    root_key: &Pubkey,
    token_program: &AccountInfo<'info>,
    token_2022_program: &AccountInfo<'info>,
    trader_signer_seeds: &[&[u8]],
) -> Result<()> {
    require!(from.owner == to.owner, IntentsError::InvalidTokenProgram);
    let (from_mint, from_owner, amount) = token_account_fields(from)?;
    let (to_mint, to_owner, _to_amount) = token_account_fields(to)?;
    require!(from_owner == *trader_key, IntentsError::WithdrawTokenOwnerNotTrader);
    require!(to_owner == *root_key, IntentsError::WithdrawTokenDestOwnerNotRoot);
    require!(from_mint == to_mint, IntentsError::WithdrawTokenMintMismatch);
    if amount == 0 {
        return Ok(());
    }
    let token_program_ai = if *from.owner == TOKEN_PROGRAM_ID {
        token_program.clone()
    } else {
        token_2022_program.clone()
    };
    let mut data = Vec::with_capacity(9);
    data.push(TOKEN_TRANSFER_IX);
    data.extend_from_slice(&amount.to_le_bytes());
    let ix = Instruction {
        program_id: *from.owner,
        accounts: vec![
            AccountMeta::new(*from.key, false),
            AccountMeta::new(*to.key, false),
            AccountMeta::new_readonly(*trader.key, true),
        ],
        data,
    };
    invoke_signed(
        &ix,
        &[
            from.clone(),
            to.clone(),
            trader.clone(),
            token_program_ai,
        ],
        &[trader_signer_seeds],
    )?;
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

#[derive(Accounts)]
pub struct WithdrawPumpTrader<'info> {
    #[account(mut)]
    pub root: Signer<'info>,
    #[account(
        seeds = [SEED_GROK_ACCOUNT, grok_account.root.as_ref()],
        bump = grok_account.bump,
        seeds::program = grok_chain_core::ID,
        constraint = grok_account.root == root.key() @ IntentsError::UnauthorizedRoot,
    )]
    pub grok_account: Account<'info, GrokAccount>,
    /// CHECK: 0-byte system-owned trader PDA. Must already exist. Not closed.
    #[account(
        mut,
        seeds = [SEED_PUMP_TRADER, grok_account.key().as_ref()],
        bump,
    )]
    pub pump_trader: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: official SPL Token. Used when remaining from_ata.owner matches.
    #[account(address = TOKEN_PROGRAM_ID)]
    pub token_program: UncheckedAccount<'info>,
    /// CHECK: official Token-2022. Used when remaining from_ata.owner matches.
    #[account(address = TOKEN_2022_PROGRAM_ID)]
    pub token_2022_program: UncheckedAccount<'info>,
}
