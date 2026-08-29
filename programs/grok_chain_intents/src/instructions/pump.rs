//! Tight pump.fun buy_v2 / sell_v2 / create_v2 adapter.
//!
//! Not a general CPI router. The inner program is hardcoded to the official
//! pump.fun id. Instruction data is constructed here (official discs + typed
//! args) and is never forwarded from the client as raw bytes.
//!
//! Pump `user` is the system-owned pump-trader PDA (seeds
//! `[b"pump-trader", grok_account]`). SpendVault is INTENTS-owned (73 bytes)
//! and is never pump user — system transfer needs a system-owned from.
//! invoke_signed does not change owner. We `invoke_signed` trader seeds only
//! for the hardcoded pump id.
//!
//! No in-ix vault→trader debit. Trader is remaining[13] (buy/sell) or
//! remaining[5] (create) only — not a named PumpTrade account. Named+remaining
//! duplicate of the same writable system account + raw debit or later system
//! CPI is UnbalancedInstruction (same as pump_amm). Root pre-funds via
//! fund_pump_trader. require_pump_trader_prefunded on buy/create.
//! Do NOT sweep trader→vault in the same ix. Leftover / sale SOL stays on
//! the trader PDA. pump_sell spends tokens, not SOL — no SOL debit.
//!
//! `pump_create` is grant-gated official create_v2. Mint is a NEW Token-2022
//! keypair signed by the client (relayer/root), never the vault. Creator
//! recorded on-chain is `grok_account.root`. Limit orders: pump has none.
//! `call` is not turned into a data-forwarding router.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

use grok_chain_core::{Grant, GrokAccount, SEED_GRANT, SEED_GROK_ACCOUNT};

use crate::constants::{
    encode_pump_buy_v2, encode_pump_create_v2, encode_pump_sell_v2,
    ASSOCIATED_TOKEN_PROGRAM_ID, MAYHEM_PROGRAM_ID, PUMP_BUY_V2_ACCOUNT_COUNT,
    PUMP_CREATE_MINT_INDEX, PUMP_CREATE_PROGRAM_INDEX, PUMP_CREATE_USER_INDEX,
    PUMP_PROGRAM_ID, PUMP_PROGRAM_INDEX_BUY,
    PUMP_PROGRAM_INDEX_SELL, PUMP_SELL_V2_ACCOUNT_COUNT, PUMP_TRADE_IX_DATA_LEN,
    PUMP_TRADER_SPACE, PUMP_USER_INDEX, SEED_PAYMASTER, SEED_PUMP_TRADER,
    SEED_SPEND_VAULT, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID,
};
use crate::core_cpi;
use crate::errors::IntentsError;
use crate::events::{PumpBought, PumpCreated, PumpSold};
use crate::instructions::common;
use crate::policy;
use crate::state::{Paymaster, PumpBuyArgs, PumpCreateArgs, PumpSellArgs, SpendVault};

const IDX_GLOBAL: usize = 0;
const IDX_BASE_MINT: usize = 1;
const IDX_BASE_TOKEN_PROGRAM: usize = 3;
const IDX_BONDING_CURVE: usize = 10;
const IDX_ASSOCIATED_BASE_USER: usize = 14;
const IDX_USER_VOLUME_ACCUMULATOR_BUY: usize = 20;
const IDX_USER_VOLUME_ACCUMULATOR_SELL: usize = 19;
const IDX_EVENT_AUTHORITY_BUY: usize = 25;
const IDX_EVENT_AUTHORITY_SELL: usize = 24;

