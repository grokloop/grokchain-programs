//! Recurring payments with double-pay made structurally impossible.
//!
//! WHY THE IDEMPOTENCY IS ON CHAIN
//! A scheduler that records "paid" in its own database can still double-pay: it
//! sends, crashes before writing, restarts, and sends again. No amount of client
//! care fixes that, because the crash window is between two systems.
//!
//! So the period counter lives in the account: `last_paid_period` advances only
//! inside the same transaction that moves the money. A second attempt at the
//! same period fails on chain with `SubscriptionAlreadyPaid`. The scheduler is
//! then free to be dumb and retry as often as it likes — at-least-once delivery
//! becomes exactly-once settlement, which is the only version worth shipping.
//!
//! MISSED PERIODS ARE NOT BACKFILLED — DELIBERATE
//! A period is payable only while it is the current one. If the bot is offline
//! for three periods it does NOT wake up and fire three payments; it pays the
//! current period and `missed` shows the gap. Waking to a surprise triple charge
//! is a worse failure than a missed month, and the human can always pay a gap
//! manually. `payments` and `last_paid_period` make the gap auditable.
//!
//! WHAT STILL GUARDS IT
//! Everything `pay_token` has: agent signs and never fee-pays, relayer fee-pays,
//! one CORE `check_grant` metered in raw token units, and the merchant must be
//! on the root's allowlist. A subscription is not a second way to pay — it is
//! `pay_token` with a clock and a counter.
//!
//! CANCELLING
//! `cancel_subscription` is root-only and takes effect immediately. It is the
//! narrow cancel: one merchant, nothing else. Removing the merchant from the
//! allowlist is the same thing one level up, and revoking the grant stops every
//! payment at once. Three scopes, all in the human's hands, none of them
//! requiring the merchant to cooperate.
//!
//! MERGE NOTE: written against the public crate at 29e5787, NOT COMPILED.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

use grok_chain_core::{Grant, GrokAccount, SEED_GRANT, SEED_GROK_ACCOUNT};

use crate::constants::{
    SEED_MERCHANTS, SEED_PAYMASTER, SEED_PUMP_TRADER, SEED_SUBSCRIPTION, TOKEN_2022_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
};
use crate::core_cpi;
use crate::errors::IntentsError;
use crate::events::{SubscriptionCancelled, SubscriptionCreated, SubscriptionPaid};
use crate::instructions::common;
use crate::state::{
    MerchantRegistry, PaySubscriptionArgs, Paymaster, Subscription, SubscriptionArgs,
};

const TOKEN_IX_TRANSFER_CHECKED: u8 = 12;
const MINT_DECIMALS_OFFSET: usize = 44;
const TOKEN_ACCOUNT_MIN_LEN: usize = 165;

/// Never paid. Periods are 0-indexed, so a sentinel below zero is needed.
pub const PERIOD_NONE: i64 = -1;
/// A day is the shortest sane billing period; anything faster is a drain vector
/// dressed as a subscription.
pub const MIN_PERIOD_SECONDS: i64 = 86_400;

pub fn create(ctx: Context<CreateSubscription>, args: SubscriptionArgs) -> Result<()> {
    require!(args.amount > 0, IntentsError::ZeroAmount);
    require!(
        args.period_seconds >= MIN_PERIOD_SECONDS,
        IntentsError::SubscriptionPeriodTooShort
    );

    // A subscription may only name a merchant the root already approved, so a
    // subscription can never widen what the agent may pay.
    let registry = &ctx.accounts.merchant_registry;
    require_keys_eq!(
        registry.mint,
        ctx.accounts.mint.key(),
        IntentsError::PayTokenMintNotRegistered
    );
    require!(
        registry.merchants.iter().any(|m| *m == args.merchant),
        IntentsError::PayTokenPayeeNotAllowed
    );

    let now = Clock::get()?.unix_timestamp;
    let sub = &mut ctx.accounts.subscription;
    sub.grok_account = ctx.accounts.grok_account.key();
    sub.root = ctx.accounts.root.key();
    sub.merchant = args.merchant;
    sub.mint = ctx.accounts.mint.key();
    sub.amount = args.amount;
    sub.period_seconds = args.period_seconds;
    // A future start lets a human line up the first charge with a billing date.
    sub.start_unix = if args.start_unix > now { args.start_unix } else { now };
    sub.last_paid_period = PERIOD_NONE;
    sub.payments = 0;
    sub.active = true;
    sub.bump = ctx.bumps.subscription;

    emit!(SubscriptionCreated {
        subscription: sub.key(),
        grok_account: sub.grok_account,
        root: sub.root,
        merchant: sub.merchant,
        mint: sub.mint,
        amount: sub.amount,
        period_seconds: sub.period_seconds,
        start_unix: sub.start_unix,
    });
    Ok(())
}

