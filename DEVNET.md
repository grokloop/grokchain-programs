# grok_chain_intents — Solana DEVNET

Cluster: **devnet** (`https://api.devnet.solana.com`)  
Not mainnet. Not a product claim. Keypairs are gitignored and were not committed.

## Program

| Field | Value |
| --- | --- |
| Program id | `EYhYtqLViS4H3FNt1Q8nGRHGt9oD87uaNsV2WJMNiRkz` |
| Status | **deployed to devnet** (upgraded after CORE landed) |
| Initial deploy signature | `F4FtsBSmvosDi72gX1c4n7J4Z8xZUjBZAN7TGtcuZ7ftbDAJmb2mMRNpGSLJcbn8fnkm2Ajj5syvG5D9o9hTdbx` |
| Initial deploy slot | `488520656` |
| Upgrade signature | `4xEMq9SaP4KZzMKc4NhfqMwo5akVzniYzjQNfD5a1iELykXHoPhhjzzYjnt2vAdMg4pCDCLQBGXpvQB8LoHLyRHP` |
| Last deployed slot | `488521581` |
| Program data address | `GDLGxyEeCT6SaHgXWZyLXYoqKNBBabYkbR6EyFb5jtfG` |
| Upgrade authority | `2GH1Uh4gFyMkBgodvZFrAddSc4XYHMNQLUff83CNybAt` |
| Artifact | `target/deploy/grok_chain_intents.so` (330496 bytes) |
| Loader | BPFLoaderUpgradeab1e |

## Explorer (devnet)

- Program: https://explorer.solana.com/address/EYhYtqLViS4H3FNt1Q8nGRHGt9oD87uaNsV2WJMNiRkz?cluster=devnet
- Initial deploy tx: https://explorer.solana.com/tx/F4FtsBSmvosDi72gX1c4n7J4Z8xZUjBZAN7TGtcuZ7ftbDAJmb2mMRNpGSLJcbn8fnkm2Ajj5syvG5D9o9hTdbx?cluster=devnet
- Upgrade tx: https://explorer.solana.com/tx/4xEMq9SaP4KZzMKc4NhfqMwo5akVzniYzjQNfD5a1iELykXHoPhhjzzYjnt2vAdMg4pCDCLQBGXpvQB8LoHLyRHP?cluster=devnet

## Deployer wallet (pubkey only)

`2GH1Uh4gFyMkBgodvZFrAddSc4XYHMNQLUff83CNybAt`  
Secret is `~/.config/solana/id.json`. Do not print, copy, or commit wallet/program keypair JSON.

## Exact deploy command

```
solana program deploy target/deploy/grok_chain_intents.so --program-id target/deploy/grok_chain_intents-keypair.json --url https://api.devnet.solana.com
```

Confirm:

```
solana program show EYhYtqLViS4H3FNt1Q8nGRHGt9oD87uaNsV2WJMNiRkz --url https://api.devnet.solana.com
```

Build (this box; default platform-tools v1.41 cannot parse Cargo.lock v4):

```
cargo-build-sbf --tools-version v1.52 --manifest-path programs/grok_chain_intents/Cargo.toml
```

## CORE CPI

`check_grant` targets CORE `declare_id` `7UtafKBBWNHEXC9PaNXu8USdZqL6VEWupsL7rS6LeVDj` via the path-dep (`grok_chain_core::ID`).

CORE **is on devnet**. Confirmed `solana program show` + CORE `DEVNET.md`:

- CORE program: `7UtafKBBWNHEXC9PaNXu8USdZqL6VEWupsL7rS6LeVDj`
- CORE deploy: `bY6KSPKygiAWUesCQbimbTVjqg2GgHUHGPaD2dd8RErSy5LuPT8f7aZ1L8Jv3NrRDEhQhkkTBsqJN3ZiBgaEs7r`
- Explorer: https://explorer.solana.com/address/7UtafKBBWNHEXC9PaNXu8USdZqL6VEWupsL7rS6LeVDj?cluster=devnet

Retired CORE local placeholder `8WDhHSfrz6hMkmX7WteAAmyuWFLryHM2Kfc1r4k8EFXE` is not on chain. Do not CPI it.

CORE source was not edited from this tree. Path-dep only.

## Id history

| Id | Status |
| --- | --- |
| `AXprcURLhSqj35v9DJyBkTSPGSoZ9AfTRxYyguQJwnT2` | retired local placeholder (keypair was missing) |
| `EYhYtqLViS4H3FNt1Q8nGRHGt9oD87uaNsV2WJMNiRkz` | new keypair; **live on devnet** |

## What this is not

No new VM. No coin. Not mainnet. `swap` / `deploy` / `call` remain stubs.
