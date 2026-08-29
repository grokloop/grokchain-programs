# grok-chain-programs

Spec tree and local Anchor crate for **Grok Chain** intents + paymaster on Solana L1.

This folder is owned by **PROGRAMS**. It is the intent router (`pay`, `swap`, `deploy`, `call`, `pump_*`, `pump_amm_*`) and the per-human spend vault + paymaster. It is not CORE (identity/policy), not the website, not brand, and not lore.

| Path | What it is |
| --- | --- |
| `SPEC.md` | Implementable v1 spec (source of truth for pay / swap / deploy / call) |
| `DEVNET.md` | Devnet rehearsal program id, deploy sig, explorer URLs |
| `programs/grok_chain_intents` | Anchor/Rust crate. `declare_id` in this tree is the live MAINNET id |
| `target/deploy/grok_chain_intents-keypair.json` | Program keypair (gitignored; do not commit) |
| `target/deploy/grok_chain_intents.so` | SBF artifact from `cargo-build-sbf` |

## Program ids

Crate `declare_id!` in this push is the **live MAINNET INTENTS** program:

`3HCErAFs93FMk2J25Qq1xRRMp6B4FyGvif8ZV8hYxQKw`

CORE CPI target (MAINNET): `44fxwzuEyNxZtgDr87mTtMYYJ1LJm6cB5aZNLyBsPjNd`

DEVNET INTENTS `EYhYtqLViS4H3FNt1Q8nGRHGt9oD87uaNsV2WJMNiRkz` still existed as rehearsal (see `DEVNET.md`). DEVNET CORE was `7UtafKBBWNHEXC9PaNXu8USdZqL6VEWupsL7rS6LeVDj`. Those ids are not what this crate declares now.

Retired local placeholder (keypair was missing): `AXprcURLhSqj35v9DJyBkTSPGSoZ9AfTRxYyguQJwnT2`. Do not use it.

Explorer (MAINNET INTENTS): https://explorer.solana.com/address/3HCErAFs93FMk2J25Qq1xRRMp6B4FyGvif8ZV8hYxQKw

Pinned in `Anchor.toml` / `Cargo.toml`: Anchor 0.30.1 (sha256 `global:` / `account:` / `event:` discriminators). `Anchor.toml` `[provider].cluster` is Localnet on purpose so a stray `anchor deploy` does not talk to mainnet.

`blake3 = "=1.5.5"` is pinned so older platform-tools cargo can parse the graph (1.8+ pulls edition2024 `cpufeatures`).

## CORE dependency

Path-depends on CORE (do not copy CORE source, do not edit CORE files):

```
grok_chain_core = { path = "../grok-chain-core/programs/grok_chain_core", features = ["cpi"] }
```

(from this workspace root; the crate `Cargo.toml` uses `../../../grok-chain-core/programs/grok_chain_core`.)

The only CPI into CORE is `check_grant`. Thin invoke, documented metas, hardcoded disc `[223, 172, 131, 140, 15, 133, 209, 250]`. Live MAINNET CORE id is `44fxwzuEyNxZtgDr87mTtMYYJ1LJm6cB5aZNLyBsPjNd`. CORE does not hardcode this router.

## What this crate is

- Sit on **Solana L1**. No new VM. No coin.
- Human root funds a **SpendVault** (pay source) and a **Paymaster** (gas vault).
- Agent signs intents. Agent never holds SOL, never is the fee payer, never deposits/withdraws.
- Grok Chain (Tanmay) does **not** pay every fee. Each human funds their own paymaster. Relayer is the only reimbursed fee payer.
- Promo float is optional / out of v1. Not the model. Not implemented.

## What is implemented

Root-signed: `init_spend_vault`, `fund_spend_vault`, `withdraw_spend_vault`, `init_paymaster`, `fund_paymaster`, `withdraw_paymaster`, `set_relayer`, `pause_paymaster`, `unpause_paymaster`, `init_pump_trader`, `fund_pump_trader`.

Agent-signed, all grant-gated (one `check_grant`, abort on CORE error, optional sponsor):

- `pay` — SpendVault → recipient. Amount must be > 0.
- `swap` — grant-gated SOL send to `out_destination` with `amount_in >= min_out`. Not a DEX. Not Jupiter. Not SPL. Not an AMM.
- `deploy` — `check_grant(0)` + `DeployRequested` event. Not a BPF deploy. Does not upload an ELF.
- `call` — `check_grant(amount)` (0 = policy ping). Optional vault debit. Optional `invoke` of remaining_accounts into an inner program with empty ix data.
- `pump_buy` / `pump_sell` / `pump_create` — official pump.fun bonding curve only (`6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`). Trader PDA is user. Vault is never user.
- `pump_amm_buy` / `pump_amm_sell` — PumpSwap only (`pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA`). Not the curve. Not Jupiter.

`IntentStub` (error 11) is reserved.

## Build

```
cargo-build-sbf --tools-version v1.52 --manifest-path programs/grok_chain_intents/Cargo.toml
```

Keypairs are gitignored. Do not commit `**/*keypair.json` or `target/`.

Read `SPEC.md` before changing Rust.