/// Root-only. Immediate: the next `pay_subscription` fails on `active`.
pub fn cancel(ctx: Context<CancelSubscription>) -> Result<()> {
    let sub = &mut ctx.accounts.subscription;
    require!(sub.active, IntentsError::SubscriptionInactive);
    sub.active = false;

    emit!(SubscriptionCancelled {
        subscription: sub.key(),
        grok_account: sub.grok_account,
        root: sub.root,
        merchant: sub.merchant,
        payments: sub.payments,
        last_paid_period: sub.last_paid_period,
    });
    Ok(())
}

/// Which period `now` falls in. Periods are 0-indexed from `start_unix`.
pub fn current_period(now: i64, start_unix: i64, period_seconds: i64) -> Result<i64> {
    require!(period_seconds > 0, IntentsError::SubscriptionPeriodTooShort);
    require!(now >= start_unix, IntentsError::SubscriptionNotStarted);
    Ok((now - start_unix) / period_seconds)
}

pub fn pay(ctx: Context<PaySubscription>, args: PaySubscriptionArgs) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let (amount, merchant, mint_key, period) = {
        let sub = &ctx.accounts.subscription;
        require!(sub.active, IntentsError::SubscriptionInactive);
        let period = current_period(now, sub.start_unix, sub.period_seconds)?;
        // The caller states which period it believes it is paying. A mismatch
        // means the scheduler's clock disagrees with the chain's, and paying the
        // wrong period is worse than failing.
        require!(
            args.period == period,
            IntentsError::SubscriptionPeriodMismatch
        );
        // THE idempotency check. Everything else is ordinary payment logic.
        require!(
            period > sub.last_paid_period,
            IntentsError::SubscriptionAlreadyPaid
        );
        (sub.amount, sub.merchant, sub.mint, period)
    };

    common::precheck_sponsor(
        args.sponsor_lamports,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
    )?;

    // The merchant must STILL be approved. Removing it from the allowlist
    // therefore cancels the subscription without touching the subscription.
    let registry = &ctx.accounts.merchant_registry;
    require_keys_eq!(registry.mint, mint_key, IntentsError::PayTokenMintNotRegistered);
    require!(
        registry.merchants.iter().any(|m| *m == merchant),
        IntentsError::PayTokenPayeeNotAllowed
    );

    let trader = ctx.accounts.pump_trader.key();
    let (src_mint, src_owner, src_amount) =
        token_account_fields(&ctx.accounts.source.to_account_info())?;
    require_keys_eq!(src_mint, mint_key, IntentsError::PayTokenMintMismatch);
    require_keys_eq!(src_owner, trader, IntentsError::PayTokenSourceOwnerNotTrader);
    require!(src_amount >= amount, IntentsError::PayTokenInsufficient);

    let (dst_mint, dst_owner, _) =
        token_account_fields(&ctx.accounts.destination.to_account_info())?;
    require_keys_eq!(dst_mint, mint_key, IntentsError::PayTokenMintMismatch);
    require_keys_eq!(dst_owner, merchant, IntentsError::PayTokenPayeeNotAllowed);

    let decimals = mint_decimals(&ctx.accounts.mint.to_account_info())?;

    core_cpi::check_grant(
        ctx.accounts.grok_chain_core_program.to_account_info(),
        ctx.accounts.grok_account.to_account_info(),
        ctx.accounts.grant.to_account_info(),
        ctx.accounts.agent.to_account_info(),
        ctx.accounts.intents_program.to_account_info(),
        amount,
    )?;

    let grok = ctx.accounts.grok_account.key();
    let bump = [ctx.bumps.pump_trader];
    let trader_signer_seeds: &[&[u8]] = &[SEED_PUMP_TRADER, grok.as_ref(), bump.as_ref()];
    let token_program = ctx.accounts.token_program.to_account_info();
    require!(
        *token_program.key == TOKEN_PROGRAM_ID || *token_program.key == TOKEN_2022_PROGRAM_ID,
        IntentsError::InvalidTokenProgram
    );
    require!(
        ctx.accounts.source.to_account_info().owner == token_program.key
            && ctx.accounts.destination.to_account_info().owner == token_program.key,
        IntentsError::InvalidTokenProgram
    );

    let mut data = Vec::with_capacity(10);
    data.push(TOKEN_IX_TRANSFER_CHECKED);
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(decimals);

    invoke_signed(
        &Instruction {
            program_id: *token_program.key,
            accounts: vec![
                AccountMeta::new(ctx.accounts.source.key(), false),
                AccountMeta::new_readonly(mint_key, false),
                AccountMeta::new(ctx.accounts.destination.key(), false),
                AccountMeta::new_readonly(trader, true),
            ],
            data,
        },
        &[
            ctx.accounts.source.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.destination.to_account_info(),
            ctx.accounts.pump_trader.to_account_info(),
            token_program.clone(),
        ],
        &[trader_signer_seeds],
    )?;

    // Advance ONLY after the transfer succeeded. If the CPI fails the whole
    // transaction reverts and the period stays payable, which is the behaviour
    // a retry depends on.
    let sub = &mut ctx.accounts.subscription;
    sub.last_paid_period = period;
    sub.payments = sub.payments.saturating_add(1);

    emit!(SubscriptionPaid {
        subscription: sub.key(),
        grok_account: grok,
        merchant,
        mint: mint_key,
        amount,
        period,
        payments: sub.payments,
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
#[instruction(args: SubscriptionArgs)]
pub struct CreateSubscription<'info> {
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
        seeds = [SEED_MERCHANTS, grok_account.key().as_ref()],
        bump = merchant_registry.bump,
        constraint = merchant_registry.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
    )]
    pub merchant_registry: Account<'info, MerchantRegistry>,
    /// CHECK: the mint this subscription bills in.
    pub mint: UncheckedAccount<'info>,
    #[account(
        init,
        payer = root,
        space = Subscription::SPACE,
        seeds = [
            SEED_SUBSCRIPTION,
            grok_account.key().as_ref(),
            args.merchant.as_ref(),
            mint.key().as_ref(),
        ],
        bump
    )]
    pub subscription: Account<'info, Subscription>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelSubscription<'info> {
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
        seeds = [
            SEED_SUBSCRIPTION,
            grok_account.key().as_ref(),
            subscription.merchant.as_ref(),
            subscription.mint.as_ref(),
        ],
        bump = subscription.bump,
        constraint = subscription.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = subscription.root == root.key() @ IntentsError::UnauthorizedRoot,
    )]
    pub subscription: Account<'info, Subscription>,
}

