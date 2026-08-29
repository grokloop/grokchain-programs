# Payments: `pay_token`, merchant allowlist, subscriptions

These are **wired into the crate** — `instructions/pay_token.rs`,
`instructions/merchants.rs`, `instructions/subscription.rs`, declared in
`mod.rs` and in `#[program]`. This file is the review note; the code is live in
the tree.

## What it closes

`pay` moves native lamports only, and the Jupiter path requires
`acc_owner == trader` on its destination, so the desk could acquire USDC and
never spend it. The only SPL transfer in the program was the root-only withdraw
sweep. Payments were impossible.

## The design decisions worth arguing with

**The payee allowlist.** CORE's grant caps an amount and names a *program*, never
a recipient. For trading that is correct — any mint is the point. For paying
people it is the wrong axis, because the program is always INTENTS. So
`MerchantRegistry` is root-owned and `pay_token` refuses any destination whose
owner is not on it. A stolen agent key then buys an attacker nothing beyond
payments to merchants the human already approved.

**Cap units.** CORE meters one `u64` with no notion of asset. `pay_token` spends
the grant cap in **raw token units**, so a USDC cap of `50_000_000` means 50
USDC, not 0.05 SOL. That is only coherent while one agent spends one asset, which
is why the registry pins a mint. A second denomination needs a second agent —
grants are per `(grok_account, agent)`, so that costs nothing.

**Idempotency on chain, not in the scheduler.** A scheduler that records "paid" in
its own database can still double-pay: send, crash before writing, restart, send
again. Nothing client-side closes a crash window between two systems. So
`last_paid_period` advances inside the same transaction that moves the money — a
repeat attempt fails on chain, and at-least-once delivery becomes exactly-once
settlement.

**Missed periods are not backfilled.** A period is payable only while it is the
current one. A bot offline for three cycles pays the current one and reports the
gap. Waking to a surprise triple charge is the worse failure, and a human can
always settle a gap by hand.

**`TransferChecked`, not `Transfer`.** Token-2022 rejects plain `Transfer` for
mints with a transfer fee, a hook, or non-transferable. Decimals are read from
the mint rather than trusted from args.

## Three scopes of cancel, none needing the merchant

| To stop | Instruction | Effect |
| --- | --- | --- |
| One subscription | `cancel_subscription` | That merchant, that mint |
| All payments to a merchant | `remove_merchant` | Every subscription to them, at once |
| All spending, keep selling | `revise_grant` cap = spent | Buys fail, sells still pass |
| Everything | `revoke_grant` | Nothing works, including exits |

## Status — READ BEFORE DEPLOYING

**NOT COMPILED.** No Rust toolchain was available where this was written. The
crate is structurally complete and self-consistent — every referenced symbol
resolves, error codes are contiguous `0..71`, braces balance, and all seven new
discriminators match the client already on `grokchain-mcp` `main` — but it has
never been through `cargo`. Expect first-build fixes.

Before any mainnet upgrade of `3HCErAF`:

1. `cargo test -p grok_chain_intents` — spec-lock tests cover the new spaces,
   seeds, period arithmetic and swap bounds.
2. `cargo-build-sbf --tools-version v1.52`
3. A **devnet dry-run of a real swap**, because the `min_out` enforcement in
   `token.rs` now rejects fills below the floor. That is the first moment you
   learn whether the floor is calibrated, and devnet is the cheap place to learn it.
4. `solana program deploy` against `3HCErAF`.

Step 3 pairs with the client change already on `grokchain-mcp` `main`, which
defaults `min_out` to Jupiter's `otherAmountThreshold` rather than `outAmount`.
Deploying the program without that client would fail every swap on any price
movement.
