//! Grok Chain intents + paymaster program (v1).
//! Implements SPEC.md. Sits on Solana L1. Not a VM, sequencer, or coin.
//!
//! `declare_id!` is the pubkey of the MAINNET-PREP keypair (not the live DEVNET id).
//! MAINNET overlay: pay + grant-gated swap/deploy/call + token. Pump trade ixs cut for size.
//! declare_id stays 3HCErAF. CORE CPI target is 44fxwzu. Not a DEX / L1 / compiler.

use anchor_lang::prelude::*;

pub mod constants;
pub mod core_cpi;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod policy;
pub mod state;

pub use constants::*;
pub use errors::*;
pub use events::*;
pub use instructions::*;
pub use state::*;

declare_id!("3HCErAFs93FMk2J25Qq1xRRMp6B4FyGvif8ZV8hYxQKw");

#[program]
pub mod grok_chain_intents {
    use super::*;

    pub fn init_spend_vault(ctx: Context<InitSpendVault>) -> Result<()> {
        instructions::spend_vault::init(ctx)
    }

    pub fn fund_spend_vault(ctx: Context<FundSpendVault>, lamports: u64) -> Result<()> {
        instructions::spend_vault::fund(ctx, lamports)
    }

    pub fn withdraw_spend_vault(ctx: Context<WithdrawSpendVault>, lamports: u64) -> Result<()> {
        instructions::spend_vault::withdraw(ctx, lamports)
    }

    pub fn init_paymaster(ctx: Context<InitPaymaster>, relayer: Pubkey) -> Result<()> {
        instructions::paymaster::init(ctx, relayer)
    }

    pub fn fund_paymaster(ctx: Context<FundPaymaster>, lamports: u64) -> Result<()> {
        instructions::paymaster::fund(ctx, lamports)
    }

    pub fn withdraw_paymaster(ctx: Context<WithdrawPaymaster>, lamports: u64) -> Result<()> {
        instructions::paymaster::withdraw(ctx, lamports)
    }

    pub fn set_relayer(ctx: Context<SetRelayer>, relayer: Pubkey) -> Result<()> {
        instructions::paymaster::set(ctx, relayer)
    }

    pub fn pause_paymaster(ctx: Context<PausePaymaster>) -> Result<()> {
        instructions::paymaster::pause(ctx)
    }

    pub fn unpause_paymaster(ctx: Context<UnpausePaymaster>) -> Result<()> {
        instructions::paymaster::unpause(ctx)
    }

    pub fn pay(ctx: Context<Pay>, args: PayArgs) -> Result<()> {
        instructions::pay::handler(ctx, args)
    }

    pub fn swap(ctx: Context<Swap>, args: SwapArgs) -> Result<()> {
        instructions::swap::handler(ctx, args)
    }

    pub fn deploy(ctx: Context<Deploy>, args: DeployArgs) -> Result<()> {
        instructions::deploy::handler(ctx, args)
    }

    pub fn call(ctx: Context<Call>, args: CallArgs) -> Result<()> {
        instructions::call::handler(ctx, args)
    }

    /// pump.fun create_v2. Launch only — buy is Jupiter (token_buy).
    /// remaining[0] mint must be a client-signed Token-2022 keypair.
    /// Creator recorded on chain is grok_account.root, not the agent.
    pub fn pump_create(ctx: Context<PumpTrade>, args: PumpCreateArgs) -> Result<()> {
        instructions::pump::create_handler(ctx, args)
    }

    pub fn init_pump_trader(ctx: Context<InitPumpTrader>) -> Result<()> {
        instructions::pump_trader::init(ctx)
    }

    pub fn fund_pump_trader(ctx: Context<FundPumpTrader>, lamports: u64) -> Result<()> {
        instructions::pump_trader::fund(ctx, lamports)
    }

