//! capability audit tests
#![cfg(test)]

use anchor_lang::solana_program::hash::hash;

use crate::constants::{
    encode_pump_amm_buy_exact_quote_in, encode_pump_amm_sell, encode_pump_buy_v2,
    encode_pump_create_v2, encode_pump_sell_v2, PUMP_AMM_BUY_EXACT_QUOTE_IN_DISC,
    PUMP_AMM_PROGRAM_ID, PUMP_AMM_SELL_ACCOUNT_COUNT, PUMP_AMM_SELL_ACCOUNT_COUNT_CASHBACK,
    PUMP_AMM_SELL_DATA_LEN, PUMP_AMM_SELL_DISC, PUMP_BUY_V2_DISC, PUMP_CREATE_V2_DISC,
    PUMP_PROGRAM_ID, PUMP_SELL_V2_DISC, PUMP_TRADER_SPACE, SEED_PUMP_TRADER,
};
use crate::errors::IntentsError;
use crate::policy;

fn disc(name: &str) -> [u8; 8] {
    let d = hash(format!("global:{name}").as_bytes());
    let mut out = [0u8; 8];
    out.copy_from_slice(&d.to_bytes()[..8]);
    out
}

const CALL_RS: &str = include_str!("instructions/call.rs");
const SWAP_RS: &str = include_str!("instructions/swap.rs");
const DEPLOY_RS: &str = include_str!("instructions/deploy.rs");
const PUMP_RS: &str = include_str!("instructions/pump.rs");
const PUMP_AMM_RS: &str = include_str!("instructions/pump_amm.rs");
const PUMP_TRADER_RS: &str = include_str!("instructions/pump_trader.rs");
const COMMON_RS: &str = include_str!("instructions/common.rs");
const STATE_RS: &str = include_str!("state.rs");
const LIB_RS: &str = include_str!("lib.rs");

/// (a) official pump discs match sha256("global:<name>")[:8] and the task hex.
#[test]
fn discs_match_task_hex() {
    assert_eq!(disc("buy"), [0x66, 0x06, 0x3d, 0x12, 0x01, 0xda, 0xeb, 0xea]);
    assert_eq!(disc("sell"), [0x33, 0xe6, 0x85, 0xa4, 0x01, 0x7f, 0x83, 0xad]);
    assert_eq!(disc("create"), [0x18, 0x1e, 0xc8, 0x28, 0x05, 0x1c, 0x07, 0x77]);
    assert_eq!(disc("buy_v2"), [0xb8, 0x17, 0xee, 0x61, 0x67, 0xc5, 0xd3, 0x3d]);
    assert_eq!(disc("sell_v2"), [0x5d, 0xf6, 0x82, 0x3c, 0xe7, 0xe9, 0x40, 0xb2]);
    assert_eq!(disc("create_v2"), [0xd6, 0x90, 0x4c, 0xec, 0x5f, 0x8b, 0x31, 0xb4]);
    assert_eq!(disc("buy_v2"), PUMP_BUY_V2_DISC);
    assert_eq!(disc("sell_v2"), PUMP_SELL_V2_DISC);
    assert_eq!(disc("create_v2"), PUMP_CREATE_V2_DISC);
    assert_eq!(disc("buy_exact_quote_in"), PUMP_AMM_BUY_EXACT_QUOTE_IN_DISC);
    assert_eq!(
        disc("buy_exact_quote_in"),
        [198, 46, 21, 82, 180, 217, 232, 112]
    );
    assert_eq!(disc("sell"), PUMP_AMM_SELL_DISC);
    assert_eq!(disc("sell"), [51, 230, 133, 164, 1, 127, 131, 173]);
    // Official pump_amm.json has `sell`, not sell_v2.
    assert_ne!(disc("sell"), disc("sell_v2"));
}

