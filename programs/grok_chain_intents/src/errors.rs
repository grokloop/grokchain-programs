use anchor_lang::prelude::*;

/// SPEC.md §8. Discriminants 0..=14 are stable. Next free code after this file: 19.
#[error_code]
pub enum IntentsError {
    #[msg("root signer does not match vault/paymaster root")]
    UnauthorizedRoot = 0,
    #[msg("agent signer does not match grant.agent")]
    AgentMismatch = 1,
    #[msg("pay amount_lamports must be greater than zero")]
    ZeroPayAmount = 2,
    #[msg("amount must be greater than zero")]
    ZeroAmount = 3,
    #[msg("spend vault has insufficient lamports (rent-exempt minimum required)")]
    InsufficientSpendVault = 4,
    #[msg("paymaster has insufficient lamports (rent-exempt minimum required)")]
    InsufficientPaymaster = 5,
    #[msg("paymaster is paused")]
    PaymasterPaused = 6,
    #[msg("fee payer is not the configured relayer")]
    RelayerMismatch = 7,
    #[msg("grant is not sponsor_eligible")]
    NotSponsorEligible = 8,
    #[msg("sponsor path requires paymaster and relayer accounts")]
    SponsorAccountsRequired = 9,
    #[msg("sponsor_lamports exceeds MAX_SPONSOR_LAMPORTS")]
    SponsorCapExceeded = 10,
    /// Reserved. swap / deploy / call no longer return this.
    #[msg("intent is not implemented")]
    IntentStub = 11,
    #[msg("vault/paymaster grok_account mismatch")]
    GrokAccountMismatch = 12,
    #[msg("lamport arithmetic overflow")]
    LamportOverflow = 13,
    #[msg("CORE program id mismatch")]
    InvalidCoreProgram = 14,
    #[msg("swap amount_in_lamports is below min_out_lamports")]
    MinOutNotMet = 15,
    #[msg("call_target account does not match args.target_program")]
    CallTargetMismatch = 16,
    #[msg("call target program is not executable")]
    TargetNotExecutable = 17,
    #[msg("remaining_accounts must not include spend_vault or paymaster")]
    ProtectedAccountInRemaining = 18,
}