const IDX_CREATE_MINT_AUTHORITY: usize = 1;
const IDX_CREATE_BONDING_CURVE: usize = 2;
const IDX_CREATE_ASSOCIATED_BONDING_CURVE: usize = 3;
const IDX_CREATE_GLOBAL: usize = 4;
const IDX_CREATE_SYSTEM: usize = 6;
const IDX_CREATE_TOKEN_PROGRAM: usize = 7;
const IDX_CREATE_ATA_PROGRAM: usize = 8;
const IDX_CREATE_MAYHEM_PROGRAM: usize = 9;
const IDX_CREATE_GLOBAL_PARAMS: usize = 10;
const IDX_CREATE_SOL_VAULT: usize = 11;
const IDX_CREATE_MAYHEM_STATE: usize = 12;
const IDX_CREATE_MAYHEM_TOKEN_VAULT: usize = 13;
const IDX_CREATE_EVENT_AUTHORITY: usize = 14;

fn trader_pda(grok: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_PUMP_TRADER, grok.as_ref()], &crate::ID)
}

pub fn buy_handler(ctx: Context<PumpTrade>, args: PumpBuyArgs) -> Result<()> {
    policy::require_pump_buy_amounts(args.amount, args.max_sol_cost)?;
    common::precheck_sponsor(
        args.sponsor_lamports,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
    )?;
    require_agent_not_fee_payer_ctx(&ctx)?;
    policy::require_pump_program(&ctx.accounts.pump_program.key())?;

    let remaining = ctx.remaining_accounts;
    policy::require_pump_account_count(remaining.len(), PUMP_BUY_V2_ACCOUNT_COUNT)?;
    let vault = ctx.accounts.spend_vault.key();
    let grok = ctx.accounts.grok_account.key();
    let (trader, bump_u8) = trader_pda(&grok);
    common::require_pump_trader_ready(&remaining[PUMP_USER_INDEX])?;
    // remaining[13] MUST be trader. Vault-as-user fails (PumpUserMustBeTrader).
    policy::require_pump_user_not_vault(&remaining[PUMP_USER_INDEX].key(), &vault)?;
    policy::require_pump_user_is_trader(&remaining[PUMP_USER_INDEX].key(), &trader)?;
    policy::require_pump_trader_system_owned(remaining[PUMP_USER_INDEX].owner)?;
    policy::require_pump_program(&remaining[PUMP_PROGRAM_INDEX_BUY].key())?;
    validate_pump_pdas(remaining, &trader, true)?;

    let pm_key = ctx.accounts.paymaster.as_ref().map(|p| p.key());
    common::reject_paymaster_in_remaining(remaining, pm_key.as_ref())?;

    // 1. CORE check_grant — abort on error. Grant cap is SOL spent (max_sol_cost).
    core_cpi::check_grant(
        ctx.accounts.grok_chain_core_program.to_account_info(),
        ctx.accounts.grok_account.to_account_info(),
        ctx.accounts.grant.to_account_info(),
        ctx.accounts.agent.to_account_info(),
        ctx.accounts.intents_program.to_account_info(),
        args.max_sol_cost,
    )?;

    // 2. No in-ix vault→trader debit. Trader is remaining[13] only.
    // Raw debit + later pump CPI / a second trader AccountInfo is
    // UnbalancedInstruction. Root pre-funds via fund_pump_trader.
    let rent0 = Rent::get()?.minimum_balance(PUMP_TRADER_SPACE);
    policy::require_pump_trader_prefunded(
        remaining[PUMP_USER_INDEX].lamports(),
        args.max_sol_cost,
        rent0,
    )?;

    // 3. Trader base ATA must already exist. Adapter does not invoke ATA
    // (not invoke_signed as trader for ATA create). Client prepends CreateIdempotent.
    require!(
        !remaining[IDX_ASSOCIATED_BASE_USER].data_is_empty(),
        IntentsError::PumpAtaCreateRequiresFeePayer
    );

    // 4. Official buy_v2 data. Never empty. Never client-supplied raw bytes.
    let data = encode_pump_buy_v2(args.amount, args.max_sol_cost);
    policy::require_nonempty_pump_data(&data)?;
    let ix = build_pump_ix(remaining, &trader, data)?;

    let bump = [bump_u8];
    let trader_signer_seeds: &[&[u8]] = &[SEED_PUMP_TRADER, grok.as_ref(), bump.as_ref()];
    invoke_signed(&ix, remaining, &[trader_signer_seeds])?;

    // 5. Leftover native SOL stays on the trader PDA.
    // Do NOT sweep trader→vault in the same ix (named vault + remaining
    // trader = UnbalancedInstruction). Same as pump_amm.

    common::reimburse_sponsor(
        &ctx.accounts.grant,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
        args.sponsor_lamports,
    )?;

    emit!(PumpBought {
        vault,
        trader,
        mint: remaining[IDX_BASE_MINT].key(),
        amount: args.amount,
        max_sol_cost: args.max_sol_cost,
        agent: ctx.accounts.agent.key(),
        grant: ctx.accounts.grant.key(),
        generation: ctx.accounts.grant.generation,
    });
    Ok(())
}

