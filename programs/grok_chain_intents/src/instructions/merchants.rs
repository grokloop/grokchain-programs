//! `MerchantRegistry` — the payee allowlist CORE cannot express.
//!
//! CORE's grant answers "how much, through which program, until when". It has no
//! notion of a recipient, because for trading the recipient is the point: any
//! mint, any pool. For paying people that is exactly backwards. A subscription
//! agent should be able to pay Helio and nobody else, and no combination of cap,
//! expiry and program allowlist can say that.
//!
//! So the allowlist lives here, root-owned, one registry per GrokAccount:
//!
//!   * only the root can add or remove a merchant — the agent never touches it
//!   * `pay_token` refuses a destination whose OWNER is not on the list
//!   * the registry pins ONE mint, which is what makes a grant cap denominate a
//!     single asset (see the CAP UNITS note in pay_token.rs)
//!
//! The security property: a stolen agent key can spend up to the cap, but only
//! to merchants the human already approved. That turns a total loss into a
//! bounded one, and it is the reason this is safe enough to leave running.
//!
//! MERGE NOTE: written against the public crate at 29e5787, NOT COMPILED.

use anchor_lang::prelude::*;

use grok_chain_core::{GrokAccount, SEED_GROK_ACCOUNT};

use crate::constants::{MAX_MERCHANTS, SEED_MERCHANTS};
use crate::errors::IntentsError;
use crate::events::{MerchantAdded, MerchantRegistryInitialized, MerchantRemoved};
use crate::state::MerchantRegistry;

pub fn init(ctx: Context<InitMerchantRegistry>, mint: Pubkey) -> Result<()> {
    let reg = &mut ctx.accounts.merchant_registry;
    reg.grok_account = ctx.accounts.grok_account.key();
    reg.root = ctx.accounts.root.key();
    reg.mint = mint;
    reg.bump = ctx.bumps.merchant_registry;
    reg.merchants = Vec::new();

    emit!(MerchantRegistryInitialized {
        merchant_registry: reg.key(),
        grok_account: reg.grok_account,
        root: reg.root,
        mint,
    });
    Ok(())
}

pub fn add(ctx: Context<UpdateMerchantRegistry>, merchant: Pubkey) -> Result<()> {
    let reg = &mut ctx.accounts.merchant_registry;
    require!(
        reg.merchants.len() < MAX_MERCHANTS,
        IntentsError::MerchantRegistryFull
    );
    require!(
        !reg.merchants.iter().any(|m| *m == merchant),
        IntentsError::MerchantAlreadyListed
    );
    reg.merchants.push(merchant);

    emit!(MerchantAdded {
        merchant_registry: reg.key(),
        grok_account: reg.grok_account,
        root: reg.root,
        merchant,
        count: reg.merchants.len() as u32,
    });
    Ok(())
}

/// Removing a merchant takes effect immediately: the next `pay_token` to it
/// fails. This is the per-merchant cancel, distinct from revoking the grant,
/// which would stop every payment at once.
pub fn remove(ctx: Context<UpdateMerchantRegistry>, merchant: Pubkey) -> Result<()> {
    let reg = &mut ctx.accounts.merchant_registry;
    let before = reg.merchants.len();
    reg.merchants.retain(|m| *m != merchant);
    require!(
        reg.merchants.len() < before,
        IntentsError::MerchantNotListed
    );

    emit!(MerchantRemoved {
        merchant_registry: reg.key(),
        grok_account: reg.grok_account,
        root: reg.root,
        merchant,
        count: reg.merchants.len() as u32,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct InitMerchantRegistry<'info> {
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
        space = MerchantRegistry::SPACE,
        seeds = [SEED_MERCHANTS, grok_account.key().as_ref()],
        bump
    )]
    pub merchant_registry: Account<'info, MerchantRegistry>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateMerchantRegistry<'info> {
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
        seeds = [SEED_MERCHANTS, grok_account.key().as_ref()],
        bump = merchant_registry.bump,
        constraint = merchant_registry.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = merchant_registry.root == root.key() @ IntentsError::UnauthorizedRoot,
    )]
    pub merchant_registry: Account<'info, MerchantRegistry>,
}
