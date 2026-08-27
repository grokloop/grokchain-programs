# Audit: swap / deploy / call (v1 grant-gated routers)

Date: 2026-08-28. Scope: `grok_chain_intents` handlers for `swap`, `deploy`, `call` plus MCP clients. BUILD + TEST + AUDIT only. This change did **not** deploy or upgrade the grokchain-devnet INTENTS binary. Local-only program ids are not live on devnet.

These three instructions are the same **mouth** as `pay`: agent signs, relayer fee-pays, bot never holds SOL, one `check_grant` CPI into CORE, abort on CORE error, optional sponsor from the human-funded paymaster. They are **not** Jupiter, not pump.fun, not an on-chain compiler, not a BPF deploy.

## Agent never fee-payer / never SOL source / never withdraws vaults

- `agent` is a `Signer` and is **not** `mut` on swap / deploy / call. Agent cannot be the Solana fee payer through these account lists.
- SOL moves only from program-owned `SpendVault` / `Paymaster` via `try_debit_program_owned`. Agent is never the debit source.
- Vault `withdraw_*` remain root-only (`spend_vault.rs` / `paymaster.rs`). Agent is not a signer on those ixs.
- MCP tools refuse when `GROKCHAIN_RELAYER_KEYPAIR` pubkey equals the agent (`AgentMustNotFeePay`). Relayer is the only outer fee payer.
- `invoke_signed` is **not** used. Inner `call` CPI is `invoke` only, so this program never signs as the vault PDA. Inner programs cannot steal vault lamports through our signer seeds (there are none).

## One check_grant per intent, abort on CORE error

- Each handler calls `core_cpi::check_grant` exactly once, **before** vault debit / inner invoke / sponsor.
- `core_cpi::check_grant` uses `invoke` (not a catch-and-continue). CORE custom errors abort the tx.
- CPI `target_program` is **this INTENTS router** (`crate::ID`), not the inner DEX / call target / requested deploy id.
- `amount_lamports` on the CPI:
  - `pay` / `swap`: the SOL the intent spends (must be > 0).
  - `call`: the optional SOL debit (0 = policy ping).
  - `deploy`: always `0` (`policy::DEPLOY_CHECK_GRANT_AMOUNT`).
- Sponsor reimbursement is **not** a second `check_grant`.

## amount 0 allowed only for call / deploy; swap and pay must move SOL

| Intent | amount 0 | What happens |
| --- | --- | --- |
| `pay` | `ZeroPayAmount` (2) | no CPI, no debit |
| `swap` | `ZeroAmount` (3) | no CPI, no debit |
| `call` | allowed | `check_grant(0)`, no vault debit (policy ping) |
| `deploy` | required (hardcoded 0) | `check_grant(0)`, no vault debit |

`swap` additionally requires `amount_in >= min_out` else `MinOutNotMet` (15). Honest: this is a SOL send with a min check, not an AMM quote.

## Router-mode honesty (allowlist is INTENTS, not inner DEX)

- Human allowlists **this INTENTS program id** on the CORE grant.
- CORE `check_grant` sees `target_program = crate::ID`.
- `call.args.target_program` is the **inner** program remaining_accounts are invoked into. It is **not** what CORE allowlists.
- `deploy.args.program_id` is recorded in `DeployRequested` only. CORE does not allowlist it.
- v1 does **not** trace or allowlist inner programs. A grant that allowlists INTENTS authorizes any swap destination, any call target, and any deploy-request pubkey, subject to cap / expiry / revoked.

## No seeds, no key export

- Events (`Swapped`, `DeployRequested`, `Called`, `Sponsored`) carry public keys and integers only.
- No seed phrases, keypair paths, or private keys in logs, errors, or account data.
- MCP schemas reject secret-named fields (`seed`, `mnemonic`, `privateKey`, …).
- No PDA signer seeds are passed to any inner program.

## IntentStub (11) is reserved and unused by these three