pub fn sell_handler(ctx: Context<PumpTrade>, args: PumpSellArgs) -> Result<()> {
    policy::require_pump_sell_amounts(args.amount, args.min_sol_output)?;
    common::precheck_sponsor(
        args.sponsor_lamports,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
    )?;
    require_agent_not_fee_payer_ctx(&ctx)?;
    policy::require_pump_program(&ctx.accounts.pump_program.key())?;

    let remaining = ctx.remaining_accounts;
    policy::require_pump_account_count(remaining.len(), PUMP_SELL_V2_ACCOUNT_COUNT)?;
    let vault = ctx.accounts.spend_vault.key();
    let grok = ctx.accounts.grok_account.key();
    let (trader, bump_u8) = trader_pda(&grok);
    common::require_pump_trader_ready(&remaining[PUMP_USER_INDEX])?;
    policy::require_pump_user_not_vault(&remaining[PUMP_USER_INDEX].key(), &vault)?;
    policy::require_pump_user_is_trader(&remaining[PUMP_USER_INDEX].key(), &trader)?;
    policy::require_pump_trader_system_owned(remaining[PUMP_USER_INDEX].owner)?;
    policy::require_pump_program(&remaining[PUMP_PROGRAM_INDEX_SELL].key())?;
    validate_pump_pdas(remaining, &trader, false)?;

    let pm_key = ctx.accounts.paymaster.as_ref().map(|p| p.key());
    common::reject_paymaster_in_remaining(remaining, pm_key.as_ref())?;

    // 1. check_grant(0) — sell spends tokens, not SOL. See PUMP_SELL_CHECK_GRANT_AMOUNT.
    core_cpi::check_grant(
        ctx.accounts.grok_chain_core_program.to_account_info(),
        ctx.accounts.grok_account.to_account_info(),
        ctx.accounts.grant.to_account_info(),
        ctx.accounts.agent.to_account_info(),
        ctx.accounts.intents_program.to_account_info(),
        policy::PUMP_SELL_CHECK_GRANT_AMOUNT,
    )?;

    // 2. Official sell_v2 data. Never empty. No ATA create (seller already holds tokens).
    let data = encode_pump_sell_v2(args.amount, args.min_sol_output);
    policy::require_nonempty_pump_data(&data)?;
    let ix = build_pump_ix(remaining, &trader, data)?;

    let bump = [bump_u8];
    let trader_signer_seeds: &[&[u8]] = &[SEED_PUMP_TRADER, grok.as_ref(), bump.as_ref()];
    invoke_signed(&ix, remaining, &[trader_signer_seeds])?;

    // 3. Sale SOL stays on the trader PDA. Do NOT sweep trader→vault
    // in the same ix (named vault + remaining trader = UnbalancedInstruction).

    common::reimburse_sponsor(
        &ctx.accounts.grant,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
        args.sponsor_lamports,
    )?;

    emit!(PumpSold {
        vault,
        trader,
        mint: remaining[IDX_BASE_MINT].key(),
        amount: args.amount,
        min_sol_output: args.min_sol_output,
        agent: ctx.accounts.agent.key(),
        grant: ctx.accounts.grant.key(),
        generation: ctx.accounts.grant.generation,
    });
    Ok(())
}

