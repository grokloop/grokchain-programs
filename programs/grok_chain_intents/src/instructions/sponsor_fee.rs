//! Sponsorship fees in $GrokChain, and the burn that consumes them.
//!
//! NOT DEPLOYED. The live MAINNET binary does not contain these instructions.
//! Nothing here charges anybody anything today, and the default fee is zero even
//! after a deploy, so shipping this changes nothing until a root deliberately
//! sets a rate. Read this file as a design that compiles, not as a live feature.
//!
//! WHAT IT DOES
//! Gasless execution is not free — the relayer fronts SOL for every sponsored
//! intent and the paymaster reimburses it. That cost is real and someone bears
//! it. This adds a second, optional leg: an agent using the paymaster also pays
//! a fee denominated in $GrokChain, which accumulates in a vault whose only exit
//! is destruction.
//!
//! WHY BURNING IS A PROPERTY HERE, NOT A PROMISE
//! There is deliberately no withdraw instruction. Not a root-gated one, not a
//! timelocked one, not one behind a multisig. The fee vault is a PDA-owned token
//! account and the single instruction that can move its balance is
//! `burn_collected_fees`, which passes it to Token-2022's BurnChecked and reduces
//! total supply.
//!
//! That distinction matters more than any announcement about it. A treasury with
//! a withdraw function that the team promises not to use is a promise. A vault
//! with no withdraw function is arithmetic. Anyone can verify which this is by
//! searching the file for a transfer out of the vault and not finding one.
//!
//! `burn_collected_fees` is permissionless on purpose. If only the root could
//! fire it, the burn would happen when the root felt like it, and "will be
//! burned" would again be a promise. Anyone may call it, it always burns the
//! entire balance, and there is no parameter to make it burn less.
//!
//! WHAT IT IS NOT
//! It is not a yield mechanism, a buyback, or a claim on anything. Burned tokens
//! are gone; nobody receives them. Supply falls and that is the whole effect.
//! Whether that matters depends entirely on whether the paymaster is ever used
//! enough to collect a meaningful amount, which is a question about adoption and
//! not one this file can answer.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

use grok_chain_core::{GrokAccount, SEED_GROK_ACCOUNT};

use crate::constants::{
    GROK_TOKEN_MINT, SEED_FEE_VAULT, SEED_SPONSOR_FEE, TOKEN_2022_PROGRAM_ID,
};
use crate::errors::IntentsError;
use crate::events::{SponsorFeeCollected, SponsorFeeSet, SponsorFeesBurned};
use crate::state::SponsorFeeConfig;

/// Token-2022 BurnChecked. The checked variant compares the decimals we pass
/// against the mint's own, so a wrong-decimals bug cannot silently burn a
/// different magnitude than intended.
const TOKEN_IX_BURN_CHECKED: u8 = 15;
const MINT_DECIMALS_OFFSET: usize = 44;
const TOKEN_ACCOUNT_MIN_LEN: usize = 165;

/// A ceiling on what a root may set, so a fee cannot quietly become
/// confiscatory for agents already relying on the paymaster. 10 $GrokChain per
/// sponsored intent at 6 decimals.
pub const MAX_FEE_PER_INTENT: u64 = 10_000_000;

/// Create the config. Separate from setting the rate because `init_if_needed`
/// requires a crate feature that arms a re-initialisation footgun across every
/// other account in this program; the merchant registry splits init from mutate
/// for the same reason.
pub fn init_fee(ctx: Context<InitSponsorFee>) -> Result<()> {
    let cfg = &mut ctx.accounts.sponsor_fee;
    cfg.grok_account = ctx.accounts.grok_account.key();
    cfg.root = ctx.accounts.root.key();
    cfg.fee_per_intent = 0;
    cfg.bump = ctx.bumps.sponsor_fee;

    emit!(SponsorFeeSet {
        grok_account: cfg.grok_account,
        root: cfg.root,
        fee_per_intent: 0,
    });
    Ok(())
}

/// Root sets the rate. Zero disables the fee entirely, and zero is what `init`
/// leaves behind: an account that never calls this is never charged.
pub fn set_fee(ctx: Context<SetSponsorFee>, fee_per_intent: u64) -> Result<()> {
    require!(
        fee_per_intent <= MAX_FEE_PER_INTENT,
        IntentsError::SponsorFeeTooHigh
    );
    let cfg = &mut ctx.accounts.sponsor_fee;
    cfg.fee_per_intent = fee_per_intent;

    emit!(SponsorFeeSet {
        grok_account: cfg.grok_account,
        root: cfg.root,
        fee_per_intent,
    });
    Ok(())
}

/**
 * Burn everything the vault holds.
 *
 * Permissionless, takes no amount, and always burns the full balance. There is
 * no partial burn and no recipient — the tokens cease to exist and total supply
 * falls by exactly what was collected.
 *
 * Burning zero is refused rather than treated as a no-op, so a caller cannot
 * emit a `SponsorFeesBurned` event that says nothing happened.
 */
