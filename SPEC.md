# Grok Chain Intents — Paymaster + Pay (v1)

Status: **deployed to Solana devnet only.** Not mainnet. This is the implementable contract for an Anchor/Rust program that lives on **Solana L1**. It is the intent router and the per-human SOL vaults. It is not a VM, not a sequencer, and not a coin.

Crate name (v1): `grok_chain_intents`  
Program module: `grok_chain_intents`  
IDL name: `grok_chain_intents`

```rust
// declare_id is the pubkey of target/deploy/grok_chain_intents-keypair.json.
// DEVNET. Deployed to Solana devnet. Not mainnet. Not a product claim.
declare_id!("EYhYtqLViS4H3FNt1Q8nGRHGt9oD87uaNsV2WJMNiRkz");
```

Anchor instruction/account/event discriminators below assume **Anchor 0.30+** (`sha256("global:<ix>")[0..8]`, `sha256("account:<Name>")[0..8]`, `sha256("event:<Name>")[0..8]`). If the workspace is on a newer Anchor that changes this, recompute and update the tables; do not silently drift.

CORE (identity/policy) is a **different program**. This crate path-depends on it for types and CPIs `check_grant` only. CORE crate: `grok_chain_core`. CORE **devnet** program id: `7UtafKBBWNHEXC9PaNXu8USdZqL6VEWupsL7rS6LeVDj` (live; not mainnet).

---

## 1. Scope and non-goals

**This program does**

- Create one `SpendVault` PDA and one `Paymaster` PDA per CORE `GrokAccount`.
- Let the human root deposit and withdraw SOL on both vaults.
- Let the root set a `relayer` and pause/unpause sponsorship.
- Execute `pay`: CPI CORE `check_grant`, then move SOL from SpendVault to a recipient.
- Optionally reimburse the relayer from the Paymaster in the **same** intent instruction, after a successful `check_grant`, when `grant.sponsor_eligible` and the paymaster is live.
- Execute `swap`: same mouth as `pay`. Grant-gated SOL send to `out_destination` with `amount_in >= min_out`. Not a DEX.
- Execute `deploy`: `check_grant(0)`, emit `DeployRequested`. Not a BPF deploy. No ELF.
- Execute `call`: `check_grant(amount)` (0 = policy ping). Optional vault debit. Optional `invoke` of remaining_accounts into `target_program` with empty ix data.

**This program does not**

- Hold or emit seed phrases, private keys, or wallet backups. None exist in this design.
- Invent a VM, sequencer, coin, stake, or validators.
- Implement official MCP. DEVEX owns the tools; they talk to this program.
- Implement a global promo pot / protocol-funded gas. Promo float is optional, out of v1, and **not the model**.
- Custody user SOL in CORE. CORE `spend_cap_lamports` is a counter. Moving SOL is this program.
- SPL / token transfers (v1 is SOL lamports only).
- A second `check_grant` per intent. One CPI. No standalone `sponsor_reimburse` that skips the grant check.
- Catch CORE errors and continue.

**Sponsored gas model (locked):** the human root funds their own paymaster. The bot/agent never holds SOL. Grok Chain (Tanmay) does not pay every fee. The relayer is the only address that may be reimbursed as the outer fee payer.

One screen: human wallet is root (funds vaults, sets relayer). Agent is a pubkey CORE authorized. Agent signs `pay`. Agent is never the fee payer and never the SOL source.

---

## 2. Account model

| Object | Type | Who signs create | Identity |
| --- | --- | --- | --- |
| Human root | Solana pubkey (wallet) | n/a — it *is* the signer | Same root as CORE `GrokAccount` |
| CORE `GrokAccount` | PDA (CORE program) | Root, via CORE | One per root. We do not create it. |
| CORE `Grant` | PDA (CORE program) | Root, via CORE | One live grant per `(GrokAccount, agent)` |
| Agent | Solana pubkey | Does not sign vault init | Public identity; signs `pay` |
| `SpendVault` | PDA (this program) | Root | One per `GrokAccount`. Pay source. |
| `Paymaster` | PDA (this program) | Root | One per `GrokAccount`. Gas vault. |
| Relayer | Solana pubkey | Set by root | Only address reimbursed as fee payer |

**Human root.** Signs every vault init/fund/withdraw, `set_relayer`, pause/unpause. Never handed to the agent.

**Agent identity.** Signs `pay` / `swap` / `deploy` / `call`. Never deposits, never withdraws, never is the fee payer, never is the SOL source.

**Relayer.** A pubkey the root chooses. Typically a service that submits the tx as the Solana fee payer and is reimbursed from the paymaster. This is **not** Tanmay/Grok Chain paying every fee.

**v1 vault keying (locked): one SpendVault and one Paymaster per `GrokAccount`.** Derived from the CORE account address, not from `root` directly. Same derivation order as CORE grants: `root → GrokAccount → vault`.

---

## 3. Constants (v1)

| Name | Value | Notes |
| --- | --- | --- |
| `SEED_SPEND_VAULT` | `b"spend-vault"` | 11 bytes |
| `SEED_PAYMASTER` | `b"paymaster"` | 9 bytes |
| `MAX_SPONSOR_LAMPORTS` | `10_000_000` | 0.01 SOL hard cap per intent |
| `SPEND_ASSET` | SOL lamports (`u64`) | No SPL in v1 |
| `CHECK_GRANT_DISCRIMINATOR` | `[223, 172, 131, 140, 15, 133, 209, 250]` | CORE `sha256("global:check_grant")[0..8]` |

