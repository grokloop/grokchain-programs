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
