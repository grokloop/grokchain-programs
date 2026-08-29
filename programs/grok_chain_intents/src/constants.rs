//! SPEC.md §3. Do not change seed bytes.

use anchor_lang::prelude::*;

/// SpendVault PDA seed. Hex: `73 70 65 6e 64 2d 76 61 75 6c 74`.
pub const SEED_SPEND_VAULT: &[u8] = b"spend-vault";

/// Paymaster PDA seed. Hex: `70 61 79 6d 61 73 74 65 72`.
pub const SEED_PAYMASTER: &[u8] = b"paymaster";

/// PumpTrader PDA seed. Hex: `70 75 6d 70 2d 74 72 61 64 65 72`.
/// 0-byte system-owned PDA. No #[account] data. pump.fun `user`.
pub const SEED_PUMP_TRADER: &[u8] = b"pump-trader";
/// Root-owned payee allowlist. A CORE grant caps an amount and names a program,
/// never a recipient, so the merchant list has to live here.
pub const SEED_MERCHANTS: &[u8] = b"merchants";
/// Bounds the registry account size. 32 payees is plenty for one account and
/// keeps the linear scan in pay_token cheap.
pub const MAX_MERCHANTS: usize = 32;
/// One subscription per (grok_account, merchant, mint).
pub const SEED_SUBSCRIPTION: &[u8] = b"subscription";

/// PumpTrader space. Always 0. Owner is the System Program, not INTENTS.
pub const PUMP_TRADER_SPACE: usize = 0;

/// Hard per-intent reimbursement cap (0.01 SOL). Arg may be smaller. 0 = no sponsor.
pub const MAX_SPONSOR_LAMPORTS: u64 = 10_000_000;

/// CORE `check_grant` disc: `sha256("global:check_grant")[0..8]`. SPEC.md §5.4 / §13.
pub const CHECK_GRANT_DISCRIMINATOR: [u8; 8] = [223, 172, 131, 140, 15, 133, 209, 250];

/// CORE check_grant metas (order normative). is_writable per account index 0..=3.
pub const CHECK_GRANT_META_WRITABLE: [bool; 4] = [false, true, false, false];

/// CORE check_grant metas (order normative). is_signer per account index 0..=3.
pub const CHECK_GRANT_META_SIGNER: [bool; 4] = [false, false, true, false];

/// Official pump.fun program (mainnet and devnet). The only inner program
/// `pump_buy` / `pump_sell` / `pump_create` will CPI into. Not a general router allowlist.
pub const PUMP_PROGRAM_ID: Pubkey = pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");

/// Official pump.fun Mayhem program (create_v2 account 9 / IDL address).
pub const MAYHEM_PROGRAM_ID: Pubkey = pubkey!("MAyhSmzXzV1pTf7LsNkrNwkWKTo4ougAJ1PPg47MD4e");

/// Token-2022 (create_v2 coins, including 2x4i…pump).
pub const TOKEN_2022_PROGRAM_ID: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

/// Legacy SPL Token (legacy pump coins).
pub const TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// Associated Token Program.
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

/// `sha256("global:buy_v2")[0..8]`. Official BUY.md.
pub const PUMP_BUY_V2_DISC: [u8; 8] = [0xb8, 0x17, 0xee, 0x61, 0x67, 0xc5, 0xd3, 0x3d];

/// `sha256("global:sell_v2")[0..8]`. Official SELL.md.
pub const PUMP_SELL_V2_DISC: [u8; 8] = [0x5d, 0xf6, 0x82, 0x3c, 0xe7, 0xe9, 0x40, 0xb2];

/// Official buy_v2 account count (BUY.md). All mandatory.
pub const PUMP_BUY_V2_ACCOUNT_COUNT: usize = 27;

/// Official sell_v2 account count (SELL.md). All mandatory.
pub const PUMP_SELL_V2_ACCOUNT_COUNT: usize = 26;

/// 0-based index of `user` in official buy_v2 / sell_v2 account lists.
pub const PUMP_USER_INDEX: usize = 13;