```rust
pub const SEED_SPEND_VAULT: &[u8] = b"spend-vault";
pub const SEED_PAYMASTER: &[u8] = b"paymaster";
pub const MAX_SPONSOR_LAMPORTS: u64 = 10_000_000;
pub const CHECK_GRANT_DISCRIMINATOR: [u8; 8] = [223, 172, 131, 140, 15, 133, 209, 250];
```

CORE seeds (do not re-derive; CORE SPEC.md §3 / §4):

```text
SEED_GROK_ACCOUNT = b"grok-account"   // CORE program_id
SEED_GRANT        = b"grant"          // CORE program_id; seeds [grant, grok_account, agent] — NOT root
```

---

## 4. PDA seed formulas

Write these as bytes. Do not hash them yourself; pass them to `Pubkey::find_program_address`.

### 4.1 `SpendVault`

```text
seeds = [
  b"spend-vault",                 // [115, 112, 101, 110, 100, 45, 118, 97, 117, 108, 116]
  grok_account.key().as_ref(),    // 32 bytes — the CORE GrokAccount PDA, not the root
]
program_id = grok_chain_intents
```

Hex of the literal seed: `73 70 65 6e 64 2d 76 61 75 6c 74`.

Bump is stored in `SpendVault.bump`.

### 4.2 `Paymaster`

```text
seeds = [
  b"paymaster",                   // [112, 97, 121, 109, 97, 115, 116, 101, 114]
  grok_account.key().as_ref(),    // 32 bytes — the CORE GrokAccount PDA, not the root
]
program_id = grok_chain_intents
```

Hex of the literal seed: `70 61 79 6d 61 73 74 65 72`.

Bump is stored in `Paymaster.bump`.

### 4.3 CORE PDAs (read / CPI only)

```text
(grok_account, _) = find([b"grok-account", root], grok_chain_core)
(grant, _)        = find([b"grant", grok_account, agent], grok_chain_core)
```

Clients and this program must **not** seed the CORE grant with `root` directly.

### 4.4 Client helpers

```rust
pub fn spend_vault_pda(program_id: &Pubkey, grok_account: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"spend-vault", grok_account.as_ref()], program_id)
}

pub fn paymaster_pda(program_id: &Pubkey, grok_account: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"paymaster", grok_account.as_ref()], program_id)
}
```

---

## 5. Exact Borsh / Anchor layouts

Serialization: Borsh, little-endian, field order = struct order.  
Every `#[account]` type is prefixed by the 8-byte Anchor account discriminator.

Do not insert hidden alignment padding between fields.

SOL custody is the **lamports on the PDA itself**. No shadow balance field.

These PDAs are **program-owned** (they store fields). `system_instruction::transfer` cannot debit a non-system owner. Debits use direct lamport movement (the PDA-signed equivalent). Credits **into** a vault from root use system transfer (root is system-owned).

### 5.1 Account discriminators

| Type | `sha256("account:<Name>")[0..8]` | bytes |
| --- | --- | --- |
| `SpendVault` | `0x4ba6fd4ceb39865d` | `75, 166, 253, 76, 235, 57, 134, 93` |
| `Paymaster` | `0x4f837b604b25836a` | `79, 131, 123, 96, 75, 37, 131, 106` |

### 5.2 `SpendVault`

```rust
#[account]
pub struct SpendVault {
    pub grok_account: Pubkey, // 32
    pub root: Pubkey,         // 32  denormalized; must match GrokAccount.root
    pub bump: u8,             //  1
}
```

**Space:** `8 + 32 + 32 + 1 = 73`

```rust
impl SpendVault {
    pub const SPACE: usize = 8 + 32 + 32 + 1; // 73
}
```

### 5.3 `Paymaster`

```rust
#[account]
pub struct Paymaster {
    pub grok_account: Pubkey, // 32
    pub root: Pubkey,         // 32
    pub relayer: Pubkey,      // 32  only address reimbursed as fee payer
    pub bump: u8,             //  1
    pub paused: bool,         //  1  root can pause sponsorship
}
```

**Space:** `8 + 32 + 32 + 32 + 1 + 1 = 106`

```rust
impl Paymaster {
    pub const SPACE: usize = 8 + 32 + 32 + 32 + 1 + 1; // 106
}
```

### 5.4 Instruction discriminators and data

| Instruction | `sha256("global:<name>")[0..8]` | bytes |
| --- | --- | --- |
| `init_spend_vault` | `0xf1ad07b3787cd53d` | `241, 173, 7, 179, 120, 124, 213, 61` |
| `fund_spend_vault` | `0x69b216714058c9e9` | `105, 178, 22, 113, 64, 88, 201, 233` |
| `withdraw_spend_vault` | `0x29eb9896817ae025` | `41, 235, 152, 150, 129, 122, 224, 37` |
| `init_paymaster` | `0x173efc28b2467236` | `23, 62, 252, 40, 178, 70, 114, 54` |
| `fund_paymaster` | `0x544388aaa8a3dc67` | `84, 67, 136, 170, 168, 163, 220, 103` |
| `withdraw_paymaster` | `0x363cc5e222b395bd` | `54, 60, 197, 226, 34, 179, 149, 189` |
| `set_relayer` | `0x17f321586e54c425` | `23, 243, 33, 88, 110, 84, 196, 37` |
| `pause_paymaster` | `0x611a98ad3b94f44d` | `97, 26, 152, 173, 59, 148, 244, 77` |
| `unpause_paymaster` | `0x8ff8d3d8627131fb` | `143, 248, 211, 216, 98, 113, 49, 251` |
| `pay` | `0x7712d841c0757adc` | `119, 18, 216, 65, 192, 117, 122, 220` |
| `swap` | `0xf8c69e91e17587c8` | `248, 198, 158, 145, 225, 117, 135, 200` |
| `deploy` | `0x43248f7624a45cd9` | `67, 36, 143, 118, 36, 164, 92, 217` |
| `call` | `0xb55e38a1c2ddc803` | `181, 94, 56, 161, 194, 221, 200, 3` |