    pub fn withdraw_pump_trader<'info>(
        ctx: Context<'_, '_, '_, 'info, WithdrawPumpTrader<'info>>,
        lamports: u64,
    ) -> Result<()> {
        instructions::pump_trader::withdraw(ctx, lamports)
    }

    pub fn token_buy(ctx: Context<TokenTrade>, args: TokenBuyArgs) -> Result<()> {
        instructions::token::token_buy_handler(ctx, args)
    }

    pub fn token_sell(ctx: Context<TokenTrade>, args: TokenSellArgs) -> Result<()> {
        instructions::token::token_sell_handler(ctx, args)
    }

    // ---- payments: paying a real payee, not trading ----

    /// Root: create the payee allowlist, pinned to one mint.
    pub fn init_merchant_registry(
        ctx: Context<InitMerchantRegistry>,
        mint: Pubkey,
    ) -> Result<()> {
        instructions::merchants::init(ctx, mint)
    }

    /// Root: approve a payee.
    pub fn add_merchant(ctx: Context<UpdateMerchantRegistry>, merchant: Pubkey) -> Result<()> {
        instructions::merchants::add(ctx, merchant)
    }

    /// Root: revoke a payee. Immediate, and cancels every subscription to them.
    pub fn remove_merchant(ctx: Context<UpdateMerchantRegistry>, merchant: Pubkey) -> Result<()> {
        instructions::merchants::remove(ctx, merchant)
    }

    /// Agent: pay an approved merchant. Grant-gated, metered in raw token units.
    pub fn pay_token(ctx: Context<PayToken>, args: PayTokenArgs) -> Result<()> {
        instructions::pay_token::handler(ctx, args)
    }

    // ---- subscriptions ----

    /// Root: start recurring billing to an already-approved merchant.
    pub fn create_subscription(
        ctx: Context<CreateSubscription>,
        args: SubscriptionArgs,
    ) -> Result<()> {
        instructions::subscription::create(ctx, args)
    }

    /// Root: stop it. Immediate, and the merchant cannot refuse or delay.
    pub fn cancel_subscription(ctx: Context<CancelSubscription>) -> Result<()> {
        instructions::subscription::cancel(ctx)
    }

    /// Agent: settle one period. Idempotent — `last_paid_period` advances in the
    /// same transaction that moves the money, so a retry cannot pay twice.
    pub fn pay_subscription(
        ctx: Context<PaySubscription>,
        args: PaySubscriptionArgs,
    ) -> Result<()> {
        instructions::subscription::pay(ctx, args)
    }
}

pub fn spend_vault_pda(program_id: &Pubkey, grok_account: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_SPEND_VAULT, grok_account.as_ref()], program_id)
}

pub fn paymaster_pda(program_id: &Pubkey, grok_account: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_PAYMASTER, grok_account.as_ref()], program_id)
}

pub fn pump_trader_pda(program_id: &Pubkey, grok_account: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_PUMP_TRADER, grok_account.as_ref()], program_id)
}

#[cfg(test)]
mod capability_audit;

#[cfg(test)]
mod spec_lock {
    use super::*;
    use anchor_lang::solana_program::hash::hash;

    fn disc(prefix: &str, name: &str) -> [u8; 8] {
        let d = hash(format!("{prefix}:{name}").as_bytes());
        let mut out = [0u8; 8];
        out.copy_from_slice(&d.to_bytes()[..8]);
        out
    }

    #[test]
    fn spaces_match_spec() {
        assert_eq!(SpendVault::SPACE, 73);
        assert_eq!(Paymaster::SPACE, 106);
        // 8 disc + grok + root + mint + bump + vec len + 32 payees
        assert_eq!(MerchantRegistry::SPACE, 8 + 32 + 32 + 32 + 1 + 4 + 32 * 32);
        // 8 disc + 4 pubkeys + amount + period + start + last_paid + payments + active + bump
        assert_eq!(Subscription::SPACE, 8 + 32 * 4 + 8 * 4 + 4 + 1 + 1);
    }

    /// The payment surface exists to do what trading intents cannot: move value
    /// to somebody else, bounded by a list only the human can edit.
    #[test]
    fn payment_seeds_and_bounds() {
        assert_eq!(SEED_MERCHANTS, b"merchants");
        assert_eq!(SEED_SUBSCRIPTION, b"subscription");
        assert_eq!(MAX_MERCHANTS, 32);
        // A day is the floor. Anything faster is a drain vector wearing a
        // subscription's clothes.
        assert_eq!(instructions::subscription::MIN_PERIOD_SECONDS, 86_400);
        // Never-paid must sit below period 0, or the first charge is skipped.
        assert!(instructions::subscription::PERIOD_NONE < 0);
    }

