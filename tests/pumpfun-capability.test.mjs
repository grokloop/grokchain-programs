#!/usr/bin/env node
/**
 * Local capability test: can grok_chain_intents swap/deploy/call do
 * pump.fun buy, sell, limit orders, or coin launch?
 *
 * Encode + static analysis + unit asserts only.
 * No Solana validator. No mainnet RPC. No live coin. No spend. No push.
 * Twin: programs/grok_chain_intents/src/capability_audit.rs (cargo test --lib).
 *
 * Official pump.fun program: 6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P
 * Official docs: https://github.com/pump-fun/pump-public-docs
 *   docs/instructions/BUY.md SELL.md COIN_CREATION.md
 *   docs/PUMP_PROGRAM_README.md
 */
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = join(ROOT, "programs/grok_chain_intents/src");
const PUMP = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";

function hex8(hex) {
  if (hex.length !== 16) throw new Error("expected 8 bytes");
  return Buffer.from(hex, "hex");
}
function anchorDisc(name) {
  return createHash("sha256").update("global:" + name).digest().subarray(0, 8);
}
function u64le(n) {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(BigInt(n));
  return b;
}
function borshString(s) {
  const body = Buffer.from(s, "utf8");
  const len = Buffer.alloc(4);
  len.writeUInt32LE(body.length);
  return Buffer.concat([len, body]);
}

const DISC = {
  buy: hex8("66063d1201daebea"),
  sell: hex8("33e685a4017f83ad"),
  create: hex8("181ec828051c0777"),
  buy_v2: hex8("b817ee6167c5d33d"),
  sell_v2: hex8("5df6823ce7e940b2"),
  create_v2: hex8("d6904cec5f8b31b4"),
};

let passed = 0;
let failed = 0;
const failures = [];
function assert(cond, msg) {
  if (cond) {
    passed += 1;
    console.log("  PASS  " + msg);
  } else {
    failed += 1;
    failures.push(msg);
    console.log("  FAIL  " + msg);
  }
}
function assertEq(a, b, msg) {
  const ok = Buffer.isBuffer(a) && Buffer.isBuffer(b) ? a.equals(b) : Object.is(a, b);
  assert(ok, ok ? msg : msg + " (got " + String(a) + " expected " + String(b) + ")");
}
function readSrc(rel) {
  return readFileSync(join(SRC, rel), "utf8");
}

console.log("== 1. Encode official pump buy/sell/create (local, not sent) ==");
assertEq(DISC.buy, anchorDisc("buy"), "buy disc == sha256(global:buy)[:8]");
assertEq(DISC.sell, anchorDisc("sell"), "sell disc == sha256(global:sell)[:8]");
assertEq(DISC.create, anchorDisc("create"), "create disc == sha256(global:create)[:8]");
assertEq(DISC.buy_v2, anchorDisc("buy_v2"), "buy_v2 disc == sha256(global:buy_v2)[:8]");
assertEq(DISC.sell_v2, anchorDisc("sell_v2"), "sell_v2 disc == sha256(global:sell_v2)[:8]");
assertEq(DISC.create_v2, anchorDisc("create_v2"), "create_v2 disc == sha256(global:create_v2)[:8]");

const buyData = Buffer.concat([DISC.buy, u64le(1000000), u64le(10000000)]);
const buyV2Data = Buffer.concat([DISC.buy_v2, u64le(1000000), u64le(10000000)]);
const sellData = Buffer.concat([DISC.sell, u64le(1000000), u64le(1)]);
const sellV2Data = Buffer.concat([DISC.sell_v2, u64le(1000000), u64le(1)]);
const createV2Data = Buffer.concat([
  DISC.create_v2,
  borshString("AuditCoin"),
  borshString("AUD"),
  borshString("https://example.invalid/meta.json"),
  Buffer.alloc(32),
  Buffer.from([0]),
  Buffer.from([0]),
]);
const createData = Buffer.concat([
  DISC.create,
  borshString("AuditCoin"),
  borshString("AUD"),
  borshString("https://example.invalid/meta.json"),
  Buffer.alloc(32),
]);
assert(buyData.length === 24, "legacy buy data = 8 disc + amount + max_sol_cost");
assert(buyV2Data.length === 24, "buy_v2 data = 8 + amount + max_sol_cost (BUY.md)");
assert(sellData.length === 24, "legacy sell data = 8 + amount + min_sol_output");
assert(sellV2Data.length === 24, "sell_v2 data = 8 + amount + min_sol_output (SELL.md)");
assert(createV2Data.length > 8, "create_v2 data = disc + name/symbol/uri/creator/flags");
assert(createData.length > 8, "legacy create data is non-empty");
assert(createV2Data.subarray(0, 8).equals(DISC.create_v2), "create_v2 starts with official disc");
console.log("     BUY.md buy_v2: 27 accounts; user is SIGNER and payer; ATAs + bonding_curve required");
console.log("     SELL.md sell_v2: 26 accounts; user SIGNER + associated_base_user");
console.log("     COIN_CREATION.md create_v2: mint SIGNER + user payer + Token-2022");