#### Root vault ixs

`init_spend_vault` / `pause_paymaster` / `unpause_paymaster`: data = discriminator only.

`fund_spend_vault` / `withdraw_spend_vault` / `fund_paymaster` / `withdraw_paymaster`: disc + Borsh `u64 lamports`.

`init_paymaster` / `set_relayer`: disc + Borsh `Pubkey relayer` (32 bytes).

#### `pay` args

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PayArgs {
    pub amount_lamports: u64,   // 8  must be > 0
    pub sponsor_lamports: u64,  // 8  0 = no reimbursement this tx
}
```

Data size: `8 + 8 + 8 = 24`.

#### `swap` args

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SwapArgs {
    pub amount_in_lamports: u64,  // 8  must be > 0
    pub min_out_lamports: u64,    // 8  require amount_in >= min_out
    pub sponsor_lamports: u64,    // 8  0 = no reimbursement this tx
}
```

Data size: `8 + 8 + 8 + 8 = 32`.

v1 swap is SOL-only. `amount_in` is debited from SpendVault to `out_destination`. `amount_in >= min_out` is the honest min check — not an AMM quote. A real AMM is later.

#### `deploy` args

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DeployArgs {
    pub sponsor_lamports: u64,  // 8  0 ok
    pub program_id: Pubkey,     // 32 recorded in DeployRequested
}
```

Data size: `8 + 8 + 32 = 48`.

v1 deploy is a grant-gated **request**. `check_grant(0)`. No `bpf_loader`. No ELF upload. `program_id` is not deployed.

#### `call` args

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CallArgs {
    pub amount_lamports: u64,   // 8  0 = policy ping (no vault debit)
    pub sponsor_lamports: u64,  // 8
    pub target_program: Pubkey, // 32 inner program for remaining_accounts invoke
}
```

Data size: `8 + 8 + 8 + 32 = 56`.

CORE `check_grant` still uses **this INTENTS router** as `target_program`. `args.target_program` is the inner program only.

---

## 6. Instructions

Clock is not used by this program in v1 (expiry lives in CORE).

Shared root constraint on every vault ix:

```text
root.key() == grok_account.root
root.key() == vault.root          // after init
root is Signer
```

Agent never appears as a signer on vault ixs.

### 6.1 `init_spend_vault`

Creates the SpendVault PDA. One per GrokAccount. Fails if already initialized. Root pays rent.

**Accounts**

| Account | Mut | Signer | Type | Constraints |
| --- | --- | --- | --- | --- |
| `root` | yes (payer) | yes | `Signer` | `== grok_account.root` |
| `grok_account` | no | no | `Account<GrokAccount>` (CORE) | seeds `[b"grok-account", grok_account.root]`, `seeds::program = grok_chain_core` |
| `spend_vault` | yes | no | `Account<SpendVault>` | `init`, seeds `[b"spend-vault", grok_account.key()]`, `space = 73`, `payer = root` |
| `system_program` | no | no | `Program<System>` | |

**Args:** none.

**Write:** `grok_account`, `root`, `bump`. Emit `SpendVaultInitialized`.

### 6.2 `fund_spend_vault`

Root system-transfers SOL onto the SpendVault PDA.

**Accounts:** `root` (mut signer), `grok_account` (CORE PDA), `spend_vault` (mut, seeds + root match), `system_program`.

**Args:** `lamports: u64`. Must be `> 0` else `ZeroAmount`.

**Emit:** `SpendVaultFunded`.

### 6.3 `withdraw_spend_vault`

Root withdraws SOL from SpendVault to **root**. Agent cannot call this.

**Accounts:** `root` (mut signer), `grok_account`, `spend_vault` (mut).

**Args:** `lamports: u64` `> 0`.

**Checks:** after debit, `spend_vault.lamports >= Rent::minimum_balance(73)` else `InsufficientSpendVault`.

**Emit:** `SpendVaultWithdrawn`.

### 6.4 `init_paymaster`

Creates the Paymaster PDA. Root pays rent. Sets `relayer`. `paused = false`.

**Accounts:** same shape as init_spend_vault, with `paymaster` seeds `[b"paymaster", grok_account.key()]`, `space = 106`.

**Args:** `relayer: Pubkey`.

**Emit:** `PaymasterInitialized`.

### 6.5 `fund_paymaster` / `withdraw_paymaster`

Same rules as the spend-vault fund/withdraw pair. Withdraw goes to **root**. Insufficient → `InsufficientPaymaster`. Rent floor is `minimum_balance(106)`.

### 6.6 `set_relayer`