    /// Periods are the whole idempotency story: a boundary that moves early or
    /// late is a double charge or a missed one.
    #[test]
    fn subscription_periods_are_exact() {
        use instructions::subscription::current_period;
        let start = 1_700_000_000i64;
        let day = 86_400i64;
        assert_eq!(current_period(start, start, day).unwrap(), 0);
        assert_eq!(current_period(start + day - 1, start, day).unwrap(), 0);
        assert_eq!(current_period(start + day, start, day).unwrap(), 1);
        assert!(current_period(start - 1, start, day).is_err());
    }

    /// min_out was silently ignored before; these are the bounds that replaced it.
    #[test]
    fn swap_outcome_is_bounded_by_balances() {
        assert!(policy::enforce_swap_outcome(1_000, 500, 1_000, 500).is_ok());
        assert!(policy::enforce_swap_outcome(1_001, 500, 1_000, 500).is_err());
        assert!(policy::enforce_swap_outcome(1_000, 499, 1_000, 500).is_err());
    }

    #[test]
    fn seeds_match_spec() {
        assert_eq!(SEED_SPEND_VAULT, b"spend-vault");
        assert_eq!(SEED_PAYMASTER, b"paymaster");
        assert_eq!(SEED_PUMP_TRADER, b"pump-trader");
        assert_eq!(PUMP_TRADER_SPACE, 0);
        assert_eq!(
            SEED_SPEND_VAULT,
            &[115, 112, 101, 110, 100, 45, 118, 97, 117, 108, 116]
        );
        assert_eq!(
            SEED_PAYMASTER,
            &[112, 97, 121, 109, 97, 115, 116, 101, 114]
        );
        assert_eq!(
            SEED_PUMP_TRADER,
            &[112, 117, 109, 112, 45, 116, 114, 97, 100, 101, 114]
        );
        assert_eq!(MAX_SPONSOR_LAMPORTS, 10_000_000);
        // CORE seeds we path-depend on — do not re-derive incorrectly.
        assert_eq!(grok_chain_core::SEED_GROK_ACCOUNT, b"grok-account");
        assert_eq!(grok_chain_core::SEED_GRANT, b"grant");
    }

