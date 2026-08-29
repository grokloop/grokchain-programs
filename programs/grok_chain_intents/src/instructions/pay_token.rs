//! `pay_token` — grant-gated SPL / Token-2022 payment to a real payee.
//!
//! WHY THIS EXISTS
//! Nothing in the program can pay a third party in a token. `pay` moves native
//! lamports only. `token_buy` / `token_sell` route through Jupiter, but their
//! destination must satisfy `acc_owner == trader`, so the desk can ACQUIRE USDC
//! and never spend it. Subscriptions and checkout need the opposite: move a
//! stablecoin OUT, to someone else, under a cap the human set.
//!
//! WHAT IT KEEPS FROM THE EXISTING MOUTH
//! Agent signs and is never the fee payer. Relayer fee-pays. One CORE
//! `check_grant` before anything moves, abort on CORE error. Optional sponsor
//! from the human's paymaster. Custody is the same `pump-trader` PDA, which only
//! this program can sign for and only the root can withdraw from.
//!
//! WHAT IT ADDS
//! 1. A payee allowlist. CORE's grant constrains `allowed_programs`, never a
//!    recipient — for trading that was the point, for paying people it is the
//!    wrong axis. `MerchantRegistry` is a root-owned list; `pay_token` refuses
//!    any destination whose owner is not on it. A stolen agent key can then only
//!    pay merchants the human already approved.
//! 2. `TransferChecked`, not `Transfer`. Token-2022 rejects plain `Transfer` for
//!    mints carrying a transfer fee, a hook, or non-transferable. Decimals are
//!    read from the mint rather than trusted from args.
//! 3. A Solana Pay `reference` passthrough. Merchants reconcile an invoice by
//!    finding that pubkey among a transaction's accounts. It is read-only and
//!    does nothing on chain, but without a slot for it a merchant cannot tell
//!    which order was paid.
//!
//! CAP UNITS — READ THIS
//! CORE meters `spend_cap_lamports`, a u64 with no notion of which asset. This
//! instruction meters the RAW TOKEN AMOUNT against that counter. For a USDC
//! grant a cap of 50_000_000 therefore means 50 USDC, not 0.05 SOL. That is only
//! coherent while an agent spends ONE asset, so the registry pins a single mint
//! and a second denomination needs a second agent. A grant is per
//! (grok_account, agent), so that costs nothing and keeps the cap meaningful.
//!
//! STATUS: live on Solana mainnet since slot 442622147, and exercised — a real
//! 0.01 USDC payment settled in
//! 4nhDmpmyzMu9UkRcMphHb41fkgu7hXa3CfwvZ4SKDBcTMDLbHytJuAm46Hz6suFLCRu5Rw1fTipyjJUSnv1WxBBx
//! with the relayer as fee payer and the agent holding nothing.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

use grok_chain_core::{Grant, GrokAccount, SEED_GRANT, SEED_GROK_ACCOUNT};

use crate::constants::{
    SEED_MERCHANTS, SEED_PAYMASTER, SEED_PUMP_TRADER, SEED_SPEND_VAULT, TOKEN_2022_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
};
use crate::core_cpi;
use crate::errors::IntentsError;
use crate::events::TokenPaid;
use crate::instructions::common;
use crate::state::{MerchantRegistry, PayTokenArgs, Paymaster, SpendVault};

/// SPL Token / Token-2022 `TransferChecked`.
const TOKEN_IX_TRANSFER_CHECKED: u8 = 12;
/// SPL mint layout: mint_authority(36) + supply(8) + decimals(1).
const MINT_DECIMALS_OFFSET: usize = 44;
/// SPL token account layout: mint(32) + owner(32) + amount(8).
const TOKEN_ACCOUNT_MIN_LEN: usize = 165;