Root replaces `paymaster.relayer`. Agent cannot. Relayer cannot.

**Args:** `relayer: Pubkey`.

**Emit:** `RelayerSet`.

### 6.7 `pause_paymaster` / `unpause_paymaster`

Root sets `paused = true` / `false`. While paused, `pay` with `sponsor_lamports > 0` fails `PaymasterPaused`. Unsponsored `pay` (`sponsor_lamports == 0`) still works.

**Emit:** `PaymasterPaused` / `PaymasterUnpaused`.

### 6.8 `pay` — IMPLEMENTED

Agent-signed intent. Moves SOL from SpendVault to `recipient`. Optionally reimburses the relayer from Paymaster.

**Args:** `PayArgs { amount_lamports, sponsor_lamports }`

**Accounts**

| # | Account | Mut | Signer | Type | Constraints |
| --- | --- | --- | --- | --- | --- |
| 0 | `agent` | no | yes | `Signer` | `== grant.agent` |
| 1 | `grok_account` | no | no | CORE `GrokAccount` | CORE seeds + bump, `seeds::program = grok_chain_core` |
| 2 | `grant` | **yes** | no | CORE `Grant` | CORE seeds `[b"grant", grok_account, agent]`; CPI needs write |
| 3 | `grok_chain_core_program` | no | no | `Program<GrokChainCore>` | `== grok_chain_core::ID` |
| 4 | `intents_program` | no | no | this program | `address = crate::ID` (CORE `target_program`) |
| 5 | `spend_vault` | yes | no | `Account<SpendVault>` | seeds; `grok_account` + `root` match |
| 6 | `recipient` | yes | no | `UncheckedAccount` | credit only |
| 7 | `system_program` | no | no | `Program<System>` | |
| 8 | `paymaster` | yes | no | `Option<Account<Paymaster>>` | required iff `sponsor_lamports > 0` |
| 9 | `fee_payer` | yes | yes if present | `Option<Signer>` | must equal `paymaster.relayer` when sponsoring |

Optional accounts: pass this program id as a dummy when `sponsor_lamports == 0` (Anchor `Option`).

**Handler, in this order (stable for tests)**

1. `amount_lamports > 0` else `ZeroPayAmount`. (`pay` must move SOL. `amount == 0` is valid on CORE `check_grant` for call/deploy — not for `pay`.)
2. If `sponsor_lamports > 0`:
   - `sponsor_lamports <= MAX_SPONSOR_LAMPORTS` else `SponsorCapExceeded`
   - `paymaster` and `fee_payer` are `Some` else `SponsorAccountsRequired`
3. **CPI `check_grant(amount_lamports)`** with `target_program = crate::ID`. Account order and disc: §9. On error: **abort**. Do not catch-and-continue. One CPI. `amount_lamports` is the SOL this intent spends under the grant cap (not a CORE vault debit).
4. Debit `amount_lamports` from SpendVault, credit `recipient`. Leave rent-exempt minimum (`SPACE = 73`). Else `InsufficientSpendVault`.
5. If `sponsor_lamports > 0` (sponsor path requested):
   - require `grant.sponsor_eligible == true` (read grant; still must have done the CPI) else `NotSponsorEligible`
   - require `paymaster.paused == false` else `PaymasterPaused`
   - require `fee_payer.key() == paymaster.relayer` else `RelayerMismatch`
   - debit `sponsor_lamports` from Paymaster, credit `fee_payer`. Leave rent-exempt minimum (`SPACE = 106`). Else `InsufficientPaymaster`.
   - emit `Sponsored`
6. If `sponsor_lamports == 0` (and/or no paymaster accounts): skip reimbursement. Agent/human/relayer pays the Solana fee as usual — this program does not touch the paymaster.
7. Emit `Paid`.

Reads you may do without CPI: `grant.sponsor_eligible`, remaining cap, expiry, generation. **Reads are not authoritative; the CPI is.**

**Fail closed.** Short vault, paused paymaster, ineligible grant, wrong relayer, CORE reject — the whole tx fails. No partial pay.

### 6.9 `swap` — IMPLEMENTED (honest SOL send, not a DEX)

Same security model as `pay`. Agent-signed. Relayer fee-pays. Bot never holds SOL. One `check_grant`. Abort on CORE error.

**Args:** `SwapArgs { amount_in_lamports, min_out_lamports, sponsor_lamports }`

**Accounts:** same shape as `pay`, with `out_destination` in place of `recipient`.

**Handler, in this order**

1. `amount_in_lamports > 0` else `ZeroAmount`. (`swap` must move SOL.)
2. `amount_in_lamports >= min_out_lamports` else `MinOutNotMet`. Honest: this is a min check on a SOL send, not an AMM quote. SPEC forbids SPL in v1.
3. If `sponsor_lamports > 0`: same prechecks as `pay` (cap, paymaster + fee_payer present).
4. **CPI `check_grant(amount_in_lamports)`** with `target_program = crate::ID`. On error: **abort**. One CPI.
5. Debit `amount_in_lamports` from SpendVault, credit `out_destination`. Leave rent-exempt minimum. Else `InsufficientSpendVault`.
6. Optional sponsor path, same as `pay`.
7. Emit `Swapped { amount_in, destination, min_out, … }`.

Do **not** call Jupiter. Do **not** invent a pool. A real AMM is later.