/// 0-based index of `program` (pump program account) in buy_v2.
pub const PUMP_PROGRAM_INDEX_BUY: usize = 26;

/// 0-based index of `program` in sell_v2.
pub const PUMP_PROGRAM_INDEX_SELL: usize = 25;

/// Official buy_v2 / sell_v2 data is disc (8) + two u64s.
pub const PUMP_TRADE_IX_DATA_LEN: usize = 24;

/// `sha256("global:create_v2")[0..8]`. Official COIN_CREATION.md / IDL.
pub const PUMP_CREATE_V2_DISC: [u8; 8] = [0xd6, 0x90, 0x4c, 0xec, 0x5f, 0x8b, 0x31, 0xb4];

/// Official create_v2 IDL account count (COIN_CREATION.md accounts 1-16). SOL-paired.
pub const PUMP_CREATE_V2_ACCOUNT_COUNT: usize = 16;

/// Official create_v2 + three optional quote remaining accounts (COIN_CREATION.md 17-19).
pub const PUMP_CREATE_V2_ACCOUNT_COUNT_WITH_QUOTE: usize = 19;

/// 0-based index of `mint` in official create_v2 (writable signer).
pub const PUMP_CREATE_MINT_INDEX: usize = 0;

/// 0-based index of `user` in official create_v2 (writable signer / payer).
pub const PUMP_CREATE_USER_INDEX: usize = 5;

/// 0-based index of `program` in official create_v2.
pub const PUMP_CREATE_PROGRAM_INDEX: usize = 15;

/// Official create_v2 name / symbol / uri character caps (COIN_CREATION.md).
pub const PUMP_CREATE_NAME_MAX: usize = 32;
pub const PUMP_CREATE_SYMBOL_MAX: usize = 13;
pub const PUMP_CREATE_URI_MAX: usize = 200;

/// Minimum create_v2 payload: disc + three empty Borsh strings + creator + two bools.
pub const PUMP_CREATE_V2_IX_DATA_MIN: usize = 8 + 4 + 4 + 4 + 32 + 1 + 1;

/// Documented Grok token CA (Token-2022). Adapter is mint-agnostic; this is
/// the coin a later local test buy would target. Not a hardcoded allowlist.
pub const GROK_TOKEN_MINT: Pubkey = pubkey!("2x4iY5AaiGyRfxzHzSY1KzQJ7K82SDqmkMApwbcRpump");

/// Construct official `buy_v2` ix data. Never empty.
pub fn encode_pump_buy_v2(amount: u64, max_sol_cost: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(PUMP_TRADE_IX_DATA_LEN);
    data.extend_from_slice(&PUMP_BUY_V2_DISC);
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&max_sol_cost.to_le_bytes());
    data
}

/// Construct official `sell_v2` ix data. Never empty.
pub fn encode_pump_sell_v2(amount: u64, min_sol_output: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(PUMP_TRADE_IX_DATA_LEN);
    data.extend_from_slice(&PUMP_SELL_V2_DISC);
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&min_sol_output.to_le_bytes());
    data
}

fn encode_borsh_string(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
    out
}

/// Official `create_v2` data (COIN_CREATION.md / IDL).
/// `creator` is forced by the adapter to `grok_account.root`.
/// `is_cashback_enabled` is official `OptionBool` = struct { bool } (1 byte).
pub fn encode_pump_create_v2(
    name: &str,
    symbol: &str,
    uri: &str,
    creator: &Pubkey,
    is_mayhem_mode: bool,
    is_cashback_enabled: bool,
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&PUMP_CREATE_V2_DISC);
    data.extend_from_slice(&encode_borsh_string(name));
    data.extend_from_slice(&encode_borsh_string(symbol));
    data.extend_from_slice(&encode_borsh_string(uri));
    data.extend_from_slice(creator.as_ref());
    data.push(u8::from(is_mayhem_mode));
    data.push(u8::from(is_cashback_enabled));
    data
}

/// Official PumpSwap AMM (post-graduation). The only inner program
/// `pump_amm_buy` will CPI into.
pub const PUMP_AMM_PROGRAM_ID: Pubkey = pubkey!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA");