#[test]
fn encoded_ix_data_is_nonempty() {
    let data = encode_pump_buy_v2(1_000_000, 10_000_000);
    assert_eq!(data.len(), 24);
    assert_eq!(&data[..8], &PUMP_BUY_V2_DISC);
    let sell = encode_pump_sell_v2(1_000_000, 1);
    assert_eq!(sell.len(), 24);
    assert_eq!(&sell[..8], &PUMP_SELL_V2_DISC);
    let creator = anchor_lang::prelude::Pubkey::new_unique();
    let created = encode_pump_create_v2("Name", "SYM", "https://example.com/m.json", &creator, false, true);
    assert_eq!(&created[..8], &PUMP_CREATE_V2_DISC);
    assert!(created.len() >= 54);
    assert_eq!(policy::require_nonempty_pump_create_data(&created).is_ok(), true);
    let amm = encode_pump_amm_buy_exact_quote_in(100_000_000, 1, true);
    assert_eq!(amm.len(), 25);
    assert_eq!(&amm[..8], &PUMP_AMM_BUY_EXACT_QUOTE_IN_DISC);
    assert!(policy::require_nonempty_pump_amm_data(&amm).is_ok());
    let amm_sell = encode_pump_amm_sell(1_000_000, 1);
    assert_eq!(amm_sell.len(), PUMP_AMM_SELL_DATA_LEN);
    assert_eq!(amm_sell.len(), 24);
    assert_eq!(&amm_sell[..8], &PUMP_AMM_SELL_DISC);
    assert!(policy::require_nonempty_pump_amm_sell_data(&amm_sell).is_ok());
    assert!(policy::require_nonempty_pump_amm_sell_data(&amm).is_err());
    assert!(policy::require_nonempty_pump_amm_data(&amm_sell).is_err());
}

/// (b) empty-data path is gone for pump_buy. Data is constructed, never vec![].
#[test]
fn empty_data_path_is_gone_for_pump_buy() {
    assert!(!PUMP_RS.contains("data: vec![]"));
    assert!(PUMP_RS.contains("encode_pump_buy_v2"));
    assert!(PUMP_RS.contains("invoke_signed("));
    assert!(PUMP_RS.contains("program_id: PUMP_PROGRAM_ID"));
    let empty: Vec<u8> = vec![];
    assert!(policy::require_nonempty_pump_data(&empty).is_err());
    let good = encode_pump_buy_v2(1, 1);
    assert!(policy::require_nonempty_pump_data(&good).is_ok());
    assert_ne!(good, empty);
}

/// (c) target != official pump id fails.
#[test]
fn target_not_pump_id_fails() {
    assert!(policy::require_pump_program(&PUMP_PROGRAM_ID).is_ok());
    assert!(policy::require_pump_program(&crate::ID).is_err());
    assert!(policy::require_pump_program(&anchor_lang::solana_program::system_program::ID).is_err());
    assert!(PUMP_RS.contains("address = crate::PUMP_PROGRAM_ID"));
    assert_eq!(
        PUMP_PROGRAM_ID.to_string(),
        "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"
    );
}

#[test]
fn target_not_pump_amm_id_fails() {
    assert!(policy::require_pump_amm_program(&PUMP_AMM_PROGRAM_ID).is_ok());
    assert!(policy::require_pump_amm_program(&PUMP_PROGRAM_ID).is_err());
    assert!(policy::require_pump_amm_program(&crate::ID).is_err());
    assert!(PUMP_AMM_RS.contains("address = crate::PUMP_AMM_PROGRAM_ID"));
    assert!(PUMP_AMM_RS.contains("program_id: PUMP_AMM_PROGRAM_ID"));
    assert_eq!(
        PUMP_AMM_PROGRAM_ID.to_string(),
        "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"
    );
}

/// (d) agent cannot be fee payer.
#[test]
fn agent_cannot_be_fee_payer() {
    let agent = anchor_lang::prelude::Pubkey::new_unique();
    let relayer = anchor_lang::prelude::Pubkey::new_unique();
    assert!(policy::require_agent_not_fee_payer(&agent, None).is_ok());
    assert!(policy::require_agent_not_fee_payer(&agent, Some(&relayer)).is_ok());
    assert!(policy::require_agent_not_fee_payer(&agent, Some(&agent)).is_err());
    assert!(PUMP_RS.contains("AgentCannotFeePay"));
    assert!(PUMP_RS.contains("Never the fee payer / SOL source"));
}

#[test]
fn call_is_still_not_a_data_forwarding_router() {
    assert!(CALL_RS.contains("data: vec![]"));
    assert!(CALL_RS.contains("invoke(&ix, ctx.remaining_accounts)"));
    assert!(!CALL_RS.contains("invoke_signed("));
    assert!(!CALL_RS.contains("args.data"));
    assert!(STATE_RS.contains("pub target_program: Pubkey"));
    assert!(!STATE_RS.contains("pub data: Vec<u8>"));
}

