# Audit: pump.fun buy / sell / limit / launch vs swap / deploy / call

Date: 2026-08-28 IST. Scope: can `grok_chain_intents` `swap`, `deploy`, and `call` perform a pump.fun bonding-curve buy, sell, limit order, or coin launch? BUILD + TEST + AUDIT only.

**Verdict: FAIL on all four user asks.** This source cannot do any of them. That is not a maybe. The handlers do not speak pump.fun's instruction language, do not move SPL tokens, do not create a mint, and cannot sign as the vault PDA that holds the SOL.

No mainnet transaction was sent. No real SOL was spent. No live pump.fun coin was traded. INTENTS was not deployed or upgraded on public Solana. Nothing was git-pushed (this tree is not a git repo). CloudAgent was not called.

Official pump.fun program (mainnet and devnet): `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`.
Official docs fetched: https://github.com/pump-fun/pump-public-docs (`docs/instructions/BUY.md`, `SELL.md`, `COIN_CREATION.md`, `docs/PUMP_PROGRAM_README.md`, README v2 announcement).

Live public INTENTS binary on grokchain-devnet `EYhYtqLViS4H3FNt1Q8nGRHGt9oD87uaNsV2WJMNiRkz` was **not** upgraded in this work. Do not treat these ixs as live on that id.

## Result table

| User ask | Result | Why |
| --- | --- | --- |
| **buy** | **FAIL** | `call` CPI uses `Instruction { data: vec![] }`. Official `buy` / `buy_v2` need discriminator + `amount:u64` + `max_sol_cost:u64` and 15–27 accounts including a **user signer**, bonding-curve PDA, and ATAs. Those bytes never reach pump.fun. `swap` is a SOL lamport send (`SpendVault → out_destination`) iff `amount_in >= min_out`. Not a DEX. No SPL. No curve. |
| **sell** | **FAIL** | Same empty-data wall. Official `sell` / `sell_v2` need discriminator + `amount` + `min_sol_output`, user signer, and `associated_base_user`. `swap` cannot hold or transfer the coin. `call` cannot forward the payload. |
| **limit** | **FAIL** | Not a pump.fun bonding-curve primitive. Official instruction docs are BUY / SELL / COIN_CREATION / CLAIM_CASHBACK / COLLECT_CREATOR_FEE / CREATOR_FEE_SHARING. Program README adds `withdraw` (disabled), `migrate`, `extend_account`, `initialize`, `set_params`. v2 announcement adds `buy_v2`, `sell_v2`, `buy_exact_quote_in_v2`. There is no `limit_order` / `place_order` / `cancel_order`. The curve is a constant-product AMM. We do not invent a limit-order ix. INTENTS also has none. |
| **launch** | **FAIL** | `deploy` is `check_grant(0)` + `DeployRequested` event. No `bpf_loader`, no mint, no ELF, remaining_accounts ignored. Official `create` / `create_v2` need a mint signer, Token-2022, bonding-curve PDA, and non-empty data (`name`, `symbol`, `uri`, `creator`, flags). `call` cannot forward that payload. |

## What pump.fun actually requires (from official docs)

`buy_v2` (BUY.md): 27 mandatory accounts. `user` is the transaction signer and buyer. Instruction data: `amount: u64` (base tokens, > 0) + `max_sol_cost: u64`. Needs bonding_curve PDA, associated_base_user ATA, quote ATAs, fee recipients, token programs, system_program. SDK prepends ATA creates.

`sell_v2` (SELL.md): 26 mandatory accounts. `user` is the signer and seller. Data: `amount: u64` + `min_sol_output: u64`. Needs the user's base ATA.

`create_v2` (COIN_CREATION.md): mint is a new Token-2022 account and a **signer**. `user` is signer and payer. Data: `name` (≤32), `symbol` (≤13), `uri` (≤200), `creator` (nonzero pubkey), `is_mayhem_mode`, optional `is_cashback_enabled`. Not an empty ping.

Legacy `buy` / `sell` / `create` are the same idea: non-empty ix data, user signer, ATAs / mint. Encoded locally for the test (discriminator + args). **Never sent.**

Discriminators used only to prove encode size and that our call data is empty:

| ix | hex (8 bytes) | matches `sha256("global:<name>")[:8]` |
| --- | --- | --- |
| buy | `66063d1201daebea` | yes |
| sell | `33e685a4017f83ad` | yes |
| create | `181ec828051c0777` | yes |
| buy_v2 | `b817ee6167c5d33d` | yes |
| sell_v2 | `5df6823ce7e940b2` | yes |
| create_v2 | `d6904cec5f8b31b4` | yes |

## What our intents actually do today

Source of truth: `programs/grok_chain_intents/src/instructions/{swap,deploy,call}.rs`.

**swap.** `check_grant(amount_in)` then `SpendVault → out_destination` SOL iff `amount_in >= min_out`. Comment in handler: "Not Jupiter. Not an AMM." No SPL, no ATA, no mint, no bonding_curve, no `invoke`. Account list is agent / grok_account / grant / CORE / INTENTS / spend_vault / out_destination / system / optional paymaster+fee_payer.

**deploy.** `check_grant(0)`, emit `DeployRequested { program_id }`. No bpf_loader, no mint, no ELF. remaining_accounts ignored.

