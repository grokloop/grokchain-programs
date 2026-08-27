use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
};

use grok_chain_core::{Grant, GrokAccount, SEED_GRANT, SEED_GROK_ACCOUNT};

use crate::constants::{SEED_PAYMASTER, SEED_SPEND_VAULT};
use crate::core_cpi;
use crate::errors::IntentsError;
use crate::events::Called;
use crate::instructions::common;
use crate::state::{CallArgs, Paymaster, SpendVault};

/// Grant-gated router call. Not an on-chain compiler.
/// check_grant always uses this INTENTS router (router mode).
/// amount 0 = policy ping (no vault debit). amount > 0 = SpendVault → recipient.
/// remaining_accounts empty: succeed after check_grant.
/// remaining_accounts non-empty: invoke (NOT invoke_signed) into args.target_program
/// with empty ix data. Never signs as the vault PDA.
/// The target program AccountInfo must appear in remaining_accounts so invoke can
/// find it (client prepends it). It is filtered out of the inner metas.
pub fn handler(ctx: Context<Call>, args: CallArgs) -> Result<()> {
    require_keys_eq!(
        ctx.accounts.call_target.key(),
        args.target_program,
        IntentsError::CallTargetMismatch
    );
    require!(
        ctx.accounts.call_target.executable,
        IntentsError::TargetNotExecutable
    );

    common::precheck_sponsor(
        args.sponsor_lamports,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
    )?;

    let pm_key = ctx.accounts.paymaster.as_ref().map(|p| p.key());
    common::reject_protected_remaining(
        ctx.remaining_accounts,
        &ctx.accounts.spend_vault.key(),
        pm_key.as_ref(),
    )?;

    // 1. CORE check_grant — abort on error. One CPI. Never skip.
    // target_program on the CPI is this router, not the inner call target.
    core_cpi::check_grant(
        ctx.accounts.grok_chain_core_program.to_account_info(),
        ctx.accounts.grok_account.to_account_info(),
        ctx.accounts.grant.to_account_info(),
        ctx.accounts.agent.to_account_info(),
        ctx.accounts.intents_program.to_account_info(),
        args.amount_lamports,
    )?;

    // 2. Optional vault debit. amount 0 is a policy ping: no debit.
    if args.amount_lamports > 0 {
        common::debit_spend_vault(
            &ctx.accounts.spend_vault,
            &ctx.accounts.recipient.to_account_info(),
            args.amount_lamports,
        )?;
    }

    // 3. Optional inner invoke. Empty remaining_accounts = grant-checked only.
    // invoke, never invoke_signed: we must not sign as spend_vault / paymaster.
    let remaining_len = ctx.remaining_accounts.len() as u32;
    if !ctx.remaining_accounts.is_empty() {
        require!(
            ctx.remaining_accounts
                .iter()
                .any(|a| a.key == &args.target_program),
            IntentsError::CallTargetMismatch
        );
        let metas: Vec<AccountMeta> = ctx
            .remaining_accounts
            .iter()
            .filter(|a| a.key != &args.target_program)
            .map(|a| {
                if a.is_writable {
                    AccountMeta::new(*a.key, a.is_signer)
                } else {
                    AccountMeta::new_readonly(*a.key, a.is_signer)
                }
            })
            .collect();
        let ix = Instruction {
            program_id: args.target_program,
            accounts: metas,
            data: vec![],
        };
        invoke(&ix, ctx.remaining_accounts)?;
    }

    // 4. Optional sponsor reimbursement.
    common::reimburse_sponsor(
        &ctx.accounts.grant,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
        args.sponsor_lamports,
    )?;

    emit!(Called {
        target_program: args.target_program,
        recipient: ctx.accounts.recipient.key(),
        amount_lamports: args.amount_lamports,
        remaining_len,
        agent: ctx.accounts.agent.key(),
        grant: ctx.accounts.grant.key(),
        generation: ctx.accounts.grant.generation,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct Call<'info> {
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
    #[account(
        mut,
        seeds = [SEED_SPEND_VAULT, grok_account.key().as_ref()],
        bump = spend_vault.bump,
        constraint = spend_vault.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = spend_vault.root == grok_account.root @ IntentsError::UnauthorizedRoot,
    )]
    pub spend_vault: Account<'info, SpendVault>,
    /// CHECK: recipient of an optional SOL debit. Dummy ok when amount == 0.
    #[account(mut)]
    pub recipient: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: inner program remaining_accounts are invoked into. Must match args.
    pub call_target: UncheckedAccount<'info>,
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