/// Official pump fee program (AMM buy account 22).
pub const PUMP_AMM_FEE_PROGRAM_ID: Pubkey = pubkey!("pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ");

/// Wrapped SOL mint (PumpSwap quote for SOL-paired graduated coins).
pub const WSOL_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

/// `sha256("global:buy_exact_quote_in")[0..8]`. Official pump_amm.json.
pub const PUMP_AMM_BUY_EXACT_QUOTE_IN_DISC: [u8; 8] = [198, 46, 21, 82, 180, 217, 232, 112];

/// Official IDL buy / buy_exact_quote_in account count (23) + pool_v2 +
/// breaking fee recipient + recipient quote ATA. Non-cashback.
pub const PUMP_AMM_BUY_ACCOUNT_COUNT: usize = 26;

/// Cashback: + user_volume_accumulator quote ATA before pool_v2.
pub const PUMP_AMM_BUY_ACCOUNT_COUNT_CASHBACK: usize = 27;

/// 0-based index of `user` in official PumpSwap buy account list.
pub const PUMP_AMM_USER_INDEX: usize = 1;

/// 0-based index of `program` in official PumpSwap buy account list.
pub const PUMP_AMM_PROGRAM_INDEX: usize = 16;

/// Official buy_exact_quote_in data: disc (8) + two u64s + OptionBool (1).
pub const PUMP_AMM_BUY_EXACT_QUOTE_IN_DATA_LEN: usize = 25;

/// Official GlobalConfig protocol fee recipients (PUMP_SWAP_README.md).
pub const PUMP_AMM_PROTOCOL_FEE_RECIPIENTS: [Pubkey; 8] = [
    pubkey!("62qc2CNXwrYqQScmEdiZFFAnJR262PxWEuNQtxfafNgV"),
    pubkey!("7VtfL8fvgNfhz17qKRMjzQEXgbdpnHHHQRh54R9jP2RJ"),
    pubkey!("7hTckgnGnLQR6sdH7YkqFTAA7VwTfYFaZ6EhEsU3saCX"),
    pubkey!("9rPYyANsfQZw3DnDmKE3YCQF5E8oD89UXoHn9JFEhJUz"),
    pubkey!("AVmoTthdrX6tKt4nDjco2D775W2YK3sDhxPcMmzUAmTY"),
    pubkey!("FWsW1xNtWscwNmKv6wVsU1iTzRN6wmmk3MjxRP5tT7hz"),
    pubkey!("G5UZAVbAf46s7cKWoyKu8kYTip9DGTpbLZ2qa9Aq69dP"),
    pubkey!("JCRGumoE9Qi5BBgULTgdgTLjSgkCMSbF62ZZfGs84JeU"),
];

/// Official reserved (mayhem) fee recipients (FEE_RECIPIENTS.md).
pub const PUMP_AMM_RESERVED_FEE_RECIPIENTS: [Pubkey; 8] = [
    pubkey!("GesfTA3X2arioaHp8bbKdjG9vJtskViWACZoYvxp4twS"),
    pubkey!("4budycTjhs9fD6xw62VBducVTNgMgJJ5BgtKq7mAZwn6"),
    pubkey!("8SBKzEQU4nLSzcwF4a74F2iaUDQyTfjGndn6qUWBnrpR"),
    pubkey!("4UQeTP1T39KZ9Sfxzo3WR5skgsaP6NZa87BAkuazLEKH"),
    pubkey!("8sNeir4QsLsJdYpc9RZacohhK1Y5FLU3nC5LXgYB4aa6"),
    pubkey!("Fh9HmeLNUMVCvejxCtCL2DbYaRyBFVJ5xrWkLnMH6fdk"),
    pubkey!("463MEnMeGyJekNZFQSTUABBEbLnvMTALbT6ZmsxAbAdq"),
    pubkey!("6AUH3WEHucYZyC61hqpqYUWVto5qA5hjHuNQ32GNnNxA"),
];

