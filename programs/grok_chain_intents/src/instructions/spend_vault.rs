use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke, system_instruction};

use grok_chain_core::{GrokAccount, SEED_GROK_ACCOUNT};

use crate::constants::SEED_SPEND_VAULT;
use crate::errors::IntentsError;
use crate::events::{SpendVaultFunded, SpendVaultInitialized, SpendVaultWithdrawn};
use crate::instructions::try_debit_program_owned;
use crate::state::SpendVault;

pub fn init(ctx: Context<InitSpendVault>) -> Result<()> {
    let vault = &mut ctx.accounts.spend_vault;
    vault.grok_account = ctx.accounts.grok_account.key();
    vault.root = ctx.accounts.root.key();
    vault.bump = ctx.bumps.spend_vault;
    emit!(SpendVaultInitialized {
        spend_vault: vault.key(),
        grok_account: vault.grok_account,
        root: vault.root,
    });
    Ok(())
}

pub fn fund(ctx: Context<FundSpendVault>, lamports: u64) -> Result<()> {
    require!(lamports > 0, IntentsError::ZeroAmount);
    invoke(
        &system_instruction::transfer(
            &ctx.accounts.root.key(),
            &ctx.accounts.spend_vault.key(),
            lamports,
        ),
        &[
            ctx.accounts.root.to_account_info(),
            ctx.accounts.spend_vault.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;
    emit!(SpendVaultFunded {
        spend_vault: ctx.accounts.spend_vault.key(),
        grok_account: ctx.accounts.grok_account.key(),
        root: ctx.accounts.root.key(),
        lamports,
    });
    Ok(())
}

pub fn withdraw(ctx: Context<WithdrawSpendVault>, lamports: u64) -> Result<()> {
    require!(lamports > 0, IntentsError::ZeroAmount);
    let min = Rent::get()?.minimum_balance(SpendVault::SPACE);
    require!(
        try_debit_program_owned(
            &ctx.accounts.spend_vault.to_account_info(),
            &ctx.accounts.root.to_account_info(),
            lamports,
            min,
        )?,
        IntentsError::InsufficientSpendVault
    );
    emit!(SpendVaultWithdrawn {
        spend_vault: ctx.accounts.spend_vault.key(),
        grok_account: ctx.accounts.grok_account.key(),
        root: ctx.accounts.root.key(),
        lamports,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct InitSpendVault<'info> {
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
        space = SpendVault::SPACE,
        seeds = [SEED_SPEND_VAULT, grok_account.key().as_ref()],
        bump
    )]
    pub spend_vault: Account<'info, SpendVault>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FundSpendVault<'info> {
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
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawSpendVault<'info> {
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
}