### 6.10 `deploy` — IMPLEMENTED (grant-gated request, not a BPF deploy)

Agent-signed. `amount_lamports == 0` for `check_grant` (call/deploy path). Optional sponsor.

**Args:** `DeployArgs { sponsor_lamports, program_id }`

**Accounts**

| # | Account | Mut | Signer | Type | Constraints |
| --- | --- | --- | --- | --- | --- |
| 0 | `agent` | no | yes | `Signer` | `== grant.agent` |
| 1 | `grok_account` | no | no | CORE `GrokAccount` | CORE seeds |
| 2 | `grant` | **yes** | no | CORE `Grant` | CORE seeds; CPI needs write |
| 3 | `grok_chain_core_program` | no | no | `Program<GrokChainCore>` | |
| 4 | `intents_program` | no | no | this program | `address = crate::ID` |
| 5 | `spend_vault` | no | no | `Account<SpendVault>` | present; **never debited** |
| 6 | `system_program` | no | no | `Program<System>` | |
| 7 | `paymaster` | yes | no | `Option<Account<Paymaster>>` | required iff `sponsor_lamports > 0` |
| 8 | `fee_payer` | yes | yes if present | `Option<Signer>` | must equal `paymaster.relayer` when sponsoring |

**Handler**

1. Sponsor prechecks if `sponsor_lamports > 0`.
2. **CPI `check_grant(0)`** with `target_program = crate::ID`. Abort on CORE error. One CPI.
3. Do **not** debit SpendVault. Do **not** invoke `bpf_loader` / `bpf_loader_upgradeable`. Do **not** upload ELF. Do **not** pretend a program was deployed.
4. remaining_accounts are ignored (default is event + grant check only). A later version may CPI a harmless allowlisted init.
5. Optional sponsor path, same as `pay`.
6. Emit `DeployRequested { program_id, agent, grant, generation }`.

### 6.11 `call` — IMPLEMENTED (grant-gated router)

Agent-signed. Same CORE + vault + optional sponsor accounts as `pay`, plus `call_target`.

**Args:** `CallArgs { amount_lamports, sponsor_lamports, target_program }`

`amount_lamports == 0` is valid (policy ping). `amount_lamports > 0` debits SpendVault to `recipient`.

**Accounts:** `pay` list, plus `call_target` (unchecked, must equal `args.target_program`, must be executable) inserted before the optional paymaster / fee_payer.

**Handler**

1. `call_target.key() == args.target_program` else `CallTargetMismatch`. `call_target` executable else `TargetNotExecutable`.
2. Sponsor prechecks if `sponsor_lamports > 0`.
3. remaining_accounts must not include `spend_vault` or `paymaster` else `ProtectedAccountInRemaining`.
4. **CPI `check_grant(amount_lamports)`** with `target_program = crate::ID` (the router — **not** `args.target_program`). Never skip. Abort on CORE error. One CPI.
5. If `amount_lamports > 0`: debit SpendVault → `recipient`. If `0`: no vault debit.
6. If remaining_accounts is **empty**: the call is grant-checked only (policy ping) and must still succeed after `check_grant`.
7. If remaining_accounts is **non-empty**: `invoke` (never `invoke_signed`) into `args.target_program` with those accounts and **empty** ix data. Never sign as the vault PDA.
8. Optional sponsor path, same as `pay`.
9. Emit `Called`.

Router-mode honesty: the human allowlists **INTENTS**, not the inner program.

---

## 7. Events

Anchor events. SURFACE indexes these. **Never** put key material, seeds, or keypair paths in events. Agent / root / relayer pubkeys are public identities.

Event discriminator = `sha256("event:<Name>")[0..8]`.

| Event | disc hex | bytes |
| --- | --- | --- |
| `SpendVaultInitialized` | `0xf648e7ac9e82fc66` | `246, 72, 231, 172, 158, 130, 252, 102` |
| `SpendVaultFunded` | `0x8f695c1eac2ec182` | `143, 105, 92, 30, 172, 46, 193, 130` |
| `SpendVaultWithdrawn` | `0xc525c84a77d3cf2e` | `197, 37, 200, 74, 119, 211, 207, 46` |
| `PaymasterInitialized` | `0xe7641d161d435f4b` | `231, 100, 29, 22, 29, 67, 95, 75` |
| `PaymasterFunded` | `0x52b4d0724089d5fe` | `82, 180, 208, 114, 64, 137, 213, 254` |
| `PaymasterWithdrawn` | `0xcc356d49e92e61c2` | `204, 53, 109, 73, 233, 46, 97, 194` |
| `RelayerSet` | `0xd70115a08aa94628` | `215, 1, 21, 160, 138, 169, 70, 40` |
| `PaymasterPaused` | `0xde5d34b961c40b2f` | `222, 93, 52, 185, 97, 196, 11, 47` |
| `PaymasterUnpaused` | `0x33b0efa57d624753` | `51, 176, 239, 165, 125, 98, 71, 83` |
| `Paid` | `0xf0c111eeeed281eb` | `240, 193, 17, 238, 238, 210, 129, 235` |
| `Sponsored` | `0x0d35280da5505583` | `13, 53, 40, 13, 165, 80, 85, 131` |
| `Swapped` | `0xd93434539387606d` | `217, 52, 52, 83, 147, 135, 96, 109` |
| `DeployRequested` | `0xec5d99b4d37043fc` | `236, 93, 153, 180, 211, 112, 67, 252` |
| `Called` | `0x1e61fe953c26ff05` | `30, 97, 254, 149, 60, 38, 255, 5` |

