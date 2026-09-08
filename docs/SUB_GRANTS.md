# Sub-grants

**Status: written, compiled, tested — not deployed.** The live MAINNET binary
does not contain these instructions.

An agent delegating a slice of its own authority to another agent.

## The problem

CORE issues one grant to one agent, authorised by a human. That is right for one
agent and wrong for the shape real agent systems take: a planner hires a
researcher, which hires a buyer.

Today every one of those needs its own human-issued grant. Which puts the human
back in the loop for every new agent — the thing this design exists to avoid.

## The primitive

An agent issues a sub-grant out of its own budget. No wallet appears anywhere in
the tree, and the human's cap is still the real ceiling.

```
  human ──issues──▶ CORE grant ($100, 30 days)
                        │
                        ▼
                    planner ──issues──▶ sub-grant ($40, 7 days)
                                             │
                                             ▼
                                        researcher ──issues──▶ sub-grant ($10, 1 day)
                                                                     │
                                                                     ▼
                                                                   buyer
```

The buyer spending 10 moves `spent` by 10 on itself, the researcher, the planner
and the CORE grant. The human's $100 binds the whole tree, not just the planner.

## Where it lives, and why not in CORE

Grant PDAs belong to CORE, whose deployed mainnet source is not public and so
cannot be extended here. Sub-grants therefore live in INTENTS, *beneath* a single
CORE grant.

That is not a workaround. It is better: the CORE grant stays an outer ceiling
this program cannot exceed regardless of what the sub-grant tree says, so a bug
in the delegation logic cannot spend more than the human already authorised.

## Attenuation

Authority only ever narrows going down.

| rule | why |
| --- | --- |
| cap ≤ parent's **remaining** | a parent cannot hand out what it has already spent |
| expiry ≤ parent's expiry | a delegate cannot outlive its authority |
| no `token` field | CORE meters one `u64` with no notion of asset; a sub-grant naming its own token would make every cap above it meaningless |
| `init`, never `init_if_needed` | a sub-grant is written **once**; no instruction in the program can widen delegated authority afterwards |
| `revise` may only lower | same reason, for the one mutation that exists |
| depth strictly increases, bounded at 3 | cycles become impossible and the metering walk stays affordable |

Giving an agent more means issuing to a fresh key, which is visible on chain,
rather than quietly raising a number.

## Revocation cascades without traversal

Revoking a parent kills every descendant. There is no subtree walk, no bulk
update, no loop over children.

It works because **every spend re-walks the chain to the root** and refuses if
any ancestor is revoked or expired. The cascade is a property of validation
rather than an operation someone performs — which means it cannot half-finish,
cannot run out of compute, and cannot leave a partially revoked tree behind.

Killing an entire tree is one instruction on one account, no matter how many
agents hang beneath it.

## The chain cannot be forged

Callers pass the ancestor chain through `remaining_accounts`, leaf first. Two
checks make that safe:

- each step must really be the previous node's `parent`, so a caller cannot
  splice in a richer unrelated sub-grant partway up
- the top must have no parent, so a caller cannot stop early and skip the
  ancestors that would have refused

Every node is validated before any is written, so a chain that fails halfway
leaves nothing metered.

## Instructions

| | who signs | what |
| --- | --- | --- |
| `issue_sub_grant` | the delegating agent | creates a child, capped by the parent's remaining |
| `revise_sub_grant` | the issuer, or the human root | lowers a cap or shortens an expiry |
| `revoke_sub_grant` | the issuer, or the human root | kills the node and everything beneath it |

The issuer signs but never pays: a separate `payer` covers rent, because the
agent holds no SOL.

## Cost of deploying

Measured with `cargo-build-sbf` on the same tree:

| | bytes |
| --- | --- |
| without | 519,560 |
| with sub-grants | 551,896 |
| **cost** | **32,336** |

Live binary 580,600 → **612,936** in a 645,048 allocation. Fits with 32,112
spare, no `extend`.

**If the sponsorship-fee branch also ships**, the two together come to 641,080 —
still inside the allocation, but with only **3,968 bytes spare**. That is tight
enough that a third feature would need an `extend`, and worth knowing before
merging both.

68 tests pass.

## Not built

The spend integration. `walk_and_meter` and `commit_meter` are here and tested,
but no existing instruction calls them yet — `pay_token` still meters the CORE
grant directly.

That wiring is deliberately separate: it changes the account list of an
instruction already in production use, and that deserves its own review rather
than riding along with the primitive.
