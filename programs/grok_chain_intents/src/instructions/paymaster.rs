use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke, system_instruction};

use grok_chain_core::{GrokAccount, SEED_GROK_ACCOUNT};

use crate::constants::SEED_PAYMASTER;
use crate::errors::IntentsError;
use crate::events::{
    PaymasterFunded, PaymasterInitialized, PaymasterPaused, PaymasterUnpaused, PaymasterWithdrawn,
    RelayerSet,
};
use crate::instructions::try_debit_program_owned;
use crate::state::Paymaster;

pub fn init(ctx: Context<InitPaymaster>, relayer: Pubkey) -> Result<()> {
    let pm = &mut ctx.accounts.paymaster;
    pm.grok_account = ctx.accounts.grok_account.key();
    pm.root = ctx.accounts.root.key();
    pm.relayer = relayer;
    pm.bump = ctx.bumps.paymaster;
    pm.paused = false;
    emit!(PaymasterInitialized {
        paymaster: pm.key(),
        grok_account: pm.grok_account,
        root: pm.root,
        relayer,
    });
    Ok(())
}

pub fn fund(ctx: Context<FundPaymaster>, lamports: u64) -> Result<()> {
    require!(lamports > 0, IntentsError::ZeroAmount);
    invoke(
        &system_instruction::transfer(
            &ctx.accounts.root.key(),
            &ctx.accounts.paymaster.key(),
            lamports,
        ),
        &[
            ctx.accounts.root.to_account_info(),
            ctx.accounts.paymaster.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;
    emit!(PaymasterFunded {
        paymaster: ctx.accounts.paymaster.key(),
        grok_account: ctx.accounts.grok_account.key(),
        root: ctx.accounts.root.key(),
        lamports,
    });
    Ok(())
}

pub fn withdraw(ctx: Context<WithdrawPaymaster>, lamports: u64) -> Result<()> {
    require!(lamports > 0, IntentsError::ZeroAmount);
    let min = Rent::get()?.minimum_balance(Paymaster::SPACE);
    require!(
        try_debit_program_owned(
            &ctx.accounts.paymaster.to_account_info(),
            &ctx.accounts.root.to_account_info(),
            lamports,
            min,
        )?,
        IntentsError::InsufficientPaymaster
    );
    emit!(PaymasterWithdrawn {
        paymaster: ctx.accounts.paymaster.key(),
        grok_account: ctx.accounts.grok_account.key(),
        root: ctx.accounts.root.key(),
        lamports,
    });
    Ok(())
}

pub fn set(ctx: Context<SetRelayer>, relayer: Pubkey) -> Result<()> {
    ctx.accounts.paymaster.relayer = relayer;
    emit!(RelayerSet {
        paymaster: ctx.accounts.paymaster.key(),
        grok_account: ctx.accounts.grok_account.key(),
        root: ctx.accounts.root.key(),
        relayer,
    });
    Ok(())
}

pub fn pause(ctx: Context<PausePaymaster>) -> Result<()> {
    ctx.accounts.paymaster.paused = true;
    emit!(PaymasterPaused {
        paymaster: ctx.accounts.paymaster.key(),
        grok_account: ctx.accounts.grok_account.key(),
        root: ctx.accounts.root.key(),
    });
    Ok(())
}

pub fn unpause(ctx: Context<UnpausePaymaster>) -> Result<()> {
    ctx.accounts.paymaster.paused = false;
    emit!(PaymasterUnpaused {
        paymaster: ctx.accounts.paymaster.key(),
        grok_account: ctx.accounts.grok_account.key(),
        root: ctx.accounts.root.key(),
    });
    Ok(())
}

#[derive(Accounts)]
pub struct InitPaymaster<'info> {
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
        init,
        payer = root,
        space = Paymaster::SPACE,
        seeds = [SEED_PAYMASTER, grok_account.key().as_ref()],
        bump
    )]
    pub paymaster: Account<'info, Paymaster>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FundPaymaster<'info> {
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
        seeds = [SEED_PAYMASTER, grok_account.key().as_ref()],
        bump = paymaster.bump,
        constraint = paymaster.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = paymaster.root == root.key() @ IntentsError::UnauthorizedRoot,
    )]
    pub paymaster: Account<'info, Paymaster>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawPaymaster<'info> {
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
        seeds = [SEED_PAYMASTER, grok_account.key().as_ref()],
        bump = paymaster.bump,
        constraint = paymaster.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = paymaster.root == root.key() @ IntentsError::UnauthorizedRoot,
    )]
    pub paymaster: Account<'info, Paymaster>,
}

#[derive(Accounts)]
pub struct SetRelayer<'info> {
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
        seeds = [SEED_PAYMASTER, grok_account.key().as_ref()],
        bump = paymaster.bump,
        constraint = paymaster.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = paymaster.root == root.key() @ IntentsError::UnauthorizedRoot,
    )]
    pub paymaster: Account<'info, Paymaster>,
}

#[derive(Accounts)]
pub struct PausePaymaster<'info> {
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
        seeds = [SEED_PAYMASTER, grok_account.key().as_ref()],
        bump = paymaster.bump,
        constraint = paymaster.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = paymaster.root == root.key() @ IntentsError::UnauthorizedRoot,
    )]
    pub paymaster: Account<'info, Paymaster>,
}

#[derive(Accounts)]
pub struct UnpausePaymaster<'info> {
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
        seeds = [SEED_PAYMASTER, grok_account.key().as_ref()],
        bump = paymaster.bump,
        constraint = paymaster.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = paymaster.root == root.key() @ IntentsError::UnauthorizedRoot,
    )]
    pub paymaster: Account<'info, Paymaster>,
}
