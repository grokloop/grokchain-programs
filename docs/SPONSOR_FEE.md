# Sponsorship fees, and the burn

**Status: written, compiled, tested — and not deployed.** The live MAINNET binary
does not contain these instructions. No fee is being charged today, and none
would be charged even the day after a deploy, because the rate starts at zero and
only a root can change it.

Everything below describes code you can read in
[`instructions/sponsor_fee.rs`](../programs/grok_chain_intents/src/instructions/sponsor_fee.rs).
None of it describes something happening on chain right now.

## The idea

Gasless execution is not free. An agent that holds no SOL still needs its
transactions paid for, so the relayer fronts the fee and the paymaster reimburses
it. That cost is real and somebody bears it.

This adds a second leg to that arrangement: an agent using the paymaster also
pays a fee denominated in **$GrokChain**, which accumulates in a vault whose only
exit is destruction.

Not a treasury. Not a buyback. Not a claim on anything. The tokens are burned —
supply falls, nobody receives them, and that is the entire effect.

## Why the burn is a property and not a promise

There is no withdraw instruction. Not a root-gated one, not a timelocked one, not
one behind a multisig. Open the file and search for a transfer out of the fee
vault: there isn't one. The only instruction that can move that balance is
`burn_collected_fees`, which hands it to Token-2022's `BurnChecked` and reduces
total supply.

That distinction is the whole point. A treasury with a withdraw function the team
promises not to use is a promise, and promises are worth what the team is worth.
A vault with no withdraw function is arithmetic, and arithmetic does not depend
on anyone's intentions.

Three further choices follow from the same reasoning:

**The burn is permissionless.** If only a root could fire it, burning would
happen when a root felt like it, and "will be burned" would be a promise again.
Anyone may call it. The caller pays the transaction fee and receives nothing.

**It takes no amount.** There is no partial burn. It always burns the full
balance, so there is no parameter anyone can shade.

**It verifies afterwards.** The balance is read back after the CPI and the
instruction reverts unless the vault is empty. A burn that did not burn cannot
emit an event saying it did.

## The rate has a ceiling, and starts at zero

`MAX_FEE_PER_INTENT` caps what a root may set at 10 $GrokChain per sponsored
intent. A fee cannot quietly become confiscatory for agents already depending on
the paymaster.

`init_sponsor_fee` creates the config at **zero**. An account whose root never
calls `set_sponsor_fee` is never charged anything, even on a deployed program.

## Instructions

| | who | what |
| --- | --- | --- |
| `init_sponsor_fee` | root | creates the config, rate zero |
| `set_sponsor_fee` | root | sets the rate, capped at `MAX_FEE_PER_INTENT` |
| `burn_collected_fees` | **anyone** | burns the entire vault balance |

## What deploying would cost

Measured, not estimated — built with `cargo-build-sbf` against the same tree:

| | bytes |
| --- | --- |
| without the module | 519,560 |
| with it | 547,704 |
| **cost** | **28,144** |

Applied to the live binary that is 580,600 → **608,744**, against a programdata
capacity of 645,048. It fits with 36,304 bytes to spare and needs no
`solana program extend`. Buffer rent at deploy would be about 4.24 SOL, refunded
when the deploy completes.

60 tests pass. `cargo build --release` and `cargo-build-sbf` both clean.

## The honest part

A fee mechanism earns what its usage earns. The paymaster currently holds a
fraction of a SOL, and the number of sponsored intents to date is small enough to
count. A burn that consumes near-zero tokens reduces supply by near-zero.

So this is worth deploying because it is the right shape for whoever runs a
relayer at scale later — the accounting is in place, the exit is fixed, and the
guarantee does not depend on anybody's word. It is not worth deploying as a price
event, and nothing here should be described as one.

## What is not built

The collection leg. `note_collected` exists and the vault, the config and the
burn are all here, but the sponsored-intent path does not yet debit the fee. That
is the change that makes this live, and it is deliberately not in this commit —
wiring it would alter the behaviour of every existing sponsored instruction, and
that deserves its own review rather than riding along with the scaffolding.
