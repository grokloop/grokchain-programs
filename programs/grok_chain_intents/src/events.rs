use anchor_lang::prelude::*;

/// SPEC.md §7. Never emit key material, seeds, or keypair paths.

#[event]
pub struct SpendVaultInitialized {
    pub spend_vault: Pubkey,
    pub grok_account: Pubkey,
    pub root: Pubkey,
}

#[event]
pub struct SpendVaultFunded {
    pub spend_vault: Pubkey,
    pub grok_account: Pubkey,
    pub root: Pubkey,
    pub lamports: u64,
}

#[event]
pub struct SpendVaultWithdrawn {
    pub spend_vault: Pubkey,
    pub grok_account: Pubkey,
    pub root: Pubkey,
    pub lamports: u64,
}

#[event]
pub struct PaymasterInitialized {
    pub paymaster: Pubkey,
    pub grok_account: Pubkey,
    pub root: Pubkey,
    pub relayer: Pubkey,
}

#[event]
pub struct PaymasterFunded {
    pub paymaster: Pubkey,
    pub grok_account: Pubkey,
    pub root: Pubkey,
    pub lamports: u64,
}

#[event]
pub struct PaymasterWithdrawn {
    pub paymaster: Pubkey,
    pub grok_account: Pubkey,
    pub root: Pubkey,
    pub lamports: u64,
}

#[event]
pub struct RelayerSet {
    pub paymaster: Pubkey,
    pub grok_account: Pubkey,
    pub root: Pubkey,
    pub relayer: Pubkey,
}

#[event]
pub struct PaymasterPaused {
    pub paymaster: Pubkey,
    pub grok_account: Pubkey,
    pub root: Pubkey,
}

#[event]
pub struct PaymasterUnpaused {
    pub paymaster: Pubkey,
    pub grok_account: Pubkey,
    pub root: Pubkey,
}

#[event]
pub struct Paid {
    pub vault: Pubkey,
    pub recipient: Pubkey,
    pub amount_lamports: u64,
    pub agent: Pubkey,
    pub grant: Pubkey,
    pub generation: u32,
}

#[event]
pub struct Sponsored {
    pub paymaster: Pubkey,
    pub relayer: Pubkey,
    pub sponsor_lamports: u64,
    pub grant: Pubkey,
    pub generation: u32,
}

/// v1 swap is a grant-gated SOL send with a min_out check. Not a DEX.
#[event]
pub struct Swapped {
    pub vault: Pubkey,
    pub destination: Pubkey,
    pub amount_in_lamports: u64,
    pub min_out_lamports: u64,
    pub agent: Pubkey,
    pub grant: Pubkey,
    pub generation: u32,
}

/// v1 deploy is a grant-gated request. Not a BPF deploy. No ELF uploaded.
#[event]
pub struct DeployRequested {
    pub program_id: Pubkey,
    pub agent: Pubkey,
    pub grant: Pubkey,
    pub generation: u32,
}

/// v1 call is a grant-gated router. Inner CPI uses remaining_accounts + empty data.
#[event]
pub struct Called {
    pub target_program: Pubkey,
    pub recipient: Pubkey,
    pub amount_lamports: u64,
    pub remaining_len: u32,
    pub agent: Pubkey,
    pub grant: Pubkey,
    pub generation: u32,
}

/// Root created the 0-byte system-owned pump-trader PDA.
#[event]
pub struct PumpTraderInitialized {
    pub pump_trader: Pubkey,
    pub grok_account: Pubkey,
    pub root: Pubkey,
}

/// Root moved SOL SpendVault → pump-trader (no grant).
#[event]
pub struct PumpTraderFunded {
    pub pump_trader: Pubkey,
    pub spend_vault: Pubkey,
    pub grok_account: Pubkey,
    pub root: Pubkey,
    pub lamports: u64,
}

/// Grant-gated pump.fun buy_v2. Trader is pump `user`. Vault is never user.
#[event]
pub struct PumpBought {
    pub vault: Pubkey,
    pub trader: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub max_sol_cost: u64,
    pub agent: Pubkey,
    pub grant: Pubkey,
    pub generation: u32,
}

/// Grant-gated pump.fun sell_v2. Trader is pump `user`. Vault is never user.
#[event]
pub struct PumpSold {
    pub vault: Pubkey,
    pub trader: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub min_sol_output: u64,
    pub agent: Pubkey,
    pub grant: Pubkey,
    pub generation: u32,
}

/// Grant-gated pump.fun create_v2. Trader is pump `user`. Mint is a client signer.
/// Creator recorded on-chain is `grok_account.root`.
#[event]
pub struct PumpCreated {
    pub vault: Pubkey,
    pub trader: Pubkey,
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub max_sol_cost: u64,
    pub is_mayhem_mode: bool,
    pub is_cashback_enabled: bool,
    pub agent: Pubkey,
    pub grant: Pubkey,
    pub generation: u32,
}


/// Grant-gated PumpSwap AMM buy_exact_quote_in. Trader is user. Vault is never user.
#[event]
pub struct PumpAmmBought {
    pub vault: Pubkey,
    pub trader: Pubkey,
    pub mint: Pubkey,
    pub pool: Pubkey,
    pub spendable_quote_in: u64,
    pub min_base_amount_out: u64,
    pub max_sol_cost: u64,
    pub agent: Pubkey,
    pub grant: Pubkey,
    pub generation: u32,
}

/// Grant-gated PumpSwap AMM sell. Trader is user. Vault is never user.
/// Quote lands as WSOL on trader ATA. Not unwrapped or swept to vault in
/// the same ix (named vault + remaining trader = UnbalancedInstruction).
#[event]
pub struct PumpAmmSold {
    pub vault: Pubkey,
    pub trader: Pubkey,
    pub mint: Pubkey,
    pub pool: Pubkey,
    pub base_amount_in: u64,
    pub min_quote_amount_out: u64,
    pub agent: Pubkey,
    pub grant: Pubkey,
    pub generation: u32,
}