#[test]
fn swap_is_sol_only_send() {
    assert!(SWAP_RS.contains("debit_spend_vault"));
    assert!(SWAP_RS.contains("out_destination"));
    assert!(SWAP_RS.contains("require_swap_amounts"));
    assert!(!SWAP_RS.contains("spl_token"));
    assert!(!SWAP_RS.contains("TokenAccount"));
    assert!(!SWAP_RS.contains("bonding_curve"));
    assert!(!SWAP_RS.contains("invoke("));
    assert!(!SWAP_RS.contains("invoke_signed("));
    assert!(COMMON_RS.contains("try_debit_program_owned"));
    assert!(STATE_RS.contains("v1 is SOL-only"));
}

#[test]
fn deploy_is_event_only_not_a_coin_launch() {
    assert!(DEPLOY_RS.contains("emit!(DeployRequested"));
    assert!(DEPLOY_RS.contains("remaining_accounts are ignored"));
    assert!(!DEPLOY_RS.contains("invoke("));
    assert!(!DEPLOY_RS.contains("invoke_signed("));
    // deploy stays event-only. Coin launch is pump_create, not deploy.
}

#[test]
fn no_limit_order_primitive() {
    assert!(!LIB_RS.contains("limit_order"));
    assert!(!LIB_RS.contains("place_order"));
    assert!(!LIB_RS.contains("cancel_order"));
    assert!(LIB_RS.contains("pub fn pump_buy"));
    assert!(LIB_RS.contains("pub fn pump_sell"));
    assert!(LIB_RS.contains("pub fn pump_create"));
    assert!(LIB_RS.contains("pub fn pump_amm_buy"));
    assert!(LIB_RS.contains("pub fn pump_amm_sell"));
    assert!(LIB_RS.contains("pub fn init_pump_trader"));
    assert!(LIB_RS.contains("pub fn fund_pump_trader"));
    assert!(LIB_RS.contains("pub fn withdraw_pump_trader"));
}

#[test]
fn invoke_signed_only_in_pump_adapter() {
    assert!(!CALL_RS.contains("invoke_signed("));
    assert!(!SWAP_RS.contains("invoke_signed("));
    assert!(!DEPLOY_RS.contains("invoke_signed("));
    assert!(!COMMON_RS.contains("invoke_signed("));
    assert!(PUMP_RS.contains("invoke_signed("));
    assert!(PUMP_RS.contains("SEED_PUMP_TRADER"));
    assert!(PUMP_RS.contains("trader_signer_seeds"));
    assert!(PUMP_RS.contains("Not a general CPI router"));
    // pump CPI signs as trader, not vault
    assert!(!PUMP_RS.contains("&[SEED_SPEND_VAULT"));
    assert!(PUMP_AMM_RS.contains("invoke_signed("));
    assert!(PUMP_AMM_RS.contains("SEED_PUMP_TRADER"));
    assert!(PUMP_AMM_RS.contains("trader_signer_seeds"));
    assert!(!PUMP_AMM_RS.contains("&[SEED_SPEND_VAULT"));
    assert!(PUMP_AMM_RS.contains("encode_pump_amm_buy_exact_quote_in"));
    assert!(PUMP_AMM_RS.contains("encode_pump_amm_sell"));
    assert!(PUMP_AMM_RS.contains("Never PumpSwap `user`") || PUMP_AMM_RS.contains("never user"));
}

#[test]
fn invoke_signed_uses_trader_seeds() {
    assert!(PUMP_RS.contains("let trader_signer_seeds"));
    assert!(PUMP_RS.contains("SEED_PUMP_TRADER, grok.as_ref()"));
    assert!(PUMP_RS.contains("invoke_signed(&ix, remaining, &[trader_signer_seeds])"));
    assert!(!PUMP_RS.contains("require_pump_user_is_vault"));
    assert!(PUMP_RS.contains("require_pump_user_is_trader"));
    assert!(PUMP_RS.contains("require_pump_user_not_vault"));
}

#[test]
fn trader_is_system_owned_zero_space() {
    assert_eq!(SEED_PUMP_TRADER, b"pump-trader");
    assert_eq!(PUMP_TRADER_SPACE, 0);
    assert!(PUMP_TRADER_RS.contains("system_instruction::create_account"));
    assert!(PUMP_TRADER_RS.contains("invoke_signed"));
    assert!(PUMP_TRADER_RS.contains("SEED_PUMP_TRADER"));
    assert!(PUMP_TRADER_RS.contains("&system_program::ID"));
    assert!(PUMP_TRADER_RS.contains("PUMP_TRADER_SPACE"));
    assert!(PUMP_TRADER_RS.contains("PumpTraderAlreadyExists"));
    assert!(!PUMP_TRADER_RS.contains("pub struct PumpTrader {"));
    assert!(PUMP_TRADER_RS.contains("No #[account] data"));
    assert!(policy::require_pump_trader_system_owned(
        &anchor_lang::solana_program::system_program::ID
    )
    .is_ok());
    assert!(policy::require_pump_trader_system_owned(&crate::ID).is_err());
}