/// Official breaking fee recipients (BREAKING_FEE_RECIPIENT.md).
pub const PUMP_AMM_BREAKING_FEE_RECIPIENTS: [Pubkey; 8] = [
    pubkey!("5YxQFdt3Tr9zJLvkFccqXVUwhdTWJQc1fFg2YPbxvxeD"),
    pubkey!("9M4giFFMxmFGXtc3feFzRai56WbBqehoSeRE5GK7gf7"),
    pubkey!("GXPFM2caqTtQYC2cJ5yJRi9VDkpsYZXzYdwYpGnLmtDL"),
    pubkey!("3BpXnfJaUTiwXnJNe7Ej1rcbzqTTQUvLShZaWazebsVR"),
    pubkey!("5cjcW9wExnJJiqgLjq7DEG75Pm6JBgE1hNv4B2vHXUW6"),
    pubkey!("EHAAiTxcdDwQ3U4bU6YcMsQGaekdzLS3B5SmYo46kJtL"),
    pubkey!("5eHhjP8JaYkz83CWwvGU2uMUXefd3AazWGx4gpcuEEYD"),
    pubkey!("A7hAgCzFw14fejgCp387JUJRMNyz4j89JKnhtKU8piqW"),
];

/// Construct official PumpSwap `buy_exact_quote_in` ix data. Never empty.
/// `track_volume` is official OptionBool = struct { bool } (1 byte).
pub fn encode_pump_amm_buy_exact_quote_in(
    spendable_quote_in: u64,
    min_base_amount_out: u64,
    track_volume: bool,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(PUMP_AMM_BUY_EXACT_QUOTE_IN_DATA_LEN);
    data.extend_from_slice(&PUMP_AMM_BUY_EXACT_QUOTE_IN_DISC);
    data.extend_from_slice(&spendable_quote_in.to_le_bytes());
    data.extend_from_slice(&min_base_amount_out.to_le_bytes());
    data.push(u8::from(track_volume));
    data
}

/// `sha256("global:sell")[0..8]`. Official pump_amm.json — there is no sell_v2.
pub const PUMP_AMM_SELL_DISC: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];

/// Official IDL `sell` named accounts (21) + pool_v2 + breaking fee recipient +
/// recipient quote ATA. Non-cashback. BREAKING_FEE_RECIPIENT.md: 24.
/// Sell-only vs buy: IDL omits global_volume + user_volume, so fee_config /
/// fee_program sit at 19/20 (buy has them at 21/22). Do not pass buy's 26.
pub const PUMP_AMM_SELL_ACCOUNT_COUNT: usize = 24;

/// Cashback sell: + UVA quote ATA + UVA before pool_v2 (PUMP_CASHBACK_README.md).
/// 21 IDL + 2 cashback + pool_v2 + breaking + ATA = 26.
pub const PUMP_AMM_SELL_ACCOUNT_COUNT_CASHBACK: usize = 26;

/// Official IDL `sell` named-account count (no volume accumulators).
pub const PUMP_AMM_SELL_IDL_ACCOUNT_COUNT: usize = 21;

/// Official sell data: disc (8) + base_amount_in u64 + min_quote_amount_out u64.
/// No OptionBool / track_volume (buy_exact_quote_in has that extra byte).
pub const PUMP_AMM_SELL_DATA_LEN: usize = 24;

/// Construct official PumpSwap `sell` ix data. Never empty.
pub fn encode_pump_amm_sell(base_amount_in: u64, min_quote_amount_out: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(PUMP_AMM_SELL_DATA_LEN);
    data.extend_from_slice(&PUMP_AMM_SELL_DISC);
    data.extend_from_slice(&base_amount_in.to_le_bytes());
    data.extend_from_slice(&min_quote_amount_out.to_le_bytes());
    data
}

/// Official Jupiter v6 aggregator. The only inner program `token_buy` /
/// `token_sell` will CPI into. Not a general router allowlist.
pub const JUPITER_V6_PROGRAM_ID: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");

/// Official Circle USDC (mainnet). Quote mint MAY be this, WSOL, or another
/// SPL / Token-2022 mint. Not an allowlist — documented so clients can name it.
pub const USDC_MINT: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
