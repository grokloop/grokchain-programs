use anchor_lang::prelude::*;

use grok_chain_core::{Grant, GrokAccount, SEED_GRANT, SEED_GROK_ACCOUNT};

use crate::constants::{SEED_PAYMASTER, SEED_SPEND_VAULT};
use crate::core_cpi;
use crate::errors::IntentsError;
use crate::events::Swapped;
use crate::instructions::common;
use crate::policy;
use crate::state::{Paymaster, SpendVault, SwapArgs};

/// Grant-gated SOL send with a min_out check. Not Jupiter. Not an AMM.
pub fn handler(ctx: Context<Swap>, args: SwapArgs) -> Result<()> {
    policy::require_swap_amounts(args.amount_in_lamports, args.min_out_lamports)?;
    common::precheck_sponsor(
        args.sponsor_lamports,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
    )?;

    // 1. CORE check_grant — abort on error. One CPI per intent.
    // target_program = this router. amount = SOL this swap spends under the grant cap.
    core_cpi::check_grant(
        ctx.accounts.grok_chain_core_program.to_account_info(),
        ctx.accounts.grok_account.to_account_info(),
        ctx.accounts.grant.to_account_info(),
        ctx.accounts.agent.to_account_info(),
        ctx.accounts.intents_program.to_account_info(),
        args.amount_in_lamports,
    )?;

    // 2. SpendVault PDA → out_destination (SOL only; no SPL).
    common::debit_spend_vault(
        &ctx.accounts.spend_vault,
        &ctx.accounts.out_destination.to_account_info(),
        args.amount_in_lamports,
    )?;

    // 3. Optional sponsor reimbursement (paymaster → relayer).
    common::reimburse_sponsor(
        &ctx.accounts.grant,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
        args.sponsor_lamports,
    )?;

    emit!(Swapped {
        vault: ctx.accounts.spend_vault.key(),
        destination: ctx.accounts.out_destination.key(),
        amount_in_lamports: args.amount_in_lamports,
        min_out_lamports: args.min_out_lamports,
        agent: ctx.accounts.agent.key(),
        grant: ctx.accounts.grant.key(),
        generation: ctx.accounts.grant.generation,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct Swap<'info> {
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
    /// CHECK: this program (router). CORE allowlists this id. Must be executable (CORE checks).
    #[account(address = crate::ID)]
    pub intents_program: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [SEED_SPEND_VAULT, grok_account.key().as_ref()],
        bump = spend_vault.bump,
        constraint = spend_vault.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = spend_vault.root == grok_account.root @ IntentsError::UnauthorizedRoot,
    )]
    pub spend_vault: Account<'info, SpendVault>,
    /// CHECK: destination of the SOL send. Any account; we only credit lamports.
    #[account(mut)]
    pub out_destination: UncheckedAccount<'info>,
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