```rust
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

#[event]
pub struct DeployRequested {
    pub program_id: Pubkey,
    pub agent: Pubkey,
    pub grant: Pubkey,
    pub generation: u32,
}

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
```

Do not emit bumps. Do not emit seed phrases. Do not emit keypair paths.

---

## 8. Error codes

`#[error_code]` enum. **Numbers are stable.** Do not reorder. Force the discriminant.

```rust
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
```

| Code | Name | When |
| --- | --- | --- |
| 0 | `UnauthorizedRoot` | wrong root signer or vault.root mismatch |
| 1 | `AgentMismatch` | `pay` signer ≠ `grant.agent` |
| 2 | `ZeroPayAmount` | `pay` with `amount_lamports == 0` |
| 3 | `ZeroAmount` | fund/withdraw with `lamports == 0` |
| 4 | `InsufficientSpendVault` | pay/withdraw would breach rent floor |
| 5 | `InsufficientPaymaster` | sponsor/withdraw would breach rent floor |
| 6 | `PaymasterPaused` | sponsor requested while paused |
| 7 | `RelayerMismatch` | `fee_payer ≠ paymaster.relayer` |
| 8 | `NotSponsorEligible` | sponsor requested but grant flag is false |
| 9 | `SponsorAccountsRequired` | `sponsor_lamports > 0` without paymaster + fee_payer |
| 10 | `SponsorCapExceeded` | `sponsor_lamports > MAX_SPONSOR_LAMPORTS` |
| 11 | `IntentStub` | reserved; unused by swap/deploy/call |
| 12 | `GrokAccountMismatch` | vault/grant parent mismatch |
| 13 | `LamportOverflow` | `u64` overflow on credit |
| 14 | `InvalidCoreProgram` | CPI program id ≠ `grok_chain_core::ID` |
| 15 | `MinOutNotMet` | `swap` `amount_in < min_out` |
| 16 | `CallTargetMismatch` | `call_target` ≠ `args.target_program` |
| 17 | `TargetNotExecutable` | `call_target` is not executable |
| 18 | `ProtectedAccountInRemaining` | remaining_accounts includes spend_vault or paymaster |

Next free code: **19**. Add only at the end. Do not reuse 11.

CORE errors (`GrantRevoked`, `GrantExpired`, `GrantCapExceeded`, `GrantProgramDenied`, …) surface from the CPI as CORE custom errors. Abort. Do not remap.

---

## 9. CORE `check_grant` CPI (normative)

This is the **only** CPI. Keep the surface small.

**When:** before the pay body, in the same instruction / transaction.

**What you call:** `check_grant(amount_lamports)` on `grok_chain_core`.

**How you derive accounts**

```text
program_id        = grok_chain_core   // 7UtafKBBWNHEXC9PaNXu8USdZqL6VEWupsL7rS6LeVDj on devnet
(grok_account, _) = find([b"grok-account", root], program_id)
(grant, _)        = find([b"grant", grok_account, agent], program_id)
target_program    = *this* program id   // v1 router mode: the intent router
```

**CPI account metas (order is NORMATIVE)**

| # | pubkey | is_mut | is_signer | notes |
| --- | --- | --- | --- | --- |
| 0 | `grok_account` | false | false | CORE PDA |
| 1 | `grant` | **true** | false | CORE PDA; spent increments |
| 2 | `agent` | false | **true** | must sign the outer tx |
| 3 | `target_program` | false | false | `crate::ID`, executable |

`program_id` of the CPI is `grok_chain_core`.

**Instruction data:** `8-byte disc check_grant` + Borsh `u64 amount_lamports`.

Disc bytes (hardcoded, do not recompute from a different string):

```text
[223, 172, 131, 140, 15, 133, 209, 250]
```

**`amount_lamports`:** SOL this intent spends **under this grant's cap**. This is not a CORE vault debit. CORE cap is a counter.

**On error:** abort the intent. Do not catch-and-continue.

**Idempotency:** one `check_grant` per intent execution. Two CPIs in one tx increment twice. Do not add a second check for sponsor.

**No remaining-accounts** on the CPI.

Implementation: thin `invoke` with the metas above (see `core_cpi.rs`). Path-depend on CORE for `Grant` / `GrokAccount` types and `ID`. Do not copy CORE program source.

---

## 10. Instruction account metas (wire)

`is_signer` / `is_writable` as the runtime sees them.

### `init_spend_vault` / `init_paymaster`

| # | name | writable | signer |
| --- | --- | --- | --- |
| 0 | `root` | true | true |
| 1 | `grok_account` | false | false |
| 2 | `spend_vault` or `paymaster` | true | false |
| 3 | `system_program` | false | false |

### `fund_spend_vault` / `fund_paymaster`

| # | name | writable | signer |
| --- | --- | --- | --- |
| 0 | `root` | true | true |
| 1 | `grok_account` | false | false |
| 2 | vault | true | false |
| 3 | `system_program` | false | false |

### `withdraw_spend_vault` / `withdraw_paymaster`

| # | name | writable | signer |
| --- | --- | --- | --- |
| 0 | `root` | true | true |
| 1 | `grok_account` | false | false |
| 2 | vault | true | false |