    #[test]
    fn error_discriminants_0_to_54() {
        assert_eq!(IntentsError::UnauthorizedRoot as u32, 0);
        assert_eq!(IntentsError::AgentMismatch as u32, 1);
        assert_eq!(IntentsError::ZeroPayAmount as u32, 2);
        assert_eq!(IntentsError::ZeroAmount as u32, 3);
        assert_eq!(IntentsError::InsufficientSpendVault as u32, 4);
        assert_eq!(IntentsError::InsufficientPaymaster as u32, 5);
        assert_eq!(IntentsError::PaymasterPaused as u32, 6);
        assert_eq!(IntentsError::RelayerMismatch as u32, 7);
        assert_eq!(IntentsError::NotSponsorEligible as u32, 8);
        assert_eq!(IntentsError::SponsorAccountsRequired as u32, 9);
        assert_eq!(IntentsError::SponsorCapExceeded as u32, 10);
        assert_eq!(IntentsError::IntentStub as u32, 11);
        assert_eq!(IntentsError::GrokAccountMismatch as u32, 12);
        assert_eq!(IntentsError::LamportOverflow as u32, 13);
        assert_eq!(IntentsError::InvalidCoreProgram as u32, 14);
        assert_eq!(IntentsError::MinOutNotMet as u32, 15);
        assert_eq!(IntentsError::CallTargetMismatch as u32, 16);
        assert_eq!(IntentsError::TargetNotExecutable as u32, 17);
        assert_eq!(IntentsError::ProtectedAccountInRemaining as u32, 18);
        assert_eq!(IntentsError::PumpProgramMismatch as u32, 19);
        assert_eq!(IntentsError::PumpAccountCountMismatch as u32, 20);
        assert_eq!(IntentsError::PumpUserMustBeVault as u32, 21);
        assert_eq!(IntentsError::AgentCannotFeePay as u32, 22);
        assert_eq!(IntentsError::PumpEmptyDataForbidden as u32, 23);
        assert_eq!(IntentsError::PumpAtaCreateRequiresFeePayer as u32, 24);
        assert_eq!(IntentsError::InvalidTokenProgram as u32, 25);
        assert_eq!(IntentsError::PumpPdaMismatch as u32, 26);
        assert_eq!(IntentsError::PumpUserMustBeVault as u32, 21);
        assert_eq!(IntentsError::PumpUserMustBeTrader as u32, 27);
        assert_eq!(IntentsError::PumpTraderNotSystemOwned as u32, 28);
        assert_eq!(IntentsError::PumpTraderAlreadyExists as u32, 29);
        assert_eq!(IntentsError::PumpTraderNotInitialized as u32, 30);
        assert_eq!(IntentsError::PumpMintMustBeSigner as u32, 31);
        assert_eq!(IntentsError::PumpCreateNameTooLong as u32, 32);
        assert_eq!(IntentsError::PumpCreateSymbolTooLong as u32, 33);
        assert_eq!(IntentsError::PumpCreateUriTooLong as u32, 34);
        assert_eq!(IntentsError::PumpAmmProgramMismatch as u32, 35);
        assert_eq!(IntentsError::PumpAmmAccountCountMismatch as u32, 36);
        assert_eq!(IntentsError::PumpAmmPoolInvalid as u32, 37);
        assert_eq!(IntentsError::PumpAmmQuoteMustBeWsol as u32, 38);
        assert_eq!(IntentsError::PumpAmmFeeRecipientInvalid as u32, 39);
        assert_eq!(IntentsError::PumpTraderUnderfunded as u32, 40);
        assert_eq!(IntentsError::PumpAmmSellCheckGrantAmountMustBeZero as u32, 41);
        assert_eq!(IntentsError::PumpAmmSellAccountCountMismatch as u32, 42);
        assert_eq!(IntentsError::WithdrawRemainingAccountsOdd as u32, 43);
        assert_eq!(IntentsError::WithdrawTokenAccountInvalid as u32, 44);
        assert_eq!(IntentsError::WithdrawTokenOwnerNotTrader as u32, 45);
        assert_eq!(IntentsError::WithdrawTokenMintMismatch as u32, 46);
        assert_eq!(IntentsError::WithdrawTokenDestOwnerNotRoot as u32, 47);
        assert_eq!(IntentsError::InsufficientPumpTrader as u32, 48);
        assert_eq!(IntentsError::JupiterProgramMismatch as u32, 49);
        assert_eq!(IntentsError::JupiterEmptyDataForbidden as u32, 50);
        assert_eq!(IntentsError::JupiterInAmountMismatch as u32, 51);
        assert_eq!(IntentsError::JupiterSourceOwnerNotTrader as u32, 52);
        assert_eq!(IntentsError::JupiterDestOwnerNotTrader as u32, 53);
        assert_eq!(IntentsError::TokenWrapMintMustBeWsol as u32, 54);
    }

    #[test]
    fn intent_stub_reserved_and_unused_by_new_handlers() {
        // Code 11 stays reserved. swap/deploy/call must not return it.
        assert_eq!(IntentsError::IntentStub as u32, 11);
        assert_ne!(IntentsError::MinOutNotMet as u32, 11);
        assert_ne!(IntentsError::CallTargetMismatch as u32, 11);
        assert_ne!(IntentsError::TargetNotExecutable as u32, 11);
        assert_ne!(IntentsError::ProtectedAccountInRemaining as u32, 11);
        assert_ne!(IntentsError::PumpProgramMismatch as u32, 11);
        assert_ne!(IntentsError::AgentCannotFeePay as u32, 11);
    }