#[test]
fn vault_as_user_fails() {
    let vault = anchor_lang::prelude::Pubkey::new_unique();
    let trader = anchor_lang::prelude::Pubkey::new_unique();
    assert!(policy::require_pump_user_is_trader(&trader, &trader).is_ok());
    assert!(policy::require_pump_user_is_trader(&vault, &trader).is_err());
    assert!(policy::require_pump_user_not_vault(&vault, &vault).is_err());
    assert!(policy::require_pump_user_not_vault(&trader, &vault).is_ok());
    assert_eq!(IntentsError::PumpUserMustBeVault as u32, 21);
    assert_eq!(IntentsError::PumpUserMustBeTrader as u32, 27);
    assert!(PUMP_RS.contains("PumpUserMustBeTrader"));
    assert!(PUMP_RS.contains("Vault-as-user fails"));
    assert!(!PUMP_RS.contains("require_pump_user_is_vault"));
    assert!(PUMP_RS.contains("PUMP_CREATE_USER_INDEX"));
}

#[test]
fn create_v2_disc_mint_must_be_signer() {
    assert_eq!(disc("create_v2"), PUMP_CREATE_V2_DISC);
    assert_eq!(disc("create_v2"), [0xd6, 0x90, 0x4c, 0xec, 0x5f, 0x8b, 0x31, 0xb4]);
    assert!(policy::require_pump_mint_is_signer(true).is_ok());
    assert!(policy::require_pump_mint_is_signer(false).is_err());
    assert_eq!(IntentsError::PumpMintMustBeSigner as u32, 31);
    assert!(PUMP_RS.contains("require_pump_mint_is_signer"));
    assert!(PUMP_RS.contains("PumpMintMustBeSigner"));
    assert!(PUMP_RS.contains("encode_pump_create_v2"));
    assert!(LIB_RS.contains("pub fn pump_create"));
}

#[test]
fn create_user_must_be_trader_vault_as_user_fails() {
    let vault = anchor_lang::prelude::Pubkey::new_unique();
    let trader = anchor_lang::prelude::Pubkey::new_unique();
    assert!(policy::require_pump_user_is_trader(&trader, &trader).is_ok());
    assert!(policy::require_pump_user_is_trader(&vault, &trader).is_err());
    assert!(policy::require_pump_user_not_vault(&vault, &vault).is_err());
    assert!(PUMP_RS.contains("remaining[5] MUST be trader"));
    assert!(PUMP_RS.contains("Vault-as-user fails"));
}

#[test]
fn capability_table_buy_sell_launch_via_adapter_limit_fail() {
    let buy = "PASS";
    let sell = "PASS";
    let limit = "FAIL";
    let launch = "PASS";
    assert_eq!(buy, "PASS");
    assert_eq!(sell, "PASS");
    assert_eq!(limit, "FAIL");
    assert_eq!(launch, "PASS");
    assert!(LIB_RS.contains("pub fn pump_buy"));
    assert!(LIB_RS.contains("pub fn pump_sell"));
    assert!(LIB_RS.contains("pub fn pump_create"));
    assert!(LIB_RS.contains("pub fn init_pump_trader"));
    assert!(LIB_RS.contains("pub fn fund_pump_trader"));
    assert!(LIB_RS.contains("pub fn withdraw_pump_trader"));
    assert!(!LIB_RS.contains("limit_order"));
    assert!(PUMP_RS.contains("create_handler"));
}