### `set_relayer` / `pause_paymaster` / `unpause_paymaster`

| # | name | writable | signer |
| --- | --- | --- | --- |
| 0 | `root` | false | true |
| 1 | `grok_account` | false | false |
| 2 | `paymaster` | true | false |

### `pay`

| # | name | writable | signer |
| --- | --- | --- | --- |
| 0 | `agent` | false | true |
| 1 | `grok_account` | false | false |
| 2 | `grant` | true | false |
| 3 | `grok_chain_core_program` | false | false |
| 4 | `intents_program` | false | false |
| 5 | `spend_vault` | true | false |
| 6 | `recipient` | true | false |
| 7 | `system_program` | false | false |
| 8 | `paymaster` | true | false | optional |
| 9 | `fee_payer` | true | true | optional |

### `swap`

Same as `pay`, with `out_destination` in place of `recipient`.

### `deploy`

| # | name | writable | signer |
| --- | --- | --- | --- |
| 0 | `agent` | false | true |
| 1 | `grok_account` | false | false |
| 2 | `grant` | true | false |
| 3 | `grok_chain_core_program` | false | false |
| 4 | `intents_program` | false | false |
| 5 | `spend_vault` | false | false |
| 6 | `system_program` | false | false |
| 7 | `paymaster` | true | false | optional |
| 8 | `fee_payer` | true | true | optional |

### `call`

| # | name | writable | signer |
| --- | --- | --- | --- |
| 0 | `agent` | false | true |
| 1 | `grok_account` | false | false |
| 2 | `grant` | true | false |
| 3 | `grok_chain_core_program` | false | false |
| 4 | `intents_program` | false | false |
| 5 | `spend_vault` | true | false |
| 6 | `recipient` | true | false |
| 7 | `system_program` | false | false |
| 8 | `call_target` | false | false |
| 9 | `paymaster` | true | false | optional |
| 10 | `fee_payer` | true | true | optional |

---

## 11. Security notes

1. **No key material.** Logs, events, account data, and errors contain only public keys and integers. Do not add seed phrases, keypair JSON, or backup fields.

2. **Agent cannot withdraw vaults.** Only root. Agent cannot `set_relayer` or pause.

3. **Relayer cannot drain the paymaster** except the explicit `sponsor_lamports` inside a `pay` that already passed `check_grant` (and `sponsor_eligible`, not paused, relayer match, under `MAX_SPONSOR_LAMPORTS`). No standalone reimburse.

4. **Spend cap is CORE's.** Also enforce the vault has funds and a rent-exempt floor.

5. **Sponsor is not a grant debit.** `check_grant(amount_lamports)` uses the pay amount only. Paymaster reimbursement is a separate human-funded pot.

6. **Router-mode allowlist.** The human allowlists **this program id** on the CORE grant, not every inner program. CORE `target_program` is `crate::ID`.

7. **Fee payer binding.** When sponsoring, `fee_payer` is a Signer and must equal `paymaster.relayer`. The relayer opted into the tx (they submitted it).

8. **PDA spoofing.** Always constrain seeds + stored bump + parent `grok_account` + `root`. CORE accounts use `seeds::program = grok_chain_core::ID`.

9. **Program-owned lamports.** Debit via direct lamport movement. Do not `system_instruction::transfer` from a program-owned PDA (it will fail).

10. **Promo float.** Out of v1. Do not add a global pot. Do not have Grok Chain pay every fee.

11. **No SPL, no new token, no VM, no sequencer.**

---

## 12. v1 vs later

| In this program (v1) | Not in this program |
| --- | --- |
| SpendVault + Paymaster on **Solana L1** | Promo float / protocol gas pot |
| `pay` / `swap` / `deploy` / `call` with CORE `check_grant` | Jupiter / AMM / SPL / BPF deploy |
| Per-human relayer reimbursement | Tanmay-pays-all fee model |
| SOL lamports only | SPL / token spends |
| Router-mode allowlist (this program id) | Per-inner-program tracing |
| Events for SURFACE | MCP implementation (DEVEX) |
| localnet | Mainnet status, advertised program id |

Later, non-breaking additions (new instructions, new optional accounts at the **end** of metas, new error codes ≥ 15) are allowed. Changing seed strings, field order, space, or error numbers is a breaking change and a new major.

---

## 13. For CORE

You already shipped `check_grant` and `sponsor_eligible`. We do not edit your crate.

- Human allowlists **this program id** (`EYhYtqLViS4H3FNt1Q8nGRHGt9oD87uaNsV2WJMNiRkz` on devnet) in `grant.allowed_programs` — **router mode**.
- We CPI `check_grant` once per `pay` / `swap` / `deploy` / `call`, metas §9, disc hardcoded. Abort on your error. `deploy` and amount-0 `call` pass `amount_lamports = 0`.
- We read `grant.sponsor_eligible` only to gate reimbursement. Reads are not authoritative; the CPI is.
- We do not ask you to move SOL. We do not pass remaining accounts to `check_grant`.
- `amount_lamports` on the CPI is the intent spend (SpendVault debit for pay/swap/call>0; 0 for deploy and call policy ping), not the sponsor reimbursement.
- Grant PDA seeds stay `[b"grant", grok_account, agent]` — we never seed with `root`.

---

## 14. For DEVEX

Official MCP tools `pay` / `swap` / `deploy` / `call` talk to **this program**. This program talks to CORE `check_grant`.