    #[test]
    fn account_instruction_event_discriminators() {
        assert_eq!(disc("account", "SpendVault"), [75, 166, 253, 76, 235, 57, 134, 93]);
        assert_eq!(disc("account", "Paymaster"), [79, 131, 123, 96, 75, 37, 131, 106]);
        assert_eq!(disc("global", "init_spend_vault"), [241, 173, 7, 179, 120, 124, 213, 61]);
        assert_eq!(disc("global", "fund_spend_vault"), [105, 178, 22, 113, 64, 88, 201, 233]);
        assert_eq!(disc("global", "withdraw_spend_vault"), [41, 235, 152, 150, 129, 122, 224, 37]);
        assert_eq!(disc("global", "init_paymaster"), [23, 62, 252, 40, 178, 70, 114, 54]);
        assert_eq!(disc("global", "fund_paymaster"), [84, 67, 136, 170, 168, 163, 220, 103]);
        assert_eq!(disc("global", "withdraw_paymaster"), [54, 60, 197, 226, 34, 179, 149, 189]);
        assert_eq!(disc("global", "set_relayer"), [23, 243, 33, 88, 110, 84, 196, 37]);
        assert_eq!(disc("global", "pause_paymaster"), [97, 26, 152, 173, 59, 148, 244, 77]);
        assert_eq!(disc("global", "unpause_paymaster"), [143, 248, 211, 216, 98, 113, 49, 251]);
        assert_eq!(disc("global", "pay"), [119, 18, 216, 65, 192, 117, 122, 220]);
        // Locked names. Do not rename these ixs.
        assert_eq!(disc("global", "swap"), [248, 198, 158, 145, 225, 117, 135, 200]);
        assert_eq!(disc("global", "deploy"), [67, 36, 143, 118, 36, 164, 92, 217]);
        assert_eq!(disc("global", "call"), [181, 94, 56, 161, 194, 221, 200, 3]);
        assert_eq!(disc("event", "Paid"), [240, 193, 17, 238, 238, 210, 129, 235]);
        assert_eq!(disc("event", "Sponsored"), [13, 53, 40, 13, 165, 80, 85, 131]);
        assert_eq!(disc("event", "Swapped"), [217, 52, 52, 83, 147, 135, 96, 109]);
        assert_eq!(disc("event", "DeployRequested"), [236, 93, 153, 180, 211, 112, 67, 252]);
        assert_eq!(disc("event", "Called"), [30, 97, 254, 149, 60, 38, 255, 5]);
        assert_eq!(disc("global", "pump_buy"), [82, 225, 119, 231, 78, 29, 45, 70]);
        assert_eq!(disc("global", "pump_sell"), [93, 88, 60, 34, 91, 18, 86, 197]);
        assert_eq!(disc("event", "PumpBought"), [209, 183, 238, 75, 181, 1, 117, 110]);
        assert_eq!(disc("event", "PumpSold"), [17, 201, 39, 152, 27, 110, 49, 106]);
        assert_eq!(disc("global", "init_pump_trader"), [92, 98, 75, 2, 93, 219, 250, 5]);
        assert_eq!(disc("global", "fund_pump_trader"), [63, 189, 216, 54, 81, 101, 241, 97]);
        assert_eq!(disc("event", "PumpTraderInitialized"), [197, 80, 95, 239, 242, 153, 144, 179]);
        assert_eq!(disc("event", "PumpTraderFunded"), [60, 253, 86, 159, 109, 54, 46, 170]);
        assert_eq!(disc("global", "pump_create"), [24, 176, 142, 141, 243, 152, 56, 128]);
        assert_eq!(disc("event", "PumpCreated"), [126, 79, 125, 229, 148, 39, 13, 70]);
        assert_eq!(disc("global", "pump_amm_buy"), [129, 59, 179, 195, 110, 135, 61, 2]);
        assert_eq!(disc("event", "PumpAmmBought"), [234, 79, 225, 112, 20, 215, 78, 43]);
        assert_eq!(disc("global", "pump_amm_sell"), [238, 234, 142, 38, 107, 206, 76, 195]);
        assert_eq!(disc("event", "PumpAmmSold"), [66, 145, 209, 9, 84, 220, 173, 113]);
        assert_eq!(disc("global", "withdraw_pump_trader"), [188, 237, 135, 114, 143, 224, 45, 178]);
        assert_eq!(disc("event", "PumpTraderWithdrawn"), [55, 235, 116, 220, 179, 244, 66, 235]);
        assert_eq!(disc("global", "token_buy"), [116, 167, 118, 40, 127, 96, 55, 234]);
        assert_eq!(disc("global", "token_sell"), [154, 76, 173, 221, 122, 208, 158, 103]);
        assert_eq!(disc("event", "TokenBought"), [197, 182, 3, 228, 82, 236, 7, 143]);
        assert_eq!(disc("event", "TokenSold"), [88, 61, 1, 247, 185, 6, 252, 86]);
    }