pub fn burn_collected_fees(ctx: Context<BurnCollectedFees>) -> Result<()> {
    let vault = &ctx.accounts.fee_vault.to_account_info();
    let (mint, owner, amount) = token_account_fields(vault)?;

    require_keys_eq!(mint, GROK_TOKEN_MINT, IntentsError::SponsorFeeMintMismatch);
    let grok = ctx.accounts.grok_account.key();
    let (expected_authority, bump_u8) = Pubkey::find_program_address(
        &[SEED_FEE_VAULT, grok.as_ref()],
        ctx.program_id,
    );
    require_keys_eq!(
        owner,
        expected_authority,
        IntentsError::SponsorFeeVaultOwnerMismatch
    );
    require!(amount > 0, IntentsError::NothingToBurn);

    let decimals = mint_decimals(&ctx.accounts.mint.to_account_info())?;

    let mut data = Vec::with_capacity(10);
    data.push(TOKEN_IX_BURN_CHECKED);
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(decimals);

    let ix = Instruction {
        program_id: TOKEN_2022_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(vault.key(), false),
            AccountMeta::new(GROK_TOKEN_MINT, false),
            AccountMeta::new_readonly(expected_authority, true),
        ],
        data,
    };

    let bump = [bump_u8];
    let signer_seeds: &[&[u8]] = &[SEED_FEE_VAULT, grok.as_ref(), bump.as_ref()];
    invoke_signed(
        &ix,
        &[
            vault.clone(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.fee_vault_authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
        &[signer_seeds],
    )?;

    // Read the balance back rather than trusting the CPI returned cleanly. A
    // burn that did not burn should not emit an event saying it did.
    let (_, _, after) = token_account_fields(vault)?;
    require!(after == 0, IntentsError::BurnIncomplete);

    emit!(SponsorFeesBurned {
        grok_account: grok,
        amount,
        burner: ctx.accounts.burner.key(),
    });
    Ok(())
}

/// Emitted by the sponsored-intent path once this is wired in. Kept here so the
/// accounting lives with the burn it feeds.
pub fn note_collected(grok_account: Pubkey, agent: Pubkey, amount: u64) {
    if amount == 0 {
        return;
    }
    emit!(SponsorFeeCollected {
        grok_account,
        agent,
        amount,
    });
}

fn token_account_fields(info: &AccountInfo) -> Result<(Pubkey, Pubkey, u64)> {
    require_keys_eq!(
        *info.owner,
        TOKEN_2022_PROGRAM_ID,
        IntentsError::InvalidTokenProgram
    );
    let data = info.try_borrow_data()?;
    require!(
        data.len() >= TOKEN_ACCOUNT_MIN_LEN,
        IntentsError::SponsorFeeVaultMalformed
    );
    let mint = Pubkey::try_from(&data[0..32]).map_err(|_| error!(IntentsError::SponsorFeeVaultMalformed))?;
    let owner = Pubkey::try_from(&data[32..64]).map_err(|_| error!(IntentsError::SponsorFeeVaultMalformed))?;
    let amount = u64::from_le_bytes(
        data[64..72]
            .try_into()
            .map_err(|_| error!(IntentsError::SponsorFeeVaultMalformed))?,
    );
    Ok((mint, owner, amount))
}

fn mint_decimals(info: &AccountInfo) -> Result<u8> {
    require_keys_eq!(
        *info.owner,
        TOKEN_2022_PROGRAM_ID,
        IntentsError::InvalidTokenProgram
    );
    require_keys_eq!(info.key(), GROK_TOKEN_MINT, IntentsError::SponsorFeeMintMismatch);
    let data = info.try_borrow_data()?;
    require!(
        data.len() > MINT_DECIMALS_OFFSET,
        IntentsError::SponsorFeeVaultMalformed
    );
    Ok(data[MINT_DECIMALS_OFFSET])
}

#[derive(Accounts)]
pub struct InitSponsorFee<'info> {
    #[account(mut)]
    pub root: Signer<'info>,
    #[account(
        seeds = [SEED_GROK_ACCOUNT, root.key().as_ref()],
        seeds::program = grok_chain_core::ID,
        bump,
        constraint = grok_account.root == root.key() @ IntentsError::UnauthorizedRoot,
    )]
    pub grok_account: Account<'info, GrokAccount>,
    #[account(
        init,
        payer = root,
        space = SponsorFeeConfig::SPACE,
        seeds = [SEED_SPONSOR_FEE, grok_account.key().as_ref()],
        bump,
    )]
    pub sponsor_fee: Account<'info, SponsorFeeConfig>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetSponsorFee<'info> {
    pub root: Signer<'info>,
    #[account(
        seeds = [SEED_GROK_ACCOUNT, root.key().as_ref()],
        seeds::program = grok_chain_core::ID,
        bump,
        constraint = grok_account.root == root.key() @ IntentsError::UnauthorizedRoot,
    )]
    pub grok_account: Account<'info, GrokAccount>,
    #[account(
        mut,
        seeds = [SEED_SPONSOR_FEE, grok_account.key().as_ref()],
        bump = sponsor_fee.bump,
        constraint = sponsor_fee.root == root.key() @ IntentsError::UnauthorizedRoot,
    )]
    pub sponsor_fee: Account<'info, SponsorFeeConfig>,
}

#[derive(Accounts)]
pub struct BurnCollectedFees<'info> {
    /// Anyone. Pays the transaction fee and receives nothing for it.
    #[account(mut)]
    pub burner: Signer<'info>,
    /// CHECK: identifies which vault to burn; validated by the PDA derivation.
    pub grok_account: UncheckedAccount<'info>,
    /// CHECK: the $GrokChain mint, pinned to the constant in mint_decimals.
    #[account(mut)]
    pub mint: UncheckedAccount<'info>,
    /// CHECK: token account whose mint, owner and balance are read directly.
    #[account(mut)]
    pub fee_vault: UncheckedAccount<'info>,
    /// CHECK: PDA that owns the vault; re-derived before signing.
    #[account(
        seeds = [SEED_FEE_VAULT, grok_account.key().as_ref()],
        bump,
    )]
    pub fee_vault_authority: UncheckedAccount<'info>,
    /// CHECK: hardcoded allowlist.
    #[account(address = TOKEN_2022_PROGRAM_ID @ IntentsError::InvalidTokenProgram)]
    pub token_program: UncheckedAccount<'info>,
}