pub fn create_handler(ctx: Context<PumpTrade>, args: PumpCreateArgs) -> Result<()> {
    policy::require_pump_create_strings(&args.name, &args.symbol, &args.uri)?;
    policy::require_pump_create_amounts(args.max_sol_cost)?;
    common::precheck_sponsor(
        args.sponsor_lamports,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
    )?;
    require_agent_not_fee_payer_ctx(&ctx)?;
    policy::require_pump_program(&ctx.accounts.pump_program.key())?;

    let remaining = ctx.remaining_accounts;
    policy::require_pump_create_account_count(remaining.len())?;
    let vault = ctx.accounts.spend_vault.key();
    let grok = ctx.accounts.grok_account.key();
    let (trader, bump_u8) = trader_pda(&grok);
    common::require_pump_trader_ready(&remaining[PUMP_CREATE_USER_INDEX])?;
    // remaining[5] MUST be trader. Vault-as-user fails (PumpUserMustBeTrader).
    policy::require_pump_user_not_vault(&remaining[PUMP_CREATE_USER_INDEX].key(), &vault)?;
    policy::require_pump_user_is_trader(&remaining[PUMP_CREATE_USER_INDEX].key(), &trader)?;
    policy::require_pump_trader_system_owned(remaining[PUMP_CREATE_USER_INDEX].owner)?;
    // remaining[0] mint MUST be a client-signed Token-2022 keypair.
    policy::require_pump_mint_is_signer(remaining[PUMP_CREATE_MINT_INDEX].is_signer)?;
    policy::require_pump_program(&remaining[PUMP_CREATE_PROGRAM_INDEX].key())?;
    validate_pump_create_pdas(remaining)?;

    let pm_key = ctx.accounts.paymaster.as_ref().map(|p| p.key());
    common::reject_paymaster_in_remaining(remaining, pm_key.as_ref())?;

    // 1. CORE check_grant — abort on error. Grant cap is SOL budget (rent + fees).
    core_cpi::check_grant(
        ctx.accounts.grok_chain_core_program.to_account_info(),
        ctx.accounts.grok_account.to_account_info(),
        ctx.accounts.grant.to_account_info(),
        ctx.accounts.agent.to_account_info(),
        ctx.accounts.intents_program.to_account_info(),
        args.max_sol_cost,
    )?;

    // 2. No in-ix vault→trader debit. Trader is remaining[5] only.
    // Same UnbalancedInstruction hazard as pump_buy. Prefund path only.
    let rent0 = Rent::get()?.minimum_balance(PUMP_TRADER_SPACE);
    policy::require_pump_trader_prefunded(
        remaining[PUMP_CREATE_USER_INDEX].lamports(),
        args.max_sol_cost,
        rent0,
    )?;

    // 3. Official create_v2 data. Creator is grok_account.root. Never empty.
    let creator = ctx.accounts.grok_account.root;
    let data = encode_pump_create_v2(
        &args.name,
        &args.symbol,
        &args.uri,
        &creator,
        args.is_mayhem_mode,
        args.is_cashback_enabled,
    );
    policy::require_nonempty_pump_create_data(&data)?;
    let ix = build_pump_create_ix(remaining, &trader, data)?;

    let bump = [bump_u8];
    let trader_signer_seeds: &[&[u8]] = &[SEED_PUMP_TRADER, grok.as_ref(), bump.as_ref()];
    invoke_signed(&ix, remaining, &[trader_signer_seeds])?;

    // 4. Leftover native SOL stays on the trader PDA. Do NOT sweep.

    common::reimburse_sponsor(
        &ctx.accounts.grant,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
        args.sponsor_lamports,
    )?;

    emit!(PumpCreated {
        vault,
        trader,
        mint: remaining[PUMP_CREATE_MINT_INDEX].key(),
        creator,
        max_sol_cost: args.max_sol_cost,
        is_mayhem_mode: args.is_mayhem_mode,
        is_cashback_enabled: args.is_cashback_enabled,
        agent: ctx.accounts.agent.key(),
        grant: ctx.accounts.grant.key(),
        generation: ctx.accounts.grant.generation,
    });
    Ok(())
}

