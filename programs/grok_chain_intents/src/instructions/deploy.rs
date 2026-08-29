use anchor_lang::prelude::*;

use grok_chain_core::{Grant, GrokAccount, SEED_GRANT, SEED_GROK_ACCOUNT};

use crate::constants::{SEED_PAYMASTER, SEED_SPEND_VAULT};
use crate::core_cpi;
use crate::errors::IntentsError;
use crate::events::DeployRequested;
use crate::instructions::common;
use crate::policy;
use crate::state::{DeployArgs, Paymaster, SpendVault};

/// Grant-gated deploy *request*. Does not invoke bpf_loader. Does not upload ELF.
/// Does not pretend a program was deployed. Default: check_grant(0) + event.
/// Not a pump.fun coin launch. Coin launch is `pump_create`. This ix stays
/// check_grant(0) + event only.
pub fn handler(ctx: Context<Deploy>, args: DeployArgs) -> Result<()> {
    common::precheck_sponsor(
        args.sponsor_lamports,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
    )?;

    // 1. CORE check_grant(0) — abort on error. One CPI. call/deploy path.
    core_cpi::check_grant(
        ctx.accounts.grok_chain_core_program.to_account_info(),
        ctx.accounts.grok_account.to_account_info(),
        ctx.accounts.grant.to_account_info(),
        ctx.accounts.agent.to_account_info(),
        ctx.accounts.intents_program.to_account_info(),
        policy::DEPLOY_CHECK_GRANT_AMOUNT,
    )?;

    // 2. No vault debit. No bpf_loader. No ELF.
    // remaining_accounts are ignored (default is event + grant check only).

    // 3. Optional sponsor reimbursement (paymaster → relayer).
    common::reimburse_sponsor(
        &ctx.accounts.grant,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
        args.sponsor_lamports,
    )?;

    emit!(DeployRequested {
        program_id: args.program_id,
        agent: ctx.accounts.agent.key(),
        grant: ctx.accounts.grant.key(),
        generation: ctx.accounts.grant.generation,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct Deploy<'info> {
    /// Grant agent. Must sign. Never the fee payer / SOL source.
    pub agent: Signer<'info>,
    #[account(
        seeds = [SEED_GROK_ACCOUNT, grok_account.root.as_ref()],
        bump = grok_account.bump,
        seeds::program = grok_chain_core::ID,
    )]
    pub grok_account: Account<'info, GrokAccount>,
    #[account(
        mut,
        seeds = [SEED_GRANT, grok_account.key().as_ref(), agent.key().as_ref()],
        bump = grant.bump,
        seeds::program = grok_chain_core::ID,
        constraint = grant.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = grant.agent == agent.key() @ IntentsError::AgentMismatch,
    )]
    pub grant: Account<'info, Grant>,
    pub grok_chain_core_program: Program<'info, grok_chain_core::program::GrokChainCore>,
    /// CHECK: this program (router). CORE allowlists this id.
    #[account(address = crate::ID)]
    pub intents_program: UncheckedAccount<'info>,
    /// Present so deploy uses the same mouth as pay. Never debited.
    #[account(
        seeds = [SEED_SPEND_VAULT, grok_account.key().as_ref()],
        bump = spend_vault.bump,
        constraint = spend_vault.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = spend_vault.root == grok_account.root @ IntentsError::UnauthorizedRoot,
    )]
    pub spend_vault: Account<'info, SpendVault>,
    pub system_program: Program<'info, System>,
    #[account(
        mut,
        seeds = [SEED_PAYMASTER, grok_account.key().as_ref()],
        bump = paymaster.bump,
        constraint = paymaster.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = paymaster.root == grok_account.root @ IntentsError::UnauthorizedRoot,
    )]
    pub paymaster: Option<Account<'info, Paymaster>>,
    #[account(mut)]
    pub fee_payer: Option<Signer<'info>>,
}