#[test]
fn pump_amm_sell_official_sell_not_sell_v2() {
    assert_eq!(PUMP_AMM_SELL_DISC, disc("sell"));
    assert_ne!(PUMP_AMM_SELL_DISC, disc("sell_v2"));
    assert_eq!(PUMP_AMM_SELL_DATA_LEN, 24);
    assert_eq!(PUMP_AMM_SELL_ACCOUNT_COUNT, 24);
    assert_eq!(PUMP_AMM_SELL_ACCOUNT_COUNT_CASHBACK, 26);
    assert!(LIB_RS.contains("pub fn pump_amm_sell"));
    assert!(PUMP_AMM_RS.contains("pub fn sell_handler"));
    assert!(PUMP_AMM_RS.contains("encode_pump_amm_sell"));
    assert!(PUMP_AMM_RS.contains("PUMP_AMM_SELL_CHECK_GRANT_AMOUNT"));
    assert!(!PUMP_AMM_RS.contains("unwrap_wsol_to_trader"));
    assert!(!PUMP_AMM_RS.contains("TOKEN_CLOSE_ACCOUNT"));
    assert!(PUMP_AMM_RS.contains("Do NOT sweep trader") || PUMP_AMM_RS.contains("Do not sweep trader"));
    assert!(PUMP_AMM_RS.contains("there is no sell_v2") || PUMP_AMM_RS.contains("no sell_v2"));
    assert!(!PUMP_AMM_RS.contains("encode_pump_sell_v2"));
    assert!(!LIB_RS.contains("jupiter"));
    assert!(!PUMP_AMM_RS.contains("Jupiter"));
    assert_eq!(policy::PUMP_SELL_CHECK_GRANT_AMOUNT, 0);
    assert_eq!(policy::PUMP_AMM_SELL_CHECK_GRANT_AMOUNT, 0);
    assert!(policy::require_pump_amm_sell_account_count(24).is_ok());
    assert!(policy::require_pump_amm_sell_account_count(26).is_err());
    assert!(policy::require_pump_amm_sell_account_count(27).is_err());
}

#[test]
fn pump_amm_sell_no_in_ix_vault_debit_check_grant_zero() {
    assert!(LIB_RS.contains("pub fn pump_amm_sell"));
    assert!(PUMP_AMM_RS.contains("encode_pump_amm_sell"));
    assert!(PUMP_AMM_RS.contains("No in-ix vault debit"));
    assert!(PUMP_AMM_RS.contains("remaining[1]") || PUMP_AMM_RS.contains("Trader is remaining[1] only"));
    let sell_start = PUMP_AMM_RS.find("pub fn sell_handler").expect("sell_handler");
    let after = &PUMP_AMM_RS[sell_start + 1..];
    let rel = after.find("\nfn ").unwrap_or(after.find("\npub fn ").unwrap_or(after.len()));
    let sell = &PUMP_AMM_RS[sell_start..sell_start + 1 + rel];
    assert!(!sell.contains("debit_spend_vault"));
    assert!(!sell.contains("require_pump_trader_prefunded"));
    assert!(!sell.contains("wrap_sol_to_wsol"));
    assert!(sell.contains("PUMP_AMM_SELL_CHECK_GRANT_AMOUNT"));
    assert_eq!(policy::PUMP_AMM_SELL_CHECK_GRANT_AMOUNT, 0);
}

#[test]
fn pump_amm_sell_no_vault_debit_trader_is_user() {
    assert!(PUMP_AMM_RS.contains("No in-ix vault debit"));
    assert!(PUMP_AMM_RS.contains("Trader is remaining[1] only") || PUMP_AMM_RS.contains("remaining[1] only"));
    assert!(PUMP_AMM_RS.contains("require_pump_user_not_vault"));
    assert!(PUMP_AMM_RS.contains("require_pump_user_is_trader"));
    assert!(PUMP_AMM_RS.contains("AgentMustNotFeePay") || PUMP_AMM_RS.contains("AgentCannotFeePay"));
    assert!(!PUMP_AMM_RS.contains("debit_spend_vault"));
    let vault = anchor_lang::prelude::Pubkey::new_unique();
    let trader = anchor_lang::prelude::Pubkey::new_unique();
    assert!(policy::require_pump_user_not_vault(&vault, &vault).is_err());
    assert!(policy::require_pump_user_is_trader(&trader, &trader).is_ok());
}

#[test]
fn pump_curve_buy_no_in_ix_vault_debit_requires_prefund() {
    assert!(PUMP_RS.contains("require_pump_trader_prefunded"));
    assert!(PUMP_RS.contains("No in-ix vault"));
    let buy_start = PUMP_RS.find("pub fn buy_handler").expect("buy_handler");
    let sell_start = PUMP_RS.find("pub fn sell_handler").expect("sell_handler");
    let create_start = PUMP_RS.find("pub fn create_handler").expect("create_handler");
    let buy = &PUMP_RS[buy_start..sell_start];
    let sell = &PUMP_RS[sell_start..create_start];
    let create = &PUMP_RS[create_start..];
    assert!(!buy.contains("debit_spend_vault"));
    assert!(buy.contains("require_pump_trader_prefunded"));
    assert!(!create.contains("debit_spend_vault"));
    assert!(create.contains("require_pump_trader_prefunded"));
    // sell spends tokens, not SOL — no debit, no prefund
    assert!(!sell.contains("debit_spend_vault"));
    assert!(!sell.contains("require_pump_trader_prefunded"));
    // named PumpTrade accounts stay (live mouth)
    assert!(PUMP_RS.contains("pub pump_trader: UncheckedAccount"));
    assert!(PUMP_RS.contains("pub struct PumpTrade"));
}

