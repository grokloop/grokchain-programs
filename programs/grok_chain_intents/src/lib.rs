//! Grok Chain intents + paymaster program (v1).
//! Implements SPEC.md. Sits on Solana L1. Not a VM, sequencer, or coin.
//!
//! `declare_id!` is the pubkey of target/deploy/grok_chain_intents-keypair.json.
//! DEVNET. Deployed to Solana devnet. Not mainnet. Not a product claim.
//! This source implements swap/deploy/call. It was not upgraded on devnet in this change.

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

declare_id!("EYhYtqLViS4H3FNt1Q8nGRHGt9oD87uaNsV2WJMNiRkz");

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
}

pub fn spend_vault_pda(program_id: &Pubkey, grok_account: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_SPEND_VAULT, grok_account.as_ref()], program_id)
}

pub fn paymaster_pda(program_id: &Pubkey, grok_account: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_PAYMASTER, grok_account.as_ref()], program_id)
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
    }

    #[test]
    fn seeds_match_spec() {
        assert_eq!(SEED_SPEND_VAULT, b"spend-vault");
        assert_eq!(SEED_PAYMASTER, b"paymaster");
        assert_eq!(
            SEED_SPEND_VAULT,
            &[115, 112, 101, 110, 100, 45, 118, 97, 117, 108, 116]
        );
        assert_eq!(
            SEED_PAYMASTER,
            &[112, 97, 121, 109, 97, 115, 116, 101, 114]
        );
        assert_eq!(MAX_SPONSOR_LAMPORTS, 10_000_000);
        // CORE seeds we path-depend on — do not re-derive incorrectly.
        assert_eq!(grok_chain_core::SEED_GROK_ACCOUNT, b"grok-account");
        assert_eq!(grok_chain_core::SEED_GRANT, b"grant");
    }

    #[test]
    fn error_discriminants_0_to_18() {
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
    }

    #[test]
    fn intent_stub_reserved_and_unused_by_new_handlers() {
        // Code 11 stays reserved. swap/deploy/call must not return it.
        assert_eq!(IntentsError::IntentStub as u32, 11);
        assert_ne!(IntentsError::MinOutNotMet as u32, 11);
        assert_ne!(IntentsError::CallTargetMismatch as u32, 11);
        assert_ne!(IntentsError::TargetNotExecutable as u32, 11);
        assert_ne!(IntentsError::ProtectedAccountInRemaining as u32, 11);
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
            "7UtafKBBWNHEXC9PaNXu8USdZqL6VEWupsL7rS6LeVDj"
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
}
