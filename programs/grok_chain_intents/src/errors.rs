use anchor_lang::prelude::*;

/// SPEC.md §8. Discriminants 0..=18 are stable. Next free code after this file: 49.
/// 21 PumpUserMustBeVault is unused (vault is never pump user).
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
    /// MAINNET-PREP: swap / deploy / call return this. No debit. No invoke.
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
    #[msg("inner program is not the official pump.fun program")]
    PumpProgramMismatch = 19,
    #[msg("pump remaining_accounts count does not match official buy_v2/sell_v2/create_v2")]
    PumpAccountCountMismatch = 20,
    #[msg("pump user must be the SpendVault PDA")]
    PumpUserMustBeVault = 21,
    #[msg("agent cannot be the fee payer or SOL source")]
    AgentCannotFeePay = 22,
    #[msg("pump instruction data must be official disc + args; empty data is forbidden")]
    PumpEmptyDataForbidden = 23,
    #[msg("creating a vault ATA requires the relayer as fee payer")]
    PumpAtaCreateRequiresFeePayer = 24,
    #[msg("token program must be Token-2022 or legacy SPL Token")]
    InvalidTokenProgram = 25,
    #[msg("pump PDA / ATA derivation does not match official seeds")]
    PumpPdaMismatch = 26,
    #[msg("pump user must be the system-owned pump-trader PDA")]
    PumpUserMustBeTrader = 27,
    #[msg("pump-trader PDA must be owned by the System Program")]
    PumpTraderNotSystemOwned = 28,
    #[msg("pump-trader PDA already exists")]
    PumpTraderAlreadyExists = 29,
    #[msg("pump-trader PDA is not initialized")]
    PumpTraderNotInitialized = 30,
    #[msg("create_v2 mint must be a client-signed Token-2022 keypair")]
    PumpMintMustBeSigner = 31,
    #[msg("create_v2 name exceeds 32 characters")]
    PumpCreateNameTooLong = 32,
    #[msg("create_v2 symbol exceeds 13 characters")]
    PumpCreateSymbolTooLong = 33,
    #[msg("create_v2 uri exceeds 200 characters")]
    PumpCreateUriTooLong = 34,
    #[msg("inner program is not the official PumpSwap AMM program")]
    PumpAmmProgramMismatch = 35,
    #[msg("pump AMM remaining_accounts count must be 26/27 (buy) or 24/26 (sell)")]
    PumpAmmAccountCountMismatch = 36,
    #[msg("pool account is not a valid PumpSwap Pool")]
    PumpAmmPoolInvalid = 37,
    #[msg("PumpSwap quote mint must be wrapped SOL")]
    PumpAmmQuoteMustBeWsol = 38,
    #[msg("protocol or breaking fee recipient is not on the official list")]
    PumpAmmFeeRecipientInvalid = 39,
    #[msg("pump-trader needs fund_pump_trader first; no in-ix vault debit")]
    PumpTraderUnderfunded = 40,
    #[msg("pump AMM sell check_grant amount must be zero")]
    PumpAmmSellCheckGrantAmountMustBeZero = 41,
    #[msg("pump AMM sell remaining_accounts count must be 24")]
    PumpAmmSellAccountCountMismatch = 42,
    #[msg("withdraw remaining_accounts must be even [from_ata, to_ata, ...] pairs")]
    WithdrawRemainingAccountsOdd = 43,
    #[msg("withdraw token account is not a valid Token or Token-2022 account")]
    WithdrawTokenAccountInvalid = 44,
    #[msg("withdraw from ATA owner is not the pump-trader")]
    WithdrawTokenOwnerNotTrader = 45,
    #[msg("withdraw from/to ATA mint mismatch")]
    WithdrawTokenMintMismatch = 46,
    #[msg("withdraw dest ATA owner is not root")]
    WithdrawTokenDestOwnerNotRoot = 47,
    #[msg("pump-trader SOL withdraw would drop below rent-exempt minimum")]
    InsufficientPumpTrader = 48,
}