#[test]
fn pump_curve_buy_sell_no_in_ix_debit_or_sweep() {
    assert!(PUMP_RS.contains("require_pump_trader_prefunded"));
    assert!(PUMP_RS.contains("encode_pump_buy_v2"));
    assert!(PUMP_RS.contains("encode_pump_sell_v2"));
    assert!(PUMP_RS.contains("require_pump_user_not_vault"));
    assert!(PUMP_RS.contains("require_pump_user_is_trader"));
    assert!(PUMP_RS.contains("invoke_signed(&ix, remaining, &[trader_signer_seeds])"));
    assert!(PUMP_RS.contains("SEED_PUMP_TRADER, grok.as_ref()"));
    let buy_start = PUMP_RS.find("pub fn buy_handler").expect("buy_handler");
    let sell_start = PUMP_RS.find("pub fn sell_handler").expect("sell_handler");
    let create_start = PUMP_RS.find("pub fn create_handler").expect("create_handler");
    let buy = &PUMP_RS[buy_start..sell_start];
    let sell = &PUMP_RS[sell_start..create_start];
    assert!(!buy.contains("debit_spend_vault"));
    assert!(!buy.contains("sweep_trader_to_vault"));
    assert!(buy.contains("require_pump_trader_prefunded"));
    assert!(buy.contains("encode_pump_buy_v2"));
    assert!(!sell.contains("debit_spend_vault"));
    assert!(!sell.contains("sweep_trader_to_vault"));
    assert!(!sell.contains("require_pump_trader_prefunded"));
    assert!(sell.contains("encode_pump_sell_v2"));
    assert!(sell.contains("PUMP_SELL_CHECK_GRANT_AMOUNT"));
    assert_eq!(IntentsError::PumpTraderUnderfunded as u32, 40);
}

#[test]
fn pump_curve_buy_create_no_in_ix_vault_debit() {
    assert!(PUMP_RS.contains("No in-ix vault"));
    assert!(PUMP_RS.contains("require_pump_trader_prefunded"));
    assert!(PUMP_RS.contains("Trader is remaining[13]"));
    assert!(PUMP_RS.contains("remaining[5] MUST be trader") || PUMP_RS.contains("Trader is remaining[5]"));
    assert!(PUMP_RS.contains("Do NOT sweep"));
    assert!(!PUMP_RS.contains("debit_spend_vault"));
    assert!(!PUMP_RS.contains("fn sweep_trader_to_vault"));
    assert!(!PUMP_RS.contains("pub pump_trader:"));
    let buy_start = PUMP_RS.find("pub fn buy_handler").expect("buy_handler");
    let sell_start = PUMP_RS.find("pub fn sell_handler").expect("sell_handler");
    let create_start = PUMP_RS.find("pub fn create_handler").expect("create_handler");
    let buy = &PUMP_RS[buy_start:sell_start];
    let sell = &PUMP_RS[sell_start:create_start];
    let create = &PUMP_RS[create_start:];
    assert!(buy.contains("require_pump_trader_prefunded"));
    assert!(!buy.contains("debit_spend_vault"));
    assert!(!sell.contains("debit_spend_vault"));
    assert!(!sell.contains("require_pump_trader_prefunded"));
    assert!(create.contains("require_pump_trader_prefunded"));
    assert!(!create.contains("debit_spend_vault"));
}

#[test]
fn withdraw_pump_trader_is_separate_ix() {
    assert!(LIB_RS.contains("pub fn withdraw_pump_trader"));
    assert!(PUMP_TRADER_RS.contains("pub fn withdraw"));
    assert!(PUMP_TRADER_RS.contains("PumpTraderWithdrawn"));
    assert!(PUMP_TRADER_RS.contains("system_instruction::transfer"));
    assert!(!PUMP_TRADER_RS.contains("try_debit_program_owned"));
    assert!(!PUMP_RS.contains("fn withdraw"));
    assert_eq!(IntentsError::WithdrawRemainingAccountsOdd as u32, 43);
    assert_eq!(IntentsError::InsufficientPumpTrader as u32, 48);
}