fn require_agent_not_fee_payer_ctx(ctx: &Context<PumpTrade>) -> Result<()> {
    let fp = ctx.accounts.fee_payer.as_ref().map(|f| f.key());
    policy::require_agent_not_fee_payer(&ctx.accounts.agent.key(), fp.as_ref())
}

/// Rebuild a pump ix: hardcoded program id, constructed data, user forced to trader.
/// Does not invoke_signed as trader for an arbitrary remaining blob — count,
/// user slot, program slot, and PDAs are checked first. program_id is never
/// taken from the client. Vault is never written into the user slot.
fn build_pump_ix(remaining: &[AccountInfo], trader: &Pubkey, data: Vec<u8>) -> Result<Instruction> {
    require!(!data.is_empty(), IntentsError::PumpEmptyDataForbidden);
    require!(
        data.len() == PUMP_TRADE_IX_DATA_LEN,
        IntentsError::PumpEmptyDataForbidden
    );
    let mut accounts = Vec::with_capacity(remaining.len());
    for (i, acc) in remaining.iter().enumerate() {
        if i == PUMP_USER_INDEX {
            require_keys_eq!(acc.key(), *trader, IntentsError::PumpUserMustBeTrader);
            accounts.push(AccountMeta::new(*trader, true));
        } else if acc.is_writable {
            accounts.push(AccountMeta::new(*acc.key, acc.is_signer));
        } else {
            accounts.push(AccountMeta::new_readonly(*acc.key, acc.is_signer));
        }
    }
    Ok(Instruction {
        program_id: PUMP_PROGRAM_ID,
        accounts,
        data,
    })
}