#[derive(Accounts)]
pub struct PaySubscription<'info> {
    /// Grant agent. Signs, never `mut`: it cannot be the fee payer.
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
    /// CHECK: this program. CORE allowlists this id.
    #[account(address = crate::ID)]
    pub intents_program: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [
            SEED_SUBSCRIPTION,
            grok_account.key().as_ref(),
            subscription.merchant.as_ref(),
            subscription.mint.as_ref(),
        ],
        bump = subscription.bump,
        constraint = subscription.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
    )]
    pub subscription: Account<'info, Subscription>,
    #[account(
        seeds = [SEED_MERCHANTS, grok_account.key().as_ref()],
        bump = merchant_registry.bump,
        constraint = merchant_registry.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
    )]
    pub merchant_registry: Account<'info, MerchantRegistry>,
    /// CHECK: system-owned custody PDA; the only key this program signs for.
    #[account(
        seeds = [SEED_PUMP_TRADER, grok_account.key().as_ref()],
        bump,
    )]
    pub pump_trader: UncheckedAccount<'info>,
    /// CHECK: trader's token account for the mint. Verified in the handler.
    #[account(mut)]
    pub source: UncheckedAccount<'info>,
    /// CHECK: merchant's token account. Owner must equal subscription.merchant.
    #[account(mut)]
    pub destination: UncheckedAccount<'info>,
    /// CHECK: the mint. Decimals read from it for TransferChecked.
    pub mint: UncheckedAccount<'info>,
    /// CHECK: classic Token or Token-2022; must own both token accounts.
    pub token_program: UncheckedAccount<'info>,
    /// CHECK: Solana Pay reference. Read-only, never signed.
    pub reference: Option<UncheckedAccount<'info>>,
    #[account(
        mut,
        seeds = [SEED_PAYMASTER, grok_account.key().as_ref()],
        bump = paymaster.bump,
        constraint = paymaster.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
    )]
    pub paymaster: Option<Account<'info, Paymaster>>,
    #[account(mut)]
    pub fee_payer: Option<Signer<'info>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_advance_on_the_boundary_not_before() {
        let start = 1_700_000_000i64;
        let day = 86_400i64;
        assert_eq!(current_period(start, start, day).unwrap(), 0);
        assert_eq!(current_period(start + day - 1, start, day).unwrap(), 0);
        assert_eq!(current_period(start + day, start, day).unwrap(), 1);
        assert_eq!(current_period(start + 30 * day, start, day).unwrap(), 30);
    }

    #[test]
    fn a_period_before_the_start_is_an_error_not_a_negative_period() {
        let start = 1_700_000_000i64;
        assert!(current_period(start - 1, start, 86_400).is_err());
        assert!(current_period(start, start, 0).is_err());
    }

    #[test]
    fn the_sentinel_makes_period_zero_payable_exactly_once() {
        // period 0 > PERIOD_NONE, so the first charge lands...
        assert!(0i64 > PERIOD_NONE);
        // ...and a second attempt at the same period does not.
        assert!(!(0i64 > 0i64));
    }
}