- Discriminant 11 stays `IntentStub`. Do not reuse.
- `swap` / `deploy` / `call` handlers do not return it.
- New codes are only at the end: 15 `MinOutNotMet`, 16 `CallTargetMismatch`, 17 `TargetNotExecutable`, 18 `ProtectedAccountInRemaining`.
- Codes 0..=14 are unchanged.

## What is NOT implemented

- **Not Jupiter.** No AMM, no pool, no quote, no slippage vs a market. `swap` sends `amount_in` lamports of SOL to `out_destination` iff `amount_in >= min_out`.
- **Not SPL.** v1 is SOL lamports only. No token accounts, no mint, no ATA.
- **Not a BPF deploy.** `deploy` does not invoke `bpf_loader` / `bpf_loader_upgradeable`, does not upload ELF, does not allocate a program account, does not set a program data account. It emits `DeployRequested` after `check_grant(0)`.
- **Not an on-chain compiler.** `call` remaining_accounts CPI uses **empty** ix data. Useful as a grant-gated policy ping (empty remaining) or a minimal invoke (non-empty remaining). Not a general transaction builder.
- **This source was not deployed** to Solana devnet or mainnet. The grokchain-devnet INTENTS id `EYhYtqLViS4H3FNt1Q8nGRHGt9oD87uaNsV2WJMNiRkz` still runs whatever binary was last upgraded. Do not treat these new ixs as live on public Solana until an explicit upgrade (out of this task).
- Local-only INTENTS id `AXprcURLhSqj35v9DJyBkTSPGSoZ9AfTRxYyguQJwnT2` is not a public deployment.

## Residual risks

1. **Router-mode blast radius.** Allowlisting INTENTS lets the agent hit any `out_destination`, any `call` inner program, and emit any `program_id` on deploy. Humans must treat the INTENTS allowlist as “this agent may spend up to cap via the router,” not “this agent may talk to one DEX.”
2. **`call` remaining_accounts.** Inner invoke can touch any system-owned signer passed in remaining_accounts (notably the relayer/fee payer if the client includes them as writable). Do not pass `fee_payer` as a writable remaining account unless that is intended. Handler rejects remaining_accounts that include `spend_vault` or `paymaster` (`ProtectedAccountInRemaining`).
3. **Empty inner ix data.** A non-empty remaining_accounts invoke with empty data may hit an unexpected default instruction on the target. Clients should only attach remaining_accounts when they mean to invoke that program with empty data.
4. **`call` amount > 0 + inner invoke.** SOL is debited to `recipient` *before* the inner invoke. If the inner invoke fails, the whole tx aborts (no partial pay). If it succeeds, the recipient already has the SOL; the inner program is unrelated unless the client set recipient = something the inner ix expects.
5. **Devnet binary skew.** MCP will build the new ixs against the grokchain-devnet INTENTS id when `cluster=devnet`. The currently deployed binary may still be the old stub. A send may fail with `IntentStub` on chain until an upgrade. Tools must not report that as a successful public deploy of this source.
6. **Sponsor is not a grant debit.** A sponsor_eligible grant can reimburse the relayer up to `MAX_SPONSOR_LAMPORTS` per intent even for `deploy` / `call` with amount 0. Pause the paymaster or revoke sponsor_eligible to stop that.
7. **No inner-program allowlist.** CORE cannot see the `call` target. A later version could pass remaining programs to CORE; v1 does not.
8. **Validator tests.** Unit tests cover policy, discs, and error codes. Full SOL-movement / CPI tests need a local validator running this binary and CORE. Those were not executed on this box if `solana-test-validator` / `cargo-build-sbf` are missing.

## Checks performed in this change

- Agent is never writable and never the fee payer on the three ixs.
- One `check_grant` per handler; CORE error aborts.
- `swap` / `pay` reject amount 0; `call` / `deploy` allow 0.
- Router CPI target is `crate::ID`.
- No seeds exported; `invoke_signed` unused.
- `IntentStub` unused by the three handlers; code 11 reserved.
- No Jupiter, SPL, or bpf_loader in the crate source of these handlers.