    #[test]
    fn check_grant_cpi_surface_matches_core() {
        // Hardcoded CORE disc. Do not recompute from a different string.
        assert_eq!(
            CHECK_GRANT_DISCRIMINATOR,
            [223, 172, 131, 140, 15, 133, 209, 250]
        );
        assert_eq!(disc("global", "check_grant"), CHECK_GRANT_DISCRIMINATOR);
        assert_eq!(CHECK_GRANT_META_WRITABLE, [false, true, false, false]);
        assert_eq!(CHECK_GRANT_META_SIGNER, [false, false, true, false]);
        // CORE crate declare_id (deploy keypair). CPI program_id must be this.
        assert_eq!(
            grok_chain_core::ID.to_string(),
            "44fxwzuEyNxZtgDr87mTtMYYJ1LJm6cB5aZNLyBsPjNd"
        );
        // Data size: 8-byte disc + Borsh u64.
        assert_eq!(CHECK_GRANT_DISCRIMINATOR.len() + 8, 16);
    }

    #[test]
    fn pay_policy_without_runtime() {
        // pay must move SOL
        let zero = PayArgs {
            amount_lamports: 0,
            sponsor_lamports: 0,
        };
        assert!(policy::require_pay_amount(zero.amount_lamports).is_err());

        let over = PayArgs {
            amount_lamports: 1,
            sponsor_lamports: MAX_SPONSOR_LAMPORTS + 1,
        };
        assert!(over.sponsor_lamports > MAX_SPONSOR_LAMPORTS);
        assert!(policy::require_sponsor_cap(over.sponsor_lamports).is_err());

        let ok = PayArgs {
            amount_lamports: 1,
            sponsor_lamports: MAX_SPONSOR_LAMPORTS,
        };
        assert!(ok.amount_lamports > 0);
        assert!(ok.sponsor_lamports <= MAX_SPONSOR_LAMPORTS);
        assert!(policy::require_pay_amount(ok.amount_lamports).is_ok());
        assert!(policy::require_sponsor_cap(ok.sponsor_lamports).is_ok());

        // sponsor 0 skips reimbursement even if a huge vault exists
        let skip = PayArgs {
            amount_lamports: 500,
            sponsor_lamports: 0,
        };
        assert_eq!(skip.sponsor_lamports, 0);
    }

    #[test]
    fn swap_policy_without_runtime() {
        let zero = SwapArgs {
            amount_in_lamports: 0,
            min_out_lamports: 0,
            sponsor_lamports: 0,
        };
        assert!(policy::require_swap_amounts(zero.amount_in_lamports, zero.min_out_lamports).is_err());

        let min_too_high = SwapArgs {
            amount_in_lamports: 100,
            min_out_lamports: 101,
            sponsor_lamports: 0,
        };
        assert!(policy::require_swap_amounts(
            min_too_high.amount_in_lamports,
            min_too_high.min_out_lamports
        )
        .is_err());

        let ok = SwapArgs {
            amount_in_lamports: 100,
            min_out_lamports: 100,
            sponsor_lamports: 0,
        };
        assert!(policy::require_swap_amounts(ok.amount_in_lamports, ok.min_out_lamports).is_ok());

        let over = SwapArgs {
            amount_in_lamports: 1,
            min_out_lamports: 1,
            sponsor_lamports: MAX_SPONSOR_LAMPORTS + 1,
        };
        assert!(policy::require_sponsor_cap(over.sponsor_lamports).is_err());
    }

    #[test]
    fn deploy_and_call_allow_amount_zero() {
        assert_eq!(policy::DEPLOY_CHECK_GRANT_AMOUNT, 0);
        // call amount 0 is a policy ping: no vault debit. grant check still required.
        let ping = CallArgs {
            amount_lamports: 0,
            sponsor_lamports: 0,
            target_program: crate::ID,
        };
        assert_eq!(ping.amount_lamports, 0);
        assert!(policy::require_sponsor_cap(ping.sponsor_lamports).is_ok());

        let deploy = DeployArgs {
            sponsor_lamports: 0,
            program_id: crate::ID,
        };
        assert_eq!(deploy.sponsor_lamports, 0);
        assert!(policy::require_sponsor_accounts(deploy.sponsor_lamports, false).is_ok());
        assert!(policy::require_sponsor_accounts(1, false).is_err());
    }

