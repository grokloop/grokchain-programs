use anchor_lang::prelude::*;

use crate::constants::MAX_MERCHANTS;

/// SPEC.md §5.2. Field order is the Borsh order. Space = 73.
/// SOL custody is the lamports on this PDA (program-owned). No shadow balance.
#[account]
pub struct SpendVault {
    pub grok_account: Pubkey,
    pub root: Pubkey,
    pub bump: u8,
}

impl SpendVault {
    pub const SPACE: usize = 8 + 32 + 32 + 1; // 73
}

/// SPEC.md §5.3. Field order is the Borsh order. Space = 106.
/// SOL custody is the lamports on this PDA (program-owned). No shadow balance.
#[account]
pub struct Paymaster {
    pub grok_account: Pubkey,
    pub root: Pubkey,
    pub relayer: Pubkey,
    pub bump: u8,
    pub paused: bool,
}

impl Paymaster {
    pub const SPACE: usize = 8 + 32 + 32 + 32 + 1 + 1; // 106
}

/// SPEC.md §5.4. `pay` args. Wire: 8-byte disc + Borsh two u64s.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PayArgs {
    pub amount_lamports: u64,
    pub sponsor_lamports: u64,
}

/// `swap` args. Wire: disc + Borsh three u64s.
/// v1 is SOL-only: amount_in is debited from SpendVault to out_destination.
/// `amount_in >= min_out` is the honest min check. Not an AMM quote.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SwapArgs {
    pub amount_in_lamports: u64,
    pub min_out_lamports: u64,
    pub sponsor_lamports: u64,
}

/// `deploy` args. Wire: disc + Borsh u64 + Pubkey.
/// `program_id` is recorded in DeployRequested. This is not a BPF deploy.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DeployArgs {
    pub sponsor_lamports: u64,
    pub program_id: Pubkey,
}

/// `call` args. Wire: disc + Borsh two u64s + Pubkey.
/// `target_program` is the inner program remaining_accounts are invoked into.
/// CORE check_grant still uses this INTENTS router as target_program.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CallArgs {
    pub amount_lamports: u64,
    pub sponsor_lamports: u64,
    pub target_program: Pubkey,
}

/// `pump_buy` args. Wire: disc + Borsh three u64s.
/// `max_sol_cost` is the grant SOL budget (`check_grant` amount).
/// Inner pump ix data is built on-chain (buy_v2 disc + amount + max_sol_cost).
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PumpBuyArgs {
    pub amount: u64,
    pub max_sol_cost: u64,
    pub sponsor_lamports: u64,
}

/// `pump_sell` args. Wire: disc + Borsh three u64s.
/// Grant amount is 0: sell spends tokens, not SOL (see policy::PUMP_SELL_CHECK_GRANT_AMOUNT).
/// Inner pump ix data is built on-chain (sell_v2 disc + amount + min_sol_output).
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PumpSellArgs {
    pub amount: u64,
    pub min_sol_output: u64,
    pub sponsor_lamports: u64,
}

/// `pump_create` args. Wire: disc + Borsh (name, symbol, uri, flags, max_sol_cost, sponsor).
/// Inner create_v2 data is built on-chain. `creator` is `grok_account.root`, not an arg.
/// `max_sol_cost` is the grant SOL budget (rent + create fees).
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PumpCreateArgs {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub is_mayhem_mode: bool,
    pub is_cashback_enabled: bool,
    pub max_sol_cost: u64,
    pub sponsor_lamports: u64,
}


/// `pump_amm_buy` args. Wire: disc + Borsh four u64s.
/// Inner PumpSwap ix data is built on-chain (buy_exact_quote_in disc +
/// spendable_quote_in + min_base_amount_out + track_volume).
/// `max_sol_cost` is the grant SOL budget and vault debit (`>= spendable_quote_in`).
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PumpAmmBuyArgs {
    pub spendable_quote_in: u64,
    pub min_base_amount_out: u64,
    pub max_sol_cost: u64,
    pub sponsor_lamports: u64,
}