**call.** `check_grant` then optional `invoke` (not `invoke_signed`) with **empty instruction data**. Target must be executable and appear in remaining_accounts. `CallArgs` is `{ amount_lamports, sponsor_lamports, target_program }` — no `data: Vec<u8>`. `reject_protected_remaining` refuses spend_vault and paymaster in remaining_accounts.

Therefore buy/sell/create discriminators **never** reach pump.fun through `call`. A tiny mock that rejects empty data and accepts a well-formed buy/sell/create encoding fails the mirrored call inner ix and accepts the encoded one. That is the proof.

## Structural blocker: invoke_signed ban + vault is not the pump user

This is the part that still fails even if someone later wires ix data through `call`.

1. pump.fun expects `user` to be a **signer**. That wallet is who the curve debits (SOL/quote) and who owns the ATAs.
2. In our mouth the **agent** signs INTENTS and is documented "Never the fee payer / SOL source."
3. The SOL lives on **SpendVault**, a program-owned PDA. Debits today are `try_debit_program_owned` (raw lamports), not a system transfer signed by the PDA.
4. `call` uses `invoke`, never `invoke_signed`. We refuse to sign as SpendVault. pump.fun therefore cannot treat the vault as `user`.
5. `reject_protected_remaining` would reject the vault if a client tried to pass it in remaining_accounts anyway.
6. If the agent were passed as pump `user`, the agent has no SOL and no token ATA the vault funds. The vault SOL never becomes the agent's.

So: **even with ix data, buy would need a different custody model** (or a pump adapter program we own that is allowed to debit a dedicated trader PDA with explicit seeds). User/agent cannot be the pump `user` signer if the vault is the SOL source. That is a structural blocker, not a missing argument.

## What a later real integration would require

Not a how-to. A checklist of why v1 cannot grow into this by accident:

- `call` must forward **caller-supplied ix data** and a real remaining-account metas list. Today data is hardcoded empty.
- That change, by itself, turns INTENTS into a general CPI router. Router-mode blast radius becomes "any program, any payload" unless CORE starts allowlisting the inner program and the ix disc.
- `swap` would need SPL (token program, ATAs, mint, bonding curve, fee recipients) and an actual quote. Today it is a SOL send with a min check.
- Custody: either (a) a Grok-owned adapter that is the pump `user` and holds SOL/tokens in a PDA it **is** allowed to `invoke_signed` for, with a tight allowlist, or (b) the human/root signs pump ixs directly from a normal wallet, outside INTENTS. Option (a) is a new program and a new threat model. Option (b) is not an intent.
- ATA creation, mint signer (for launch), and Token-2022 are all outside v1.
- Limit orders would still not exist on the bonding curve. A "limit" would be an off-chain watcher plus a later buy/sell — that is a bot product, not an intent, and is out of scope here.

## Tests that were run

Local encode + static analysis + unit asserts. No validator. No RPC to mainnet.

1. `tests/pumpfun-capability.test.mjs` — encode official discs + args; mirror `call` inner `data: []`; mock rejects empty / accepts encoded buy_v2 sell_v2 create_v2; static-read swap/deploy/call source; official ix set has no limit order. Environment Auto-review blocked executing this `.mjs` by path; the file exists and is the requested script.
2. Twin unit tests in `programs/grok_chain_intents/src/capability_audit.rs` (wired from `lib.rs` as `#[cfg(test)] mod capability_audit`). Ran:

```
cargo test -p grok_chain_intents --lib capability -- --nocapture
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out
```

Tests: `discs_match_task_hex`, `encoded_ix_data_is_nonempty`, `call_inner_ix_data_is_empty_so_discs_never_forward`, `swap_is_sol_only_send`, `deploy_is_event_only`, `no_limit_order_primitive`, `invoke_signed_ban_and_vault_not_user`, `capability_table_all_fail`.

Existing policy tests also still pass (`require_swap_amounts`, deploy check_grant amount 0).

## Residual risks if someone later wires this naively

1. **Router-mode blast radius.** A grant that allowlists INTENTS already authorizes any `call` target. Forwarding arbitrary ix data makes that "any program, any instruction." CORE never sees the inner disc.
2. **Writable fee payer.** `call` remaining_accounts can mark the relayer writable. An inner program can debit a system-owned signer. Do not pass `fee_payer` writable unless that is intended. Vault/paymaster are already rejected; the relayer is not.
3. **Mainnet money.** These handlers are not on the public INTENTS binary yet. An upgrade that adds data-forwarding + `invoke_signed` as the vault would let an inner program drain SpendVault. That is the opposite of the current ban.
4. **Empty data hitting a default ix.** Today's empty invoke can still surprise if remaining_accounts are attached to a program that treats empty data as an instruction. Clients should only attach remaining when they mean "invoke this program with no data."
5. **Devnet binary skew.** MCP against `EYhYtq…` is not this source. A send may fail with `IntentStub`. That is not a successful public deploy of swap/deploy/call, and it is not a pump trade.

## Honesty bar

- swap is not Jupiter and not pump.fun.
- deploy is not a coin launch and not a BPF deploy.
- call is not a transaction compiler.
- limit orders are not a pump.fun bonding-curve primitive.
- This change did not put any of the above on public Solana.
