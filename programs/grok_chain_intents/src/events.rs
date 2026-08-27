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
