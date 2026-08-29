use anchor_lang::prelude::*;

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
