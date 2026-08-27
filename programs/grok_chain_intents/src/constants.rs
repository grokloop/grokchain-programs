//! SPEC.md §3. Do not change seed bytes.

/// SpendVault PDA seed. Hex: `73 70 65 6e 64 2d 76 61 75 6c 74`.
pub const SEED_SPEND_VAULT: &[u8] = b"spend-vault";

/// Paymaster PDA seed. Hex: `70 61 79 6d 61 73 74 65 72`.
pub const SEED_PAYMASTER: &[u8] = b"paymaster";

/// Hard per-intent reimbursement cap (0.01 SOL). Arg may be smaller. 0 = no sponsor.
pub const MAX_SPONSOR_LAMPORTS: u64 = 10_000_000;

/// CORE `check_grant` disc: `sha256("global:check_grant")[0..8]`. SPEC.md §5.4 / §13.
pub const CHECK_GRANT_DISCRIMINATOR: [u8; 8] = [223, 172, 131, 140, 15, 133, 209, 250];

/// CORE check_grant metas (order normative). is_writable per account index 0..=3.
pub const CHECK_GRANT_META_WRITABLE: [bool; 4] = [false, true, false, false];

/// CORE check_grant metas (order normative). is_signer per account index 0..=3.
pub const CHECK_GRANT_META_SIGNER: [bool; 4] = [false, false, true, false];