/// Official PDAs re-derived against `trader` (ATA owner + UVA seed).
fn validate_pump_pdas(remaining: &[AccountInfo], trader: &Pubkey, is_buy: bool) -> Result<()> {
    let mint = remaining[IDX_BASE_MINT].key();
    let (global, _) = Pubkey::find_program_address(&[b"global"], &PUMP_PROGRAM_ID);
    require_keys_eq!(
        remaining[IDX_GLOBAL].key(),
        global,
        IntentsError::PumpPdaMismatch
    );

    let (curve, _) =
        Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &PUMP_PROGRAM_ID);
    require_keys_eq!(
        remaining[IDX_BONDING_CURVE].key(),
        curve,
        IntentsError::PumpPdaMismatch
    );

    let uva_idx = if is_buy {
        IDX_USER_VOLUME_ACCUMULATOR_BUY
    } else {
        IDX_USER_VOLUME_ACCUMULATOR_SELL
    };
    let (uva, _) = Pubkey::find_program_address(
        &[b"user_volume_accumulator", trader.as_ref()],
        &PUMP_PROGRAM_ID,
    );
    require_keys_eq!(remaining[uva_idx].key(), uva, IntentsError::PumpPdaMismatch);

    let ea_idx = if is_buy {
        IDX_EVENT_AUTHORITY_BUY
    } else {
        IDX_EVENT_AUTHORITY_SELL
    };
    let (ea, _) = Pubkey::find_program_address(&[b"__event_authority"], &PUMP_PROGRAM_ID);
    require_keys_eq!(remaining[ea_idx].key(), ea, IntentsError::PumpPdaMismatch);

    let token_program = remaining[IDX_BASE_TOKEN_PROGRAM].key();
    require!(
        token_program == TOKEN_2022_PROGRAM_ID || token_program == TOKEN_PROGRAM_ID,
        IntentsError::InvalidTokenProgram
    );
    let (ata, _) = Pubkey::find_program_address(
        &[trader.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    require_keys_eq!(
        remaining[IDX_ASSOCIATED_BASE_USER].key(),
        ata,
        IntentsError::PumpPdaMismatch
    );
    Ok(())
}


/// Rebuild official create_v2: hardcoded program id, constructed data.
/// Mint (0) forced writable+signer. User (5) forced to trader writable+signer.
fn build_pump_create_ix(
    remaining: &[AccountInfo],
    trader: &Pubkey,
    data: Vec<u8>,
) -> Result<Instruction> {
    policy::require_nonempty_pump_create_data(&data)?;
    let mut accounts = Vec::with_capacity(remaining.len());
    for (i, acc) in remaining.iter().enumerate() {
        if i == PUMP_CREATE_MINT_INDEX {
            require!(acc.is_signer, IntentsError::PumpMintMustBeSigner);
            accounts.push(AccountMeta::new(*acc.key, true));
        } else if i == PUMP_CREATE_USER_INDEX {
            require_keys_eq!(acc.key(), *trader, IntentsError::PumpUserMustBeTrader);
            accounts.push(AccountMeta::new(*trader, true));
        } else if acc.is_writable {
            accounts.push(AccountMeta::new(*acc.key, acc.is_signer));
        } else {
            accounts.push(AccountMeta::new_readonly(*acc.key, acc.is_signer));
        }
    }
    Ok(Instruction {
        program_id: PUMP_PROGRAM_ID,
        accounts,
        data,
    })
}

/// Official create_v2 PDAs re-derived (pump + mayhem + Token-2022 ATAs).
fn validate_pump_create_pdas(remaining: &[AccountInfo]) -> Result<()> {
    let mint = remaining[PUMP_CREATE_MINT_INDEX].key();

    let (mint_authority, _) =
        Pubkey::find_program_address(&[b"mint-authority"], &PUMP_PROGRAM_ID);
    require_keys_eq!(
        remaining[IDX_CREATE_MINT_AUTHORITY].key(),
        mint_authority,
        IntentsError::PumpPdaMismatch
    );

    let (curve, _) =
        Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &PUMP_PROGRAM_ID);
    require_keys_eq!(
        remaining[IDX_CREATE_BONDING_CURVE].key(),
        curve,
        IntentsError::PumpPdaMismatch
    );

    let (global, _) = Pubkey::find_program_address(&[b"global"], &PUMP_PROGRAM_ID);
    require_keys_eq!(
        remaining[IDX_CREATE_GLOBAL].key(),
        global,
        IntentsError::PumpPdaMismatch
    );

    require_keys_eq!(
        remaining[IDX_CREATE_SYSTEM].key(),
        anchor_lang::solana_program::system_program::ID,
        IntentsError::PumpPdaMismatch
    );
    require_keys_eq!(
        remaining[IDX_CREATE_TOKEN_PROGRAM].key(),
        TOKEN_2022_PROGRAM_ID,
        IntentsError::InvalidTokenProgram
    );
    require_keys_eq!(
        remaining[IDX_CREATE_ATA_PROGRAM].key(),
        ASSOCIATED_TOKEN_PROGRAM_ID,
        IntentsError::PumpPdaMismatch
    );
    require_keys_eq!(
        remaining[IDX_CREATE_MAYHEM_PROGRAM].key(),
        MAYHEM_PROGRAM_ID,
        IntentsError::PumpPdaMismatch
    );

    let (ata, _) = Pubkey::find_program_address(
        &[curve.as_ref(), TOKEN_2022_PROGRAM_ID.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    require_keys_eq!(
        remaining[IDX_CREATE_ASSOCIATED_BONDING_CURVE].key(),
        ata,
        IntentsError::PumpPdaMismatch
    );

    let (global_params, _) =
        Pubkey::find_program_address(&[b"global-params"], &MAYHEM_PROGRAM_ID);
    require_keys_eq!(
        remaining[IDX_CREATE_GLOBAL_PARAMS].key(),
        global_params,
        IntentsError::PumpPdaMismatch
    );

    let (sol_vault, _) = Pubkey::find_program_address(&[b"sol-vault"], &MAYHEM_PROGRAM_ID);
    require_keys_eq!(
        remaining[IDX_CREATE_SOL_VAULT].key(),
        sol_vault,
        IntentsError::PumpPdaMismatch
    );

    let (mayhem_state, _) =
        Pubkey::find_program_address(&[b"mayhem-state", mint.as_ref()], &MAYHEM_PROGRAM_ID);
    require_keys_eq!(
        remaining[IDX_CREATE_MAYHEM_STATE].key(),
        mayhem_state,
        IntentsError::PumpPdaMismatch
    );

    let (mayhem_token_vault, _) = Pubkey::find_program_address(
        &[
            sol_vault.as_ref(),
            TOKEN_2022_PROGRAM_ID.as_ref(),
            mint.as_ref(),
        ],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    require_keys_eq!(
        remaining[IDX_CREATE_MAYHEM_TOKEN_VAULT].key(),
        mayhem_token_vault,
        IntentsError::PumpPdaMismatch
    );

    let (ea, _) = Pubkey::find_program_address(&[b"__event_authority"], &PUMP_PROGRAM_ID);
    require_keys_eq!(
        remaining[IDX_CREATE_EVENT_AUTHORITY].key(),
        ea,
        IntentsError::PumpPdaMismatch
    );
    Ok(())
}

#[derive(Accounts)]
pub struct PumpTrade<'info> {
    /// Grant agent. Must sign INTENTS. Never the fee payer / SOL source.
    pub agent: Signer<'info>,
    #[account(
        seeds = [SEED_GROK_ACCOUNT, grok_account.root.as_ref()],
        bump = grok_account.bump,
        seeds::program = grok_chain_core::ID,
    )]
    pub grok_account: Account<'info, GrokAccount>,
    #[account(
        mut,
        seeds = [SEED_GRANT, grok_account.key().as_ref(), agent.key().as_ref()],
        bump = grant.bump,
        seeds::program = grok_chain_core::ID,
        constraint = grant.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = grant.agent == agent.key() @ IntentsError::AgentMismatch,
    )]
    pub grant: Account<'info, Grant>,
    pub grok_chain_core_program: Program<'info, grok_chain_core::program::GrokChainCore>,
    /// CHECK: this program (router). CORE allowlists this id.
    #[account(address = crate::ID)]
    pub intents_program: UncheckedAccount<'info>,
    /// Program-owned custody. Not debited in pump_buy / pump_sell / pump_create.
    /// Never pump `user`. Root fund_pump_trader is a separate ix.
    #[account(
        mut,
        seeds = [SEED_SPEND_VAULT, grok_account.key().as_ref()],
        bump = spend_vault.bump,
        constraint = spend_vault.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = spend_vault.root == grok_account.root @ IntentsError::UnauthorizedRoot,
    )]
    pub spend_vault: Account<'info, SpendVault>,
    /// Trader is remaining[13] (buy/sell) or remaining[5] (create) only.
    /// Named+remaining duplicate of the same writable system account
    /// makes try_debit / later system CPI UnbalancedInstruction.
    pub system_program: Program<'info, System>,
    /// CHECK: hardcoded allowlist. Only program this adapter CPIs into.
    #[account(address = crate::PUMP_PROGRAM_ID, executable)]
    pub pump_program: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [SEED_PAYMASTER, grok_account.key().as_ref()],
        bump = paymaster.bump,
        constraint = paymaster.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = paymaster.root == grok_account.root @ IntentsError::UnauthorizedRoot,
    )]
    pub paymaster: Option<Account<'info, Paymaster>>,
    /// Relayer / outer fee payer. Pays ATA rent. Must not be the agent.
    #[account(
        mut,
        constraint = fee_payer.key() != agent.key() @ IntentsError::AgentCannotFeePay
    )]
    pub fee_payer: Option<Signer<'info>>,
}