/// `pump_amm_sell` args. Wire: disc + Borsh three u64s.
/// Mirrors PumpSellArgs honesty: sell spends base tokens, not SOL.
/// Grant amount is 0 (see policy::PUMP_AMM_SELL_CHECK_GRANT_AMOUNT).
/// Inner PumpSwap ix data is built on-chain (sell disc + base_amount_in +
/// min_quote_amount_out). Never client raw bytes.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PumpAmmSellArgs {
    pub base_amount_in: u64,
    pub min_quote_amount_out: u64,
    pub sponsor_lamports: u64,
}

/// `token_buy` args. Wire: disc + Borsh two u64s + sponsor + two pubkeys +
/// wrap_sol + Jupiter swap-instructions data (never empty).
/// Inner program is hardcoded Jupiter v6. `jupiter_data` is official
/// swap-instruction bytes from Jupiter. Do not take a raw client program id.
/// `in_amount` must match the amount encoded in `jupiter_data`.
/// Paying with native SOL/WSOL: check_grant(in_amount). Paying with a token
/// already on the trader (USDC or other): check_grant(0). wrap_sol wraps
/// native SOL onto the trader WSOL ATA. Adapter does not create ATAs.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TokenBuyArgs {
    pub in_amount: u64,
    pub min_out: u64,
    pub sponsor_lamports: u64,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub wrap_sol: bool,
    pub jupiter_data: Vec<u8>,
}

/// `token_sell` args. Same wire as TokenBuyArgs.
/// Selling tokens for quote: check_grant(0). Selling WSOL/SOL for USDC (or
/// another quote): check_grant(in_amount). wrap_sol when the input is native SOL.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TokenSellArgs {
    pub in_amount: u64,
    pub min_out: u64,
    pub sponsor_lamports: u64,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub wrap_sol: bool,
    pub jupiter_data: Vec<u8>,
}

/// Root-owned payee allowlist, one per GrokAccount, pinned to a single mint.
///
/// The mint is pinned because CORE meters one `u64` with no notion of asset:
/// `pay_token` spends the grant cap in RAW TOKEN UNITS, which is only coherent
/// while an agent spends one denomination. A second asset means a second agent.
#[account]
pub struct MerchantRegistry {
    pub grok_account: Pubkey,
    pub root: Pubkey,
    pub mint: Pubkey,
    pub bump: u8,
    pub merchants: Vec<Pubkey>,
}

impl MerchantRegistry {
    pub const SPACE: usize = 8 + 32 + 32 + 32 + 1 + 4 + 32 * MAX_MERCHANTS;
}

/// `pay_token` args. Wire: u64 amount + u8 decimals + u64 sponsor.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PayTokenArgs {
    /// Raw token units. Metered against the grant cap.
    pub amount: u64,
    /// Must equal the mint's own decimals; TransferChecked compares them.
    pub decimals: u8,
    pub sponsor_lamports: u64,
}

/// A recurring payment. `last_paid_period` is the idempotency key: it advances
/// inside the same transaction that moves the money, so a retry cannot pay
/// twice. `-1` means never paid; periods are 0-indexed from `start_unix`.
#[account]
pub struct Subscription {
    pub grok_account: Pubkey,
    pub root: Pubkey,
    pub merchant: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub period_seconds: i64,
    pub start_unix: i64,
    pub last_paid_period: i64,
    pub payments: u32,
    pub active: bool,
    pub bump: u8,
}

impl Subscription {
    pub const SPACE: usize = 8 + 32 + 32 + 32 + 32 + 8 + 8 + 8 + 8 + 4 + 1 + 1;
}

/// `create_subscription` args. Wire: Pubkey + u64 + i64 + i64.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SubscriptionArgs {
    pub merchant: Pubkey,
    pub amount: u64,
    pub period_seconds: i64,
    /// 0 (or any past value) means "start now".
    pub start_unix: i64,
}

/// `pay_subscription` args. Wire: i64 period + u64 sponsor.
///
/// The period is asserted by the caller rather than inferred, so a scheduler
/// whose clock has drifted fails loudly instead of paying the wrong cycle.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PaySubscriptionArgs {
    pub period: i64,
    pub sponsor_lamports: u64,
}