**Human wallet (root)** signs: CORE `create_account` / `issue_grant` / `revise_grant` / `revoke_grant`, plus this program's vault init/fund/withdraw, `set_relayer`, pause/unpause. Use a normal wallet adapter. Never export a seed into the agent runtime.

**Human funds** the SpendVault (pay source) and the Paymaster (gas). Two deposits. This is the model.

**Agent keypair:** created by the agent process. Secret stays in the agent process. Agent signs `pay`. Agent **never holds SOL**, never is the fee payer, never is the SOL source.

**Relayer:** root sets it. Attach the relayer as the **outer tx fee payer only when** `grant.sponsor_eligible == true` and the paymaster is not paused. Pass `sponsor_lamports` (≤ `MAX_SPONSOR_LAMPORTS`) and the paymaster + fee_payer accounts. If the flag is false, or `sponsor_lamports == 0`, the submitter pays the Solana fee as usual.

**What to show the human**

- this program id (router; what they allowlist on the grant)
- spend vault balance (lamports on the PDA minus rent)
- paymaster balance, relayer pubkey, paused
- `grant.sponsor_eligible` as "this grant may use your paymaster" — not a promise Grok Chain pays
- `amount_lamports` and `sponsor_lamports` on each pay

**MCP tools:** `pay` / `swap` / `deploy` / `call` submit the real INTENTS ixs. On localnet they land only if the local validator is running this binary. This source was not upgraded on grokchain-devnet in the swap/deploy/call change — do not claim those ixs are live on public Solana until an explicit upgrade. v1 swap is a grant-gated SOL send; v1 deploy is a request event; v1 call is a grant-gated router (empty remaining = policy ping).

**Do not** encode policy in MCP-only state. If it is not in the CORE `Grant` or these vault PDAs, it is not enforced.

**PDA helpers:** §4.4. Derive CORE PDAs with CORE's program id; derive vaults with this program id.

---

## 15. Suggested Anchor `#[derive(Accounts)]` (normative shapes)

See `programs/grok_chain_intents/src/instructions/*.rs`. Vault inits use `init` + seeds. `pay` uses CORE `Account<GrokAccount>` / `Account<Grant>` with `seeds::program = grok_chain_core::ID`, then `core_cpi::check_grant`.

---

## 16. Done when (first Anchor implementation)

- [x] Crate `grok_chain_intents` with Anchor 0.30.1. `declare_id!` is a generated local keypair — not advertised as live.
- [x] `SpendVault::SPACE == 73`, `Paymaster::SPACE == 106`, field order matches §5.
- [x] PDA seeds match §4 (`spend-vault` + grok_account; `paymaster` + grok_account).
- [x] Root-only init/fund/withdraw on both vaults. Agent cannot.
- [x] `set_relayer`, `pause_paymaster`, `unpause_paymaster` — root only.
- [x] `pay` — agent signer; CPI `check_grant`; vault → recipient; optional sponsor; event `Paid`.
- [x] `pay` amount 0 fails. Sponsor path checks `sponsor_eligible`, pause, relayer, cap, vault floor.
- [x] `swap` / `deploy` / `call` are real grant-gated handlers (not IntentStub). IntentStub (11) reserved.
- [x] No test, log, or event prints a seed phrase, private key, or backup file.
- [x] Spec-lock unit tests: spaces, seeds, error 0..=14, discriminator hashes, CORE CPI metas/disc.
- [ ] Validator / TS tests (no local validator on this box).
- [x] `cargo-build-sbf --tools-version v1.52` produced `target/deploy/grok_chain_intents.so`. Deployed to **devnet** (see DEVNET.md). Not mainnet.

That is v1. Stop there.

---

## 17. Test vectors (minimal)

Use any root/agent/relayer keypairs. These are **not** wallets to import; they are unit fixtures.

1. `pay` with `amount_lamports == 0` → `ZeroPayAmount` (no CPI).
2. `pay` with `sponsor_lamports > MAX_SPONSOR_LAMPORTS` → `SponsorCapExceeded`.
3. `pay` with `sponsor_lamports > 0` and dummy optional accounts → `SponsorAccountsRequired`.
4. `pay` after successful `check_grant` with empty spend vault → `InsufficientSpendVault` (whole tx fails; CORE spent still increments only if the tx lands — it will not).
5. `pay` sponsor when `grant.sponsor_eligible == false` → `NotSponsorEligible`.
6. `pay` sponsor when `paymaster.paused` → `PaymasterPaused`.
7. `pay` sponsor when `fee_payer ≠ relayer` → `RelayerMismatch`.
8. `swap` amount 0 → `ZeroAmount`. `swap` `amount_in < min_out` → `MinOutNotMet`. Happy path moves SOL to `out_destination`. Agent lamports stay 0. Missing grant → CORE abort.
8b. `deploy` `check_grant(0)` succeeds; SpendVault unchanged; no ELF / no bpf_loader.
8c. `call` amount 0 + empty remaining → policy ping success. `call` amount > 0 moves SOL. `check_grant` required.
9. non-root cannot fund / withdraw / set_relayer / pause.
10. agent cannot withdraw either vault.

Clock: none in this program. CORE expiry is CORE's problem (`check_grant` fails `GrantExpired`).

---

*PROGRAMS · Grok Chain intents + paymaster · v1 spec. Implementation follows this file; this file does not follow the implementation.*
