# grok-chain-programs

**The on-chain half of letting an AI agent spend money without holding a wallet.**

This is the Solana program an agent's payments actually run through. A human
funds a vault and issues a capability grant — a cap, an expiry, an allowlist of
payees. The agent signs intents against that grant. The program enforces the
limits; nothing is trusted to the client.

Live on Solana mainnet as `3HCErAFs93FMk2J25Qq1xRRMp6B4FyGvif8ZV8hYxQKw`.
If you just want to *use* this, you want [grokchain-mcp](https://github.com/grokloop/grokchain-mcp)
instead — this repo is the program source.

## What it enforces

| Guarantee | How |
| --- | --- |
| The agent cannot overspend | `check_grant` meters every debit against the cap |
| The agent cannot pay strangers | `pay_token` checks a root-owned merchant allowlist |
| A subscription cannot double-charge | `last_paid_period` advances in the same tx that pays |
| A swap cannot be sandwiched into a bad fill | balances are snapshotted around the CPI, `min_out` enforced after |
| The agent cannot pay its own gas | it is never the fee payer; the relayer is reimbursed from a paymaster |
| A human can stop it instantly | `revise_grant` to `cap = spent` blocks buys but still permits sells |

That last row matters more than it looks: `revoke_grant` also blocks sells,
which strands whatever the agent is holding. Revising the cap down is the
correct soft kill.

## Verifying what is deployed

The binary running on mainnet was built from this tree. To check rather than
trust:

```
solana program dump 3HCErAFs93FMk2J25Qq1xRRMp6B4FyGvif8ZV8hYxQKw live.so --url mainnet-beta
sha256sum live.so
```

Live at the time of writing: `ed4582d2a800c69d7ce257536244ac1fced9a5141f9fe0ef73ef1e1bb3072eb7`
(first 538,024 bytes; the dump is zero-padded to the 645,048-byte allocation).

---

Spec tree and local Anchor crate for **Grok Chain** intents + paymaster on Solana L1.

This folder is owned by **PROGRAMS**. It is the intent router (`pay`, `pay_token`, `swap`, `deploy`, `call`, `token_buy`, `token_sell`) and the per-human spend vault + paymaster. Pump trade ixs were cut from the live MAINNET binary for size; `init`/`fund`/`withdraw_pump_trader` stay. It is not CORE (identity/policy), not the website, not brand, and not lore.

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

Root-signed: `init_spend_vault`, `fund_spend_vault`, `withdraw_spend_vault`, `init_paymaster`, `fund_paymaster`, `withdraw_paymaster`, `set_relayer`, `pause_paymaster`, `unpause_paymaster`, `init_pump_trader`, `fund_pump_trader`, `withdraw_pump_trader` (SOL + token ATA sweep to root; not grant-gated; does not close the trader).

Agent-signed, all grant-gated (one `check_grant`, abort on CORE error, optional sponsor):

- `pay` — SpendVault → recipient. Amount must be > 0.
- `swap` — grant-gated SOL send to `out_destination` with `amount_in >= min_out`. Not a DEX. Not Jupiter. Not SPL. Not an AMM. Unchanged.
- `deploy` — `check_grant(0)` + `DeployRequested` event. Not a BPF deploy. Does not upload an ELF.
- `call` — `check_grant(amount)` (0 = policy ping). Optional vault debit. Optional `invoke` of remaining_accounts into an inner program with empty ix data.
- `pay_token` — SPL / Token-2022 to an approved merchant. Live on MAINNET 3HCErAF. Merchant registry + subscriptions are on the same binary. Do not invent a live 0.01 USDC shop payment.
- `token_buy` / `token_sell` — grant-gated Jupiter v6 (`JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4`). Pump-trader PDA is the swapper. Quote mint may be WSOL, official USDC (`EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v`), or another SPL / Token-2022 mint. Paying with native SOL/WSOL: `check_grant(sol_in)`. Paying with USDC / another token already on the trader: `check_grant(0)`. Native SOL is prefunded onto the trader (`fund_pump_trader`); no in-ix vault debit (UnbalancedInstruction). Adapter wraps SOL when asked. Does not unwrap or sweep. Remaining accounts come from Jupiter swap-instructions. Not an AMM of our own. Old `swap` is still the SOL send. Jupiter still reaches graduated pump coins.
- `pump_buy` / `pump_sell` / `pump_create` / `pump_amm_buy` / `pump_amm_sell` — **cut from the live MAINNET binary for size** (upgrade slot 442622147). Source files may still sit on disk; they are not in `#[program]`. Use `token_buy` / `token_sell` for graduated coins. Trader vault ixs stay.

`IntentStub` (error 11) is reserved.

## Build

```
cargo-build-sbf --tools-version v1.52 --manifest-path programs/grok_chain_intents/Cargo.toml
```

Keypairs are gitignored. Do not commit `**/*keypair.json` or `target/`.

Read `SPEC.md` before changing Rust.