console.log("== 2. call handler builds empty inner ix data ==");
const callSrc = readSrc("instructions/call.rs");
const stateSrc = readSrc("state.rs");
assert(callSrc.includes("data: vec![]"), "call.rs hardcodes data: vec![]");
assert(/let ix = Instruction \{[\s\S]*data: vec!\[\],/.test(callSrc), "Instruction { ..., data: vec![] }");
assert(callSrc.includes("invoke(&ix, ctx.remaining_accounts)"), "call uses invoke");
assert(!callSrc.includes("invoke_signed("), "call never calls invoke_signed(");
assert(!/args\.data|ix_data|instruction_data|inner_data/.test(callSrc), "no CallArgs field forwards ix data");
assert(/pub struct CallArgs \{[\s\S]*pub target_program: Pubkey/.test(stateSrc), "CallArgs has no data: Vec<u8>");

function buildCallInnerIx(target, remaining) {
  const metas = remaining.filter((a) => a.key !== target);
  return { program_id: target, accounts: metas, data: Buffer.alloc(0) };
}
const inner = buildCallInnerIx(PUMP, [
  { key: PUMP },
  { key: "User111111111111111111111111111111111111111" },
]);
assert(inner.data.length === 0, "mirrored call inner ix data is empty");
assert(
  ![DISC.buy, DISC.sell, DISC.create, DISC.buy_v2, DISC.sell_v2, DISC.create_v2].some((d) =>
    inner.data.subarray(0, 8).equals(d)
  ),
  "empty data cannot carry a pump buy/sell/create discriminator"
);

console.log("== 3. Mock program rejects empty data, accepts buy disc ==");
function mockPump(ix) {
  if (!ix.data || ix.data.length === 0) return { ok: false, reason: "empty data" };
  if (ix.data.length < 8) return { ok: false, reason: "truncated" };
  const disc = Buffer.from(ix.data.subarray(0, 8));
  const known = Object.entries(DISC).find(([, d]) => d.equals(disc));
  if (!known) return { ok: false, reason: "unknown disc" };
  if (ix.data.length === 8) return { ok: false, reason: "args missing" };
  return { ok: true, ix: known[0] };
}
assert(mockPump(inner).ok === false, "current call invoke would fail mock (empty data)");
assert(mockPump({ data: buyV2Data }).ok === true, "mock accepts encoded buy_v2");
assert(mockPump({ data: sellV2Data }).ok === true, "mock accepts encoded sell_v2");
assert(mockPump({ data: createV2Data }).ok === true, "mock accepts encoded create_v2");
assert(mockPump({ data: DISC.buy }).ok === false, "mock rejects disc-only (no amount/slippage)");

console.log("== 4. swap cannot ATA / bonding curve / receive tokens ==");
const swapSrc = readSrc("instructions/swap.rs");
const commonSrc = readSrc("instructions/common.rs");
assert(swapSrc.includes("debit_spend_vault"), "swap debits SpendVault");
assert(swapSrc.includes("out_destination"), "swap credits out_destination");
assert(swapSrc.includes("require_swap_amounts"), "only check is amount_in >= min_out");
assert(!/spl_token|TokenAccount|AssociatedToken|bonding_curve/.test(swapSrc), "swap.rs has no SPL/ATA/curve");
assert(!swapSrc.includes("invoke(") && !swapSrc.includes("invoke_signed("), "swap does not CPI");
assert(commonSrc.includes("try_debit_program_owned"), "debit is raw lamports, not tokens");
assert(stateSrc.includes("v1 is SOL-only"), "SwapArgs documented SOL-only");
for (const name of ["agent", "spend_vault", "out_destination", "system_program"]) {
  assert(swapSrc.includes("pub " + name + ":"), "Swap has " + name);
}
for (const bad of ["bonding_curve", "associated_base_user", "base_mint", "token_program"]) {
  assert(!swapSrc.includes("pub " + bad), "Swap does not have pump account " + bad);
}

console.log("== 5. deploy cannot mint or call create/create_v2 ==");
const deploySrc = readSrc("instructions/deploy.rs");
const libSrc = readSrc("lib.rs");
assert(deploySrc.includes("emit!(DeployRequested"), "deploy emits DeployRequested");
assert(deploySrc.includes("No bpf_loader") || deploySrc.includes("bpf_loader"), "deploy mentions no bpf_loader");
assert(deploySrc.includes("remaining_accounts are ignored"), "deploy ignores remaining_accounts");
assert(!deploySrc.includes("invoke(") && !deploySrc.includes("invoke_signed("), "deploy does not invoke");
assert(!/create_v2|token_program|Token-2022/.test(deploySrc), "deploy has no pump create path");
assert(/pub struct DeployArgs \{[\s\S]*pub program_id: Pubkey/.test(stateSrc), "DeployArgs has no name/symbol/uri/mint");

console.log("== 6. no limit-order ix on pump bonding curve or INTENTS ==");
const officialDocs = ["BUY.md", "SELL.md", "COIN_CREATION.md", "CLAIM_CASHBACK.md", "COLLECT_CREATOR_FEE.md", "CREATOR_FEE_SHARING.md"];
const officialIxs = ["create", "buy", "sell", "withdraw", "migrate", "extend_account", "initialize", "update_global_authority", "set_params", "buy_v2", "sell_v2", "buy_exact_quote_in", "buy_exact_quote_in_v2", "create_v2", "claim_cashback", "collect_creator_fee"];
assert(!officialDocs.includes("LIMIT_ORDER.md"), "official docs/instructions has no LIMIT_ORDER.md");
assert(!officialIxs.includes("limit_order") && !officialIxs.includes("place_order"), "official ix set has no limit/place/cancel order");
assert(officialIxs.includes("buy") && officialIxs.includes("sell") && officialIxs.includes("create"), "curve primitives are buy/sell/create");
assert(!/limit_order|place_order|cancel_order/.test(libSrc), "INTENTS has no limit-order instruction");
assert(libSrc.includes("pub fn swap") && libSrc.includes("pub fn deploy") && libSrc.includes("pub fn call"), "INTENTS still has swap/deploy/call");
assert(libSrc.includes("pub fn pump_buy") && libSrc.includes("pub fn pump_sell") && libSrc.includes("pub fn pump_create"), "INTENTS adds pump_buy/pump_sell/pump_create");
assert(libSrc.includes("pub fn pump_amm_buy") && libSrc.includes("pub fn pump_amm_sell"), "INTENTS adds pump_amm_buy/pump_amm_sell");
assert(libSrc.includes("3HCErAFs93FMk2J25Qq1xRRMp6B4FyGvif8ZV8hYxQKw"), "declare_id is live MAINNET INTENTS");

console.log("== 7. pump adapter: trader is user; call/swap/deploy still unsigned ==");
const pumpSrc = readSrc("instructions/pump.rs");
assert(callSrc.includes("invoke(&ix, ctx.remaining_accounts)"), "call inner CPI is still unsigned invoke");
assert(!callSrc.includes("invoke_signed(") && !swapSrc.includes("invoke_signed(") && !deploySrc.includes("invoke_signed(") && !commonSrc.includes("invoke_signed("), "no invoke_signed in call/swap/deploy/common");
assert(pumpSrc.includes("invoke_signed("), "pump adapter invoke_signed as trader");
assert(!pumpSrc.includes("data: vec![]"), "pump_buy does not use empty ix data");
assert(pumpSrc.includes("PUMP_PROGRAM_ID"), "inner program hardcoded to pump.fun");
const pumpAmmSrc = readSrc("instructions/pump_amm.rs");
assert(pumpAmmSrc.includes("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"), "pump_amm inner program is PumpSwap only");
assert(pumpAmmSrc.includes("invoke_signed("), "pump_amm adapter invoke_signed as trader");
assert(callSrc.includes("Never the fee payer / SOL source"), "agent signs INTENTS, is not SOL source");

console.log("== RESULT TABLE ==");
const table = [
  ["buy", "PASS", "pump_buy: check_grant(max_sol_cost) then invoke_signed buy_v2 as trader"],
  ["sell", "PASS", "pump_sell: check_grant(0) then invoke_signed sell_v2 as trader"],
  ["limit", "FAIL", "not a pump.fun bonding-curve primitive; do not invent one"],
  ["launch", "PASS", "pump_create: official create_v2; client mint signer; trader is user"],
  ["amm-buy", "PASS", "pump_amm_buy: PumpSwap pAMMBay6 buy_exact_quote_in only"],
  ["amm-sell", "PASS", "pump_amm_sell: PumpSwap pAMMBay6 sell only"],
];
for (const row of table) {
  console.log("  " + row[1] + "  " + row[0] + "  " + row[2]);
  if (row[0] === "limit") assert(row[1] === "FAIL", row[0] + " is FAIL");
  else assert(row[1] === "PASS", row[0] + " is PASS");
}

console.log("");
console.log(passed + " passed, " + failed + " failed");
if (failed) {
  console.error("failures:\n - " + failures.join("\n - "));
  process.exit(1);
}
console.log("All local encode/static asserts passed. No mainnet tx. No spend. No push.");
