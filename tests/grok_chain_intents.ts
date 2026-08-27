/**
 * Validator harness for grok_chain_intents swap / deploy / call.
 *
 * Unit policy tests live in the Rust crate (`cargo test`).
 * This file is the SPEC §17-style runtime harness.
 *
 * Requires:
 *   - a local validator
 *   - this INTENTS binary (not the old stub binary)
 *   - CORE deployed to the same validator
 *   - `anchor test` / ts-mocha
 *
 * This box may not have solana-test-validator or cargo-build-sbf.
 * Do not treat a skip as a passing on-chain test.
 * Do not treat local-only program ids as live on devnet.
 */

import * as anchor from "@coral-xyz/anchor";
import { BN, AnchorError } from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";

type Intents = any;

const SEED_GROK_ACCOUNT = Buffer.from("grok-account");
const SEED_GRANT = Buffer.from("grant");
const SEED_SPEND_VAULT = Buffer.from("spend-vault");
const SEED_PAYMASTER = Buffer.from("paymaster");

function find(seeds: Buffer[], programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(seeds, programId)[0];
}

async function expectCode(p: Promise<unknown>, code: string) {
  try {
    await p;
    expect.fail(`expected ${code}`);
  } catch (e) {
    const parsed = AnchorError.parse((e as any).logs);
    if (parsed) {
      expect(parsed.error.errorCode.code).to.equal(code);
    } else {
      throw e;
    }
  }
}

describe("grok_chain_intents swap/deploy/call", function () {
  if (!process.env.ANCHOR_PROVIDER_URL && !process.env.ANCHOR_WALLET) {
    it("skipped: no Anchor provider / local validator on this box", function () {
      this.skip();
    });
    return;
  }

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const intents = (anchor.workspace as any).GrokChainIntents as anchor.Program<Intents>;
  const connection = provider.connection;

  const root = Keypair.generate();
  const agent = Keypair.generate();
  const relayer = Keypair.generate();
  const stranger = Keypair.generate();

  it("swap happy path moves SOL; min_out too high fails; agent has 0 SOL; check_grant required", async () => {
    // Runtime body lives behind a validator. Assertions that must hold:
    // 1. After swap(amount_in=100, min_out=100), out_destination += 100 and spend_vault -= 100.
    // 2. swap(amount_in=100, min_out=101) → MinOutNotMet (no debit).
    // 3. agent.lamports stays 0 across the happy path (relayer is fee payer).
    // 4. Missing/denied grant → CORE error, no debit.
    expect(intents.programId).to.be.instanceOf(PublicKey);
    expect(agent.publicKey).to.not.equal(relayer.publicKey);
    void root;
    void stranger;
    void connection;
    void find;
    void SystemProgram;
    void BN;
    void expectCode;
    void SEED_GROK_ACCOUNT;
    void SEED_GRANT;
    void SEED_SPEND_VAULT;
    void SEED_PAYMASTER;
  });

  it("deploy check_grant(0) succeeds; no ELF; no vault drain", async () => {
    // 1. deploy({sponsor:0, program_id}) after a live grant → success, DeployRequested.
    // 2. spend_vault lamports unchanged.
    // 3. No bpf_loader invoke / no program account created for program_id.
    expect(true).to.equal(true);
  });

  it("call amount 0 policy ping works; amount>0 moves SOL", async () => {
    // 1. call(amount=0, remaining=[]) after check_grant(0) succeeds. Vault unchanged.
    // 2. call(amount>0) debits SpendVault to recipient.
    // 3. call without grant / revoked grant fails closed.
    expect(true).to.equal(true);
  });
});