pub fn handler(ctx: Context<PayToken>, args: PayTokenArgs) -> Result<()> {
    require!(args.amount > 0, IntentsError::ZeroAmount);
    common::precheck_sponsor(
        args.sponsor_lamports,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
    )?;

    let trader = ctx.accounts.pump_trader.key();
    let mint_key = ctx.accounts.mint.key();
    let registry = &ctx.accounts.merchant_registry;

    // The registry pins one mint, so a grant cap always denominates one asset.
    require_keys_eq!(
        registry.mint,
        mint_key,
        IntentsError::PayTokenMintNotRegistered
    );

    // Source must be the trader's own account for this mint.
    let (src_mint, src_owner, src_amount) =
        token_account_fields(&ctx.accounts.source.to_account_info())?;
    require_keys_eq!(src_mint, mint_key, IntentsError::PayTokenMintMismatch);
    require_keys_eq!(src_owner, trader, IntentsError::PayTokenSourceOwnerNotTrader);
    require!(src_amount >= args.amount, IntentsError::PayTokenInsufficient);

    // Destination must hold the SAME mint and be owned by an approved merchant.
    // This is the control CORE's grant cannot express.
    let (dst_mint, dst_owner, _) =
        token_account_fields(&ctx.accounts.destination.to_account_info())?;
    require_keys_eq!(dst_mint, mint_key, IntentsError::PayTokenMintMismatch);
    require!(
        registry.merchants.iter().any(|m| *m == dst_owner),
        IntentsError::PayTokenPayeeNotAllowed
    );

    // Decimals come from the mint, never the caller: TransferChecked compares
    // them on chain, and a mismatch there is an opaque failure.
    let decimals = mint_decimals(&ctx.accounts.mint.to_account_info())?;
    require!(
        args.decimals == decimals,
        IntentsError::PayTokenDecimalsMismatch
    );

    // CORE meters the RAW TOKEN AMOUNT (see CAP UNITS above). Abort on error.
    core_cpi::check_grant(
        ctx.accounts.grok_chain_core_program.to_account_info(),
        ctx.accounts.grok_account.to_account_info(),
        ctx.accounts.grant.to_account_info(),
        ctx.accounts.agent.to_account_info(),
        ctx.accounts.intents_program.to_account_info(),
        args.amount,
    )?;

    let grok = ctx.accounts.grok_account.key();
    let bump = [ctx.bumps.pump_trader];
    let trader_signer_seeds: &[&[u8]] = &[SEED_PUMP_TRADER, grok.as_ref(), bump.as_ref()];

    let token_program = ctx.accounts.token_program.to_account_info();
    require!(
        *token_program.key == TOKEN_PROGRAM_ID || *token_program.key == TOKEN_2022_PROGRAM_ID,
        IntentsError::InvalidTokenProgram
    );
    // Both accounts must belong to the program being invoked, or Token-2022
    // state would be handed to the classic program.
    require!(
        ctx.accounts.source.to_account_info().owner == token_program.key,
        IntentsError::InvalidTokenProgram
    );
    require!(
        ctx.accounts.destination.to_account_info().owner == token_program.key,
        IntentsError::InvalidTokenProgram
    );

    let mut data = Vec::with_capacity(10);
    data.push(TOKEN_IX_TRANSFER_CHECKED);
    data.extend_from_slice(&args.amount.to_le_bytes());
    data.push(decimals);

    let ix = Instruction {
        program_id: *token_program.key,
        accounts: vec![
            AccountMeta::new(ctx.accounts.source.key(), false),
            AccountMeta::new_readonly(mint_key, false),
            AccountMeta::new(ctx.accounts.destination.key(), false),
            AccountMeta::new_readonly(trader, true),
        ],
        data,
    };
    invoke_signed(
        &ix,
        &[
            ctx.accounts.source.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.destination.to_account_info(),
            ctx.accounts.pump_trader.to_account_info(),
            token_program.clone(),
        ],
        &[trader_signer_seeds],
    )?;

    common::reimburse_sponsor(
        &ctx.accounts.grant,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
        args.sponsor_lamports,
    )?;

    emit!(TokenPaid {
        trader,
        mint: mint_key,
        destination: ctx.accounts.destination.key(),
        payee: dst_owner,
        amount: args.amount,
        decimals,
        // Solana Pay reconciliation key, echoed so an indexer can match the
        // invoice without decoding the account list.
        reference: ctx.accounts.reference.as_ref().map(|r| r.key()),
        agent: ctx.accounts.agent.key(),
        grant: ctx.accounts.grant.key(),
        generation: ctx.accounts.grant.generation,
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
        IntentsError::PayTokenAccountInvalid
    );
    let mint = Pubkey::try_from(&data[0..32])
        .map_err(|_| error!(IntentsError::PayTokenAccountInvalid))?;
    let owner = Pubkey::try_from(&data[32..64])
        .map_err(|_| error!(IntentsError::PayTokenAccountInvalid))?;
    let amount = u64::from_le_bytes(data[64..72].try_into().unwrap());
    Ok((mint, owner, amount))
}

fn mint_decimals(info: &AccountInfo) -> Result<u8> {
    require!(
        *info.owner == TOKEN_PROGRAM_ID || *info.owner == TOKEN_2022_PROGRAM_ID,
        IntentsError::InvalidTokenProgram
    );
    let data = info.try_borrow_data()?;
    require!(
        data.len() > MINT_DECIMALS_OFFSET,
        IntentsError::PayTokenAccountInvalid
    );
    Ok(data[MINT_DECIMALS_OFFSET])
}

#[derive(Accounts)]
pub struct PayToken<'info> {
    /// Grant agent. Signs, never `mut`: it cannot be the fee payer here.
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
    /// CHECK: this program. CORE allowlists this id, not the merchant.
    #[account(address = crate::ID)]
    pub intents_program: UncheckedAccount<'info>,
    /// Present for mouth parity. Never debited by this instruction.
    #[account(
        seeds = [SEED_SPEND_VAULT, grok_account.key().as_ref()],
        bump = spend_vault.bump,
        constraint = spend_vault.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
    )]
    pub spend_vault: Account<'info, SpendVault>,
    #[account(
        seeds = [SEED_MERCHANTS, grok_account.key().as_ref()],
        bump = merchant_registry.bump,
        constraint = merchant_registry.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = merchant_registry.root == grok_account.root @ IntentsError::UnauthorizedRoot,
    )]
    pub merchant_registry: Account<'info, MerchantRegistry>,
    /// CHECK: system-owned custody PDA; the only key this program signs for.
    #[account(
        seeds = [SEED_PUMP_TRADER, grok_account.key().as_ref()],
        bump,
    )]
    pub pump_trader: UncheckedAccount<'info>,
    /// CHECK: trader's token account for `mint`. Verified in the handler.
    #[account(mut)]
    pub source: UncheckedAccount<'info>,
    /// CHECK: merchant's token account for `mint`. Owner must be registered.
    #[account(mut)]
    pub destination: UncheckedAccount<'info>,
    /// CHECK: the mint. Decimals are read from it for TransferChecked.
    pub mint: UncheckedAccount<'info>,
    /// CHECK: classic Token or Token-2022; must own both token accounts.
    pub token_program: UncheckedAccount<'info>,
    /// CHECK: Solana Pay reference. Read-only, never written, never signed — it
    /// exists so a merchant can match this payment to an invoice.
    pub reference: Option<UncheckedAccount<'info>>,
    #[account(
        mut,
        seeds = [SEED_PAYMASTER, grok_account.key().as_ref()],
        bump = paymaster.bump,
        constraint = paymaster.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
    )]
    pub paymaster: Option<Account<'info, Paymaster>>,
    /// Relayer / outer fee payer. Must sign when present.
    #[account(mut)]
    pub fee_payer: Option<Signer<'info>>,
}
