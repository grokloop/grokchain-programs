# INTENTS upgrade: `pay_token` + merchant allowlist

Drop-in sources for MAINNET INTENTS `3HCErAFs93FMk2J25Qq1xRRMp6B4FyGvif8ZV8hYxQKw`.
Not wired into `mod.rs` — merge deliberately, see the checklist below.

## The gap this closes

After `token_buy` / `token_sell` the desk can **acquire** USDC. It still cannot
**spend** it. Verified against the source at `29e5787`:

- `pay` moves native lamports only — no mint, no token program anywhere in it.
- The Jupiter path requires `acc_owner == trader` for its destination, so a swap
  can never land value on a third party.
- The only SPL transfer in the whole program is the **root-only** withdraw sweep.

So there is no way for an agent to pay a merchant in a token, which is the whole
subscription and checkout story.

## Instructions added

| Instruction | Signer | What it does |
| --- | --- | --- |
| `init_merchant_registry(mint)` | root | Creates the payee allowlist, pinned to one mint |
| `add_merchant(pubkey)` | root | Approves a payee |
| `remove_merchant(pubkey)` | root | Revokes one payee — the per-merchant cancel |
| `pay_token(amount, decimals, sponsor)` | agent | Pays an approved merchant, grant-gated |

## Three decisions worth reviewing

**The payee allowlist is the point.** CORE's grant says how much, through which
program, until when. It has no recipient concept — `check_grant` compares
`allowed_programs` against `target_program` and nothing else. For trading that
is correct. For paying people it is the wrong axis, because the program is
always INTENTS. `MerchantRegistry` adds the missing control, root-owned, and
`pay_token` refuses any destination whose owner is not on it. A stolen agent key
then buys the attacker nothing except payments to merchants you already
approved.

**Cap units.** CORE meters a single u64 with no notion of asset. `pay_token`
meters the **raw token amount**, so on a USDC grant a cap of `50_000_000` means
50 USDC. That is only coherent while one agent spends one asset, which is why
the registry pins a mint — a second denomination means a second agent, and a
grant is per (grok_account, agent), so that is free.

**`TransferChecked`, not `Transfer`.** Token-2022 rejects plain `Transfer` for
mints with a transfer fee, a transfer hook, or non-transferable. Decimals are
read from the mint rather than trusted from args, because TransferChecked
compares them on chain and a mismatch there fails opaquely. The existing
withdraw sweep still uses plain `Transfer` — fine for USDC and for pump.fun
mints today, but worth the same change.

## Merge checklist

1. Copy `pay_token.rs` and `merchants.rs` into `src/instructions/`, add both to
   `mod.rs`, and declare the four handlers in `#[program]`.
2. Constants: `SEED_MERCHANTS: &[u8] = b"merchants"` and `MAX_MERCHANTS` (32 is
   a reasonable start; it sets the account size).
3. State: `MerchantRegistry { grok_account, root, mint, bump, merchants: Vec<Pubkey> }`
   with `SPACE = 8 + 32 + 32 + 32 + 1 + 4 + 32 * MAX_MERCHANTS`, plus
   `PayTokenArgs { amount: u64, decimals: u8, sponsor_lamports: u64 }`.
4. Events: `TokenPaid`, `MerchantRegistryInitialized`, `MerchantAdded`,
   `MerchantRemoved`.
5. Errors, appended after 54 (codes are append-only): `PayTokenMintNotRegistered`,
   `PayTokenMintMismatch`, `PayTokenSourceOwnerNotTrader`, `PayTokenPayeeNotAllowed`,
   `PayTokenDecimalsMismatch`, `PayTokenInsufficient`, `PayTokenAccountInvalid`,
   `MerchantRegistryFull`, `MerchantAlreadyListed`, `MerchantNotListed`.
6. `cargo test`, `cargo-build-sbf`, then upgrade `3HCErAF`. The client already
   expects these discriminators:

   | ix | `sha256("global:<name>")[..8]` |
   | --- | --- |
   | `pay_token` | `[165, 233, 248, 250, 110, 155, 215, 142]` |
   | `init_merchant_registry` | `[50, 15, 122, 207, 163, 181, 242, 7]` |
   | `add_merchant` | `[198, 82, 166, 156, 165, 93, 203, 72]` |
   | `remove_merchant` | `[55, 213, 255, 172, 106, 179, 207, 38]` |

## Status

**NOT COMPILED, NOT DEPLOYED** — no Rust toolchain where this was written.
Reviewed, not compiler-checked; expect import fixes on the first build.

## Separate finding, not fixed here

`token_buy` / `token_sell` do not enforce `min_out`. `require_token_amounts`
takes it as `_min_out` and ignores it, and there is no post-CPI balance check, so
the only slippage floor is whatever the client put inside `jupiter_data` — while
the emitted `TokenBought { min_out }` reports a floor nothing verified.
`require_jupiter_in_amount` is also a substring scan (`hay.windows(8).any(...)`),
so it does not bind the in-amount field, only asserts those bytes appear
somewhere.

The standard fix is a post-CPI balance check on the trader's own accounts:
record source and destination balances before the CPI, and afterwards require
`spent <= in_amount` and `received >= min_out`. That makes the route's contents
irrelevant — whatever Jupiter did, the outcome is bounded — and it is worth more
than any amount of parsing the route data.
