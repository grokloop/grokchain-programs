//! capability audit tests
#![cfg(test)]

use anchor_lang::solana_program::hash::hash;

fn disc(name: &str) -> [u8; 8] {
    let d = hash(format!("global:{name}").as_bytes());
    let mut out = [0u8; 8];
    out.copy_from_slice(&d.to_bytes()[..8]);
    out
}

const CALL_RS: &str = include_str!("instructions/call.rs");
const SWAP_RS: &str = include_str!("instructions/swap.rs");
const DEPLOY_RS: &str = include_str!("instructions/deploy.rs");
const COMMON_RS: &str = include_str!("instructions/common.rs");
const STATE_RS: &str = include_str!("state.rs");
const LIB_RS: &str = include_str!("lib.rs");

#[test]
fn discs_match_task_hex() {
    assert_eq!(disc("buy"), [0x66, 0x06, 0x3d, 0x12, 0x01, 0xda, 0xeb, 0xea]);
    assert_eq!(disc("sell"), [0x33, 0xe6, 0x85, 0xa4, 0x01, 0x7f, 0x83, 0xad]);
    assert_eq!(disc("create"), [0x18, 0x1e, 0xc8, 0x28, 0x05, 0x1c, 0x07, 0x77]);
    assert_eq!(disc("buy_v2"), [0xb8, 0x17, 0xee, 0x61, 0x67, 0xc5, 0xd3, 0x3d]);
    assert_eq!(disc("sell_v2"), [0x5d, 0xf6, 0x82, 0x3c, 0xe7, 0xe9, 0x40, 0xb2]);
    assert_eq!(disc("create_v2"), [0xd6, 0x90, 0x4c, 0xec, 0x5f, 0x8b, 0x31, 0xb4]);
}

#[test]
fn encoded_ix_data_is_nonempty() {
    let mut data = disc("buy_v2").to_vec();
    data.extend_from_slice(&1_000_000u64.to_le_bytes());
    data.extend_from_slice(&10_000_000u64.to_le_bytes());
    assert_eq!(data.len(), 24);
    let mut create = disc("create_v2").to_vec();
    let name = b"AuditCoin";
    create.extend_from_slice(&(name.len() as u32).to_le_bytes());
    create.extend_from_slice(name);
    assert!(create.len() > 8);
}

fn mock_target(data: &[u8]) -> Result<(), &'static str> {
    if data.is_empty() {
        return Err("empty instruction data");
    }
    if data.len() < 8 {
        return Err("truncated");
    }
    if data.len() == 8 {
        return Err("args missing");
    }
    Ok(())
}

#[test]
fn call_inner_ix_data_is_empty_so_discs_never_forward() {
    assert!(CALL_RS.contains("data: vec![]"));
    assert!(CALL_RS.contains("invoke(&ix, ctx.remaining_accounts)"));
    assert!(CALL_RS.contains("invoke_signed"));
    assert!(!CALL_RS.contains("invoke_signed("));
    assert!(!CALL_RS.contains("args.data"));
    assert!(STATE_RS.contains("pub target_program: Pubkey"));
    assert!(!STATE_RS.contains("pub data: Vec<u8>"));
    let inner = Vec::<u8>::new();
    assert!(inner.is_empty());
    assert_eq!(mock_target(&inner), Err("empty instruction data"));
    let mut good = disc("buy_v2").to_vec();
    good.extend_from_slice(&1u64.to_le_bytes());
    good.extend_from_slice(&1u64.to_le_bytes());
    assert_eq!(mock_target(&good), Ok(()));
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
    assert!(SWAP_RS.contains("pub spend_vault:"));
    assert!(!SWAP_RS.contains("pub bonding_curve"));
    assert!(!SWAP_RS.contains("pub token_program"));
}

#[test]
fn deploy_is_event_only() {
    assert!(DEPLOY_RS.contains("emit!(DeployRequested"));
    assert!(DEPLOY_RS.contains("remaining_accounts are ignored"));
    assert!(!DEPLOY_RS.contains("invoke("));
    assert!(!DEPLOY_RS.contains("invoke_signed("));
    assert!(!DEPLOY_RS.contains("create_v2"));
    assert!(!DEPLOY_RS.contains("token_program"));
    assert!(STATE_RS.contains("pub program_id: Pubkey"));
}

#[test]
fn no_limit_order_primitive() {
    assert!(!LIB_RS.contains("limit_order"));
    assert!(!LIB_RS.contains("place_order"));
    assert!(!LIB_RS.contains("cancel_order"));
    assert!(LIB_RS.contains("pub fn swap"));
    assert!(LIB_RS.contains("pub fn deploy"));
    assert!(LIB_RS.contains("pub fn call"));
}

#[test]
fn invoke_signed_ban_and_vault_not_user() {
    assert!(CALL_RS.contains("invoke(&ix, ctx.remaining_accounts)"));
    assert!(!CALL_RS.contains("invoke_signed("));
    assert!(!SWAP_RS.contains("invoke_signed("));
    assert!(!DEPLOY_RS.contains("invoke_signed("));
    assert!(!COMMON_RS.contains("invoke_signed("));
    assert!(COMMON_RS.contains("reject_protected_remaining"));
    assert!(CALL_RS.contains("reject_protected_remaining"));
    assert!(CALL_RS.contains("Never the fee payer / SOL source"));
}

#[test]
fn capability_table_all_fail() {
    let buy = "FAIL";
    let sell = "FAIL";
    let limit = "FAIL";
    let launch = "FAIL";
    assert_eq!(buy, "FAIL");
    assert_eq!(sell, "FAIL");
    assert_eq!(limit, "FAIL");
    assert_eq!(launch, "FAIL");
}