    #[test]
    fn amount_zero_forbidden_for_swap_and_pay_allowed_for_call_deploy() {
        assert!(policy::require_pay_amount(0).is_err());
        assert!(policy::require_swap_amounts(0, 0).is_err());
        // call/deploy path: 0 is the check_grant amount
        assert_eq!(policy::DEPLOY_CHECK_GRANT_AMOUNT, 0);
    }

    #[test]
    fn pump_amm_buy_requires_prefund() {
        // rent0 for 0-byte trader is 0 on-chain only at runtime; here just the math.
        assert!(policy::require_pump_trader_prefunded(0, 100_000_000, 890_880).is_err());
        assert!(policy::require_pump_trader_prefunded(100_000_000, 100_000_000, 890_880).is_err());
        assert!(policy::require_pump_trader_prefunded(100_890_880, 100_000_000, 890_880).is_ok());
    }

    #[test]
    fn pump_buy_requires_prefund() {
        // Curve buy: max_sol_cost + 0-byte rent already on trader. Same math as AMM.
        assert!(policy::require_pump_trader_prefunded(0, 100_000_000, 890_880).is_err());
        assert!(policy::require_pump_trader_prefunded(100_000_000, 100_000_000, 890_880).is_err());
        assert!(policy::require_pump_trader_prefunded(100_890_880, 100_000_000, 890_880).is_ok());
        assert_eq!(IntentsError::PumpTraderUnderfunded as u32, 40);
    }

    #[test]
    fn pump_amm_sell_check_grant_zero_and_amounts() {
        assert_eq!(policy::PUMP_SELL_CHECK_GRANT_AMOUNT, 0);
        assert_eq!(policy::PUMP_AMM_SELL_CHECK_GRANT_AMOUNT, 0);
        assert!(policy::require_pump_amm_sell_check_grant_amount(0).is_ok());
        assert!(policy::require_pump_amm_sell_check_grant_amount(1).is_err());
        assert!(policy::require_pump_amm_sell_amounts(0, 1).is_err());
        assert!(policy::require_pump_amm_sell_amounts(1, 0).is_ok());
        assert!(policy::require_pump_amm_sell_amounts(1, 1).is_ok());
        assert!(policy::require_pump_amm_sell_account_count(24).is_ok());
        assert!(policy::require_pump_amm_sell_account_count(26).is_err());
        assert!(policy::require_pump_amm_sell_account_count(27).is_err());
        let data = encode_pump_amm_sell(1_000, 1);
        assert_eq!(data.len(), 24);
        assert_eq!(&data[..8], &PUMP_AMM_SELL_DISC);
        assert!(policy::require_nonempty_pump_amm_sell_data(&data).is_ok());
        let buy = encode_pump_amm_buy_exact_quote_in(1, 1, true);
        assert!(policy::require_nonempty_pump_amm_sell_data(&buy).is_err());
    }

    #[test]
    fn token_buy_sell_policy_without_runtime() {
        assert!(policy::require_token_amounts(0).is_err());
        assert!(policy::require_token_amounts(1).is_ok());
        // min_out is held after the swap, not before it
        assert!(policy::enforce_swap_outcome(1, 1, 1, 1).is_ok());
        assert!(policy::enforce_swap_outcome(2, 1, 1, 1).is_err());
        assert!(policy::enforce_swap_outcome(1, 0, 1, 1).is_err());
        let mut data = vec![9u8; 8];
        data.extend_from_slice(&42u64.to_le_bytes());
        assert!(policy::require_nonempty_jupiter_data(&data).is_ok());
        assert!(policy::require_jupiter_in_amount(&data, 42).is_ok());
        assert!(policy::require_jupiter_in_amount(&data, 1).is_err());
        assert!(policy::require_jupiter_program(&crate::JUPITER_V6_PROGRAM_ID).is_ok());
        assert!(policy::require_wrap_mint(true, &crate::WSOL_MINT).is_ok());
        assert_eq!(policy::token_check_grant_amount(&crate::WSOL_MINT, false, 7), 7);
        assert_eq!(policy::token_check_grant_amount(&crate::USDC_MINT, false, 7), 0);
        assert_eq!(policy::TOKEN_NON_SOL_CHECK_GRANT_AMOUNT, 0);
    }

}
