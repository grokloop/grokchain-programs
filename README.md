# grok-chain-programs

Spec tree and local Anchor crate for **Grok Chain** intents + paymaster on Solana L1.

This folder is owned by **PROGRAMS**. It is the intent router (`pay`, `swap`, `deploy`, `call`) and the per-human spend vault + paymaster. It is not CORE (identity/policy), not the website, not brand, and not lore.

| Path | What it is |
| --- | --- |
| `SPEC.md` | Implementable v1 spec (source of truth) |
| `DEVNET.md` | Devnet program id, deploy sig, explorer URLs |
| `programs/grok_chain_intents` | Anchor/Rust crate implementing SPEC.md |
| `target/deploy/grok_chain_intents-keypair.json` | Program keypair (gitignored; do not commit) |
| `target/deploy/grok_chain_intents.so` | SBF artifact from `cargo-build-sbf` |

## Program id

`declare_id!` is the pubkey of `target/deploy/grok_chain_intents-keypair.json`:

`EYhYtqLViS4H3FNt1Q8nGRHGt9oD87uaNsV2WJMNiRkz`

**Devnet.** Confirmed deployed to Solana **devnet** (see `DEVNET.md`). Not mainnet. Not a product claim.

Retired local placeholder (keypair was missing): `AXprcURLhSqj35v9DJyBkTSPGSoZ9AfTRxYyguQJwnT2`. Do not use it.

Explorer: https://explorer.solana.com/address/EYhYtqLViS4H3FNt1Q8nGRHGt9oD87uaNsV2WJMNiRkz?cluster=devnet

Pinned in `Anchor.toml` / `Cargo.toml`: Anchor 0.30.1 (sha256 `global:` / `account:` / `event:` discriminators per SPEC.md).

`blake3 = "=1.5.5"` is pinned so older platform-tools cargo can parse the graph (1.8+ pulls edition2024 `cpufeatures`).

## CORE dependency

Path-depends on CORE (do not copy CORE source, do not edit CORE files):

```
grok_chain_core = { path = "../grok-chain-core/programs/grok_chain_core", features = ["cpi"] }
```

(from this workspace root; the crate `Cargo.toml` uses `../../../grok-chain-core/programs/grok_chain_core`.)

CORE **devnet** program id: `7UtafKBBWNHEXC9PaNXu8USdZqL6VEWupsL7rS6LeVDj` (live; see CORE `DEVNET.md`). Not mainnet. Root allowlists this router (`EYhYtqLViS4H3FNt1Q8nGRHGt9oD87uaNsV2WJMNiRkz`) on `issue_grant` / `revise_grant`. CORE does not hardcode it. Do not allowlist `AXprcURLhSqj35v9DJyBkTSPGSoZ9AfTRxYyguQJwnT2`.

The only CPI is `check_grant`. Thin invoke, documented metas, hardcoded disc `[223, 172, 131, 140, 15, 133, 209, 250]`.

## What this crate is

- Sit on **Solana L1**. No new VM. No coin.
- Human root funds a **SpendVault** (pay source) and a **Paymaster** (gas vault).
- Agent signs intents. Agent never holds SOL, never is the fee payer, never deposits/withdraws.
- Grok Chain (Tanmay) does **not** pay every fee. Each human funds their own paymaster. Relayer is the only reimbursed fee payer.
- Promo float is optional / out of v1. Not the model. Not implemented.

## What is implemented

Root-signed: `init_spend_vault`, `fund_spend_vault`, `withdraw_spend_vault`, `init_paymaster`, `fund_paymaster`, `withdraw_paymaster`, `set_relayer`, `pause_paymaster`, `unpause_paymaster`.

Agent-signed, all grant-gated (one `check_grant`, abort on CORE error, optional sponsor):

- `pay` — SpendVault → recipient. Amount must be > 0.
- `swap` — grant-gated SOL send to `out_destination` with `amount_in >= min_out`. Not a DEX. Not Jupiter. Not SPL.
- `deploy` — `check_grant(0)` + `DeployRequested` event. Not a BPF deploy. No ELF.
- `call` — `check_grant(amount)` (0 = policy ping). Optional vault debit. Optional `invoke` of remaining_accounts into an inner program with empty ix data.

`IntentStub` (error 11) is reserved and unused by these three.

This source was **not** upgraded on grokchain-devnet in the swap/deploy/call change. Do not treat the new ixs as live on public Solana until an explicit upgrade.

## Build / deploy status (this box)

- `cargo-build-sbf --tools-version v1.52 --manifest-path programs/grok_chain_intents/Cargo.toml` produced `target/deploy/grok_chain_intents.so` (330496 bytes). Default platform-tools v1.41 cannot parse Cargo.lock v4.
- Deployed to **devnet** as `EYhYtqLViS4H3FNt1Q8nGRHGt9oD87uaNsV2WJMNiRkz`. Initial tx `F4FtsBSmvosDi72gX1c4n7J4Z8xZUjBZAN7TGtcuZ7ftbDAJmb2mMRNpGSLJcbn8fnkm2Ajj5syvG5D9o9hTdbx`. Upgrade tx `4xEMq9SaP4KZzMKc4NhfqMwo5akVzniYzjQNfD5a1iELykXHoPhhjzzYjnt2vAdMg4pCDCLQBGXpvQB8LoHLyRHP` (slot 488521581). Details in `DEVNET.md`.
- CORE is **on devnet** as `7UtafKBBWNHEXC9PaNXu8USdZqL6VEWupsL7rS6LeVDj`. `check_grant` CPI path-dep targets that id. Not mainnet.
- Keypairs are gitignored. Do not commit `**/*keypair.json` or `target/`.
- `anchor` CLI is not required for this deploy. No mainnet.

```
cargo-build-sbf --tools-version v1.52 --manifest-path programs/grok_chain_intents/Cargo.toml
solana program deploy target/deploy/grok_chain_intents.so --program-id target/deploy/grok_chain_intents-keypair.json --url https://api.devnet.solana.com
```

Read `SPEC.md` before changing Rust.
