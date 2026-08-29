//! Tight PumpSwap AMM adapter (official post-graduation program).
//!
//! Inner program is hardcoded to `pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA`.
//! Instruction data is constructed here (`buy_exact_quote_in` / `sell` disc +
//! typed args) and is never forwarded from the client as raw bytes.
//!
//! PumpSwap `user` is the system-owned pump-trader PDA
//! (seeds `[b"pump-trader", grok_account]`). SpendVault is never user.
//! invoke_signed uses trader seeds only.
//!
//! Official buy layout (idl/pump_amm.json + docs/BREAKING_FEE_RECIPIENT.md):
//! 23 IDL accounts + remaining `pool-v2` + breaking fee recipient +
//! recipient quote ATA = 26 (non-cashback). Cashback inserts the UVA
//! quote ATA before `pool-v2` (27).
//!
//! Official sell layout (idl/pump_amm.json — there is no sell_v2):
//! 21 IDL accounts (no global_volume / user_volume) + pool-v2 + breaking
//! fee + recipient quote ATA = 24. Sell IDL has no cashback remaining
//! and no UVA. Do NOT accept buy's 26/27 shape (volume accs would shift
//! fee_config). Passing buy's 26-account order to sell mis-binds
//! fee_config to global_volume and will fail on-chain.
//!
//! Quote is WSOL. Buy: after check_grant we wrap spendable_quote_in onto
//! the trader WSOL ATA (system transfer + SyncNative), CPI the AMM.
//! Sell: CPI the AMM (base tokens in). Do not wrap SOL. Do NOT sweep
//! trader→vault in the same ix (named vault + remaining trader =
//! UnbalancedInstruction). Quote WSOL stays on the trader ATA.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    system_instruction,
};

use grok_chain_core::{Grant, GrokAccount, SEED_GRANT, SEED_GROK_ACCOUNT};

use crate::constants::{
    encode_pump_amm_buy_exact_quote_in, encode_pump_amm_sell, ASSOCIATED_TOKEN_PROGRAM_ID,
    PUMP_AMM_BUY_ACCOUNT_COUNT, PUMP_AMM_BUY_ACCOUNT_COUNT_CASHBACK,
    PUMP_AMM_BUY_EXACT_QUOTE_IN_DATA_LEN, PUMP_AMM_FEE_PROGRAM_ID, PUMP_AMM_PROGRAM_ID,
    PUMP_AMM_PROGRAM_INDEX, PUMP_AMM_SELL_ACCOUNT_COUNT, PUMP_AMM_SELL_DATA_LEN,
    PUMP_AMM_USER_INDEX, PUMP_TRADER_SPACE, SEED_PAYMASTER,
    SEED_PUMP_TRADER, SEED_SPEND_VAULT, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID, WSOL_MINT,
};
use crate::core_cpi;
use crate::errors::IntentsError;
use crate::events::{PumpAmmBought, PumpAmmSold};
use crate::instructions::common;
use crate::policy;
use crate::state::{Paymaster, PumpAmmBuyArgs, PumpAmmSellArgs, SpendVault};

const IDX_POOL: usize = 0;
const IDX_USER: usize = 1;
const IDX_GLOBAL_CONFIG: usize = 2;
const IDX_BASE_MINT: usize = 3;
const IDX_QUOTE_MINT: usize = 4;
const IDX_USER_BASE: usize = 5;
const IDX_USER_QUOTE: usize = 6;
const IDX_POOL_BASE: usize = 7;
const IDX_POOL_QUOTE: usize = 8;
const IDX_PROTOCOL_FEE_RECIPIENT: usize = 9;
const IDX_PROTOCOL_FEE_RECIPIENT_ATA: usize = 10;
const IDX_BASE_TOKEN_PROGRAM: usize = 11;
const IDX_QUOTE_TOKEN_PROGRAM: usize = 12;
const IDX_SYSTEM: usize = 13;
const IDX_ATA_PROGRAM: usize = 14;
const IDX_EVENT_AUTHORITY: usize = 15;
const IDX_PROGRAM: usize = 16;
const IDX_CREATOR_VAULT_ATA: usize = 17;
const IDX_CREATOR_VAULT_AUTH: usize = 18;
const IDX_GLOBAL_VOLUME: usize = 19;
const IDX_USER_VOLUME: usize = 20;
const IDX_FEE_CONFIG_BUY: usize = 21;
const IDX_FEE_PROGRAM_BUY: usize = 22;
/// Official sell IDL: no volume accumulators. fee_config/fee_program shift up.
const IDX_FEE_CONFIG_SELL: usize = 19;
const IDX_FEE_PROGRAM_SELL: usize = 20;

/// SPL Token `SyncNative` (instruction 17).
const TOKEN_SYNC_NATIVE: u8 = 17;

pub fn buy_handler(ctx: Context<PumpAmmTrade>, args: PumpAmmBuyArgs) -> Result<()> {
    policy::require_pump_amm_buy_amounts(
        args.spendable_quote_in,
        args.min_base_amount_out,
        args.max_sol_cost,
    )?;
    common::precheck_sponsor(
        args.sponsor_lamports,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
    )?;
    require_agent_not_fee_payer_ctx(&ctx)?;
    policy::require_pump_amm_program(&ctx.accounts.pump_amm_program.key())?;
    policy::require_pump_amm_account_count(ctx.remaining_accounts.len())?;
    let vault = ctx.accounts.spend_vault.key();
    let grok = ctx.accounts.grok_account.key();
    let (trader, bump_u8) = Pubkey::find_program_address(
        &[SEED_PUMP_TRADER, grok.as_ref()],
        &crate::ID,
    );
    common::require_pump_trader_ready(&ctx.remaining_accounts[IDX_USER])?;
    policy::require_pump_user_not_vault(&ctx.remaining_accounts[IDX_USER].key(), &vault)?;
    policy::require_pump_user_is_trader(&ctx.remaining_accounts[IDX_USER].key(), &trader)?;
    policy::require_pump_trader_system_owned(ctx.remaining_accounts[IDX_USER].owner)?;
    policy::require_pump_amm_program(&ctx.remaining_accounts[IDX_PROGRAM].key())?;
    validate_pump_amm_accounts(ctx.remaining_accounts, &trader, false)?;

    let pm_key = ctx.accounts.paymaster.as_ref().map(|p| p.key());
    common::reject_paymaster_in_remaining(ctx.remaining_accounts, pm_key.as_ref())?;

    // 1. CORE check_grant — abort on error. Grant cap is SOL spent (max_sol_cost).
    core_cpi::check_grant(
        ctx.accounts.grok_chain_core_program.to_account_info(),
        ctx.accounts.grok_account.to_account_info(),
        ctx.accounts.grant.to_account_info(),
        ctx.accounts.agent.to_account_info(),
        ctx.accounts.intents_program.to_account_info(),
        args.max_sol_cost,
    )?;

    // 2. No in-ix vault→trader debit. Trader is remaining[1] only.
    // Raw debit + later system CPI / a second trader AccountInfo is
    // UnbalancedInstruction. Root pre-funds via fund_pump_trader.
    let rent0 = Rent::get()?.minimum_balance(PUMP_TRADER_SPACE);
    policy::require_pump_trader_prefunded(
        ctx.remaining_accounts[IDX_USER].lamports(),
        args.spendable_quote_in,
        rent0,
    )?;

    // 3. Trader base ATA and WSOL ATA must already exist. Adapter does not
    // invoke ATA create. Client prepends CreateIdempotent (relayer payer).
    require!(
        !ctx.remaining_accounts[IDX_USER_BASE].data_is_empty(),
        IntentsError::PumpAtaCreateRequiresFeePayer
    );
    require!(
        !ctx.remaining_accounts[IDX_USER_QUOTE].data_is_empty(),
        IntentsError::PumpAtaCreateRequiresFeePayer
    );

    let bump = [bump_u8];
    let trader_signer_seeds: &[&[u8]] = &[SEED_PUMP_TRADER, grok.as_ref(), bump.as_ref()];

    // 4. Wrap spendable_quote_in native SOL onto trader WSOL ATA.
    // All four accounts come from remaining (same 'info) so we do not mix
    // Context remaining vs Accounts lifetimes.
    wrap_sol_to_wsol(
        &ctx.remaining_accounts[IDX_USER],
        &ctx.remaining_accounts[IDX_USER_QUOTE],
        &ctx.remaining_accounts[IDX_SYSTEM],
        &ctx.remaining_accounts[IDX_QUOTE_TOKEN_PROGRAM],
        args.spendable_quote_in,
        trader_signer_seeds,
    )?;

    // 5. Official buy_exact_quote_in. Never empty. Never client raw bytes.
    let data = encode_pump_amm_buy_exact_quote_in(
        args.spendable_quote_in,
        args.min_base_amount_out,
        true,
    );
    policy::require_nonempty_pump_amm_data(&data)?;
    let ix = build_pump_amm_ix(ctx.remaining_accounts, &trader, data)?;
    invoke_signed(&ix, ctx.remaining_accounts, &[trader_signer_seeds])?;

    // 6. Leftover native SOL stays on the trader PDA (system-owned).
    // Sweep would mix remaining trader with named vault AccountInfo and
    // fail to compile. Client sets max_sol_cost == spendable_quote_in so
    // wrap consumes the debit and leftover is 0-byte rent only.
    common::reimburse_sponsor(
        &ctx.accounts.grant,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
        args.sponsor_lamports,
    )?;

    emit!(PumpAmmBought {
        vault,
        trader,
        mint: ctx.remaining_accounts[IDX_BASE_MINT].key(),
        pool: ctx.remaining_accounts[IDX_POOL].key(),
        spendable_quote_in: args.spendable_quote_in,
        min_base_amount_out: args.min_base_amount_out,
        max_sol_cost: args.max_sol_cost,
        agent: ctx.accounts.agent.key(),
        grant: ctx.accounts.grant.key(),
        generation: ctx.accounts.grant.generation,
    });
    Ok(())
}

pub fn sell_handler(ctx: Context<PumpAmmTrade>, args: PumpAmmSellArgs) -> Result<()> {
    policy::require_pump_amm_sell_amounts(args.base_amount_in, args.min_quote_amount_out)?;
    common::precheck_sponsor(
        args.sponsor_lamports,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
    )?;
    require_agent_not_fee_payer_ctx(&ctx)?;
    policy::require_pump_amm_program(&ctx.accounts.pump_amm_program.key())?;
    policy::require_pump_amm_sell_account_count(ctx.remaining_accounts.len())?;
    let vault = ctx.accounts.spend_vault.key();
    let grok = ctx.accounts.grok_account.key();
    let (trader, bump_u8) = Pubkey::find_program_address(
        &[SEED_PUMP_TRADER, grok.as_ref()],
        &crate::ID,
    );
    common::require_pump_trader_ready(&ctx.remaining_accounts[IDX_USER])?;
    policy::require_pump_user_not_vault(&ctx.remaining_accounts[IDX_USER].key(), &vault)?;
    policy::require_pump_user_is_trader(&ctx.remaining_accounts[IDX_USER].key(), &trader)?;
    policy::require_pump_trader_system_owned(ctx.remaining_accounts[IDX_USER].owner)?;
    policy::require_pump_amm_program(&ctx.remaining_accounts[IDX_PROGRAM].key())?;
    validate_pump_amm_accounts(ctx.remaining_accounts, &trader, true)?;

    let pm_key = ctx.accounts.paymaster.as_ref().map(|p| p.key());
    common::reject_paymaster_in_remaining(ctx.remaining_accounts, pm_key.as_ref())?;

    // 1. check_grant(0). Sell spends base tokens and RECEIVES quote.
    // Grant is a SOL-spent cap. Same honesty as bonding-curve PUMP_SELL_CHECK_GRANT_AMOUNT.
    policy::require_pump_amm_sell_check_grant_amount(policy::PUMP_AMM_SELL_CHECK_GRANT_AMOUNT)?;
    core_cpi::check_grant(
        ctx.accounts.grok_chain_core_program.to_account_info(),
        ctx.accounts.grok_account.to_account_info(),
        ctx.accounts.grant.to_account_info(),
        ctx.accounts.agent.to_account_info(),
        ctx.accounts.intents_program.to_account_info(),
        policy::PUMP_AMM_SELL_CHECK_GRANT_AMOUNT,
    )?;

    // 2. No in-ix vault debit. Trader is remaining[1] only. Seller already
    // holds base tokens on remaining[5]. WSOL ATA must exist to receive quote.
    // Prefund check is buy-only (spendable quote). Sell does not prefund.
    require!(
        !ctx.remaining_accounts[IDX_USER_BASE].data_is_empty(),
        IntentsError::PumpAtaCreateRequiresFeePayer
    );
    require!(
        !ctx.remaining_accounts[IDX_USER_QUOTE].data_is_empty(),
        IntentsError::PumpAtaCreateRequiresFeePayer
    );

    let bump = [bump_u8];
    let trader_signer_seeds: &[&[u8]] = &[SEED_PUMP_TRADER, grok.as_ref(), bump.as_ref()];

    // 3. Official sell. Never empty. Never client raw bytes.
    let data = encode_pump_amm_sell(args.base_amount_in, args.min_quote_amount_out);
    policy::require_nonempty_pump_amm_sell_data(&data)?;
    let ix = build_pump_amm_ix(ctx.remaining_accounts, &trader, data)?;
    invoke_signed(&ix, ctx.remaining_accounts, &[trader_signer_seeds])?;

    // 4. Do not wrap SOL. Do NOT sweep trader → vault in this ix
    // (same UnbalancedInstruction reason as buy). Quote WSOL stays on
    // the trader ATA.

    common::reimburse_sponsor(
        &ctx.accounts.grant,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
        args.sponsor_lamports,
    )?;

    emit!(PumpAmmSold {
        vault,
        trader,
        mint: ctx.remaining_accounts[IDX_BASE_MINT].key(),
        pool: ctx.remaining_accounts[IDX_POOL].key(),
        base_amount_in: args.base_amount_in,
        min_quote_amount_out: args.min_quote_amount_out,
        agent: ctx.accounts.agent.key(),
        grant: ctx.accounts.grant.key(),
        generation: ctx.accounts.grant.generation,
    });
    Ok(())
}

fn require_agent_not_fee_payer_ctx(ctx: &Context<PumpAmmTrade>) -> Result<()> {
    let fp = ctx.accounts.fee_payer.as_ref().map(|f| f.key());
    policy::require_agent_not_fee_payer(&ctx.accounts.agent.key(), fp.as_ref())
}

fn wrap_sol_to_wsol<'info>(
    trader: &AccountInfo<'info>,
    wsol_ata: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    quote_token_program: &AccountInfo<'info>,
    amount: u64,
    trader_signer_seeds: &[&[u8]],
) -> Result<()> {
    require_keys_eq!(
        *quote_token_program.key,
        TOKEN_PROGRAM_ID,
        IntentsError::InvalidTokenProgram
    );
    invoke_signed(
        &system_instruction::transfer(trader.key, wsol_ata.key, amount),
        &[trader.clone(), wsol_ata.clone(), system_program.clone()],
        &[trader_signer_seeds],
    )?;
    let sync_ix = Instruction {
        program_id: TOKEN_PROGRAM_ID,
        accounts: vec![AccountMeta::new(*wsol_ata.key, false)],
        data: vec![TOKEN_SYNC_NATIVE],
    };
    invoke(&sync_ix, &[wsol_ata.clone()])?;
    Ok(())
}

/// Unused on purpose. Named vault + remaining trader is UnbalancedInstruction.
/// Buy leftover and sell proceeds stay on the trader PDA.
#[allow(dead_code)]
fn sweep_trader_to_vault<'info>(
    trader: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    trader_signer_seeds: &[&[u8]],
    spend_vault: &Account<'info, SpendVault>,
) -> Result<()> {
    let rent0 = Rent::get()?.minimum_balance(PUMP_TRADER_SPACE);
    let lamports = trader.lamports();
    if lamports <= rent0 {
        return Ok(());
    }
    let leftover = lamports
        .checked_sub(rent0)
        .ok_or(error!(IntentsError::LamportOverflow))?;
    let vault_ai = spend_vault.to_account_info();
    invoke_signed(
        &system_instruction::transfer(trader.key, vault_ai.key, leftover),
        &[trader.clone(), vault_ai, system_program.clone()],
        &[trader_signer_seeds],
    )?;
    Ok(())
}

fn build_pump_amm_ix(
    remaining: &[AccountInfo],
    trader: &Pubkey,
    data: Vec<u8>,
) -> Result<Instruction> {
    require!(!data.is_empty(), IntentsError::PumpEmptyDataForbidden);
    require!(
        data.len() == PUMP_AMM_BUY_EXACT_QUOTE_IN_DATA_LEN
            || data.len() == PUMP_AMM_SELL_DATA_LEN,
        IntentsError::PumpEmptyDataForbidden
    );
    let mut accounts = Vec::with_capacity(remaining.len());
    for (i, acc) in remaining.iter().enumerate() {
        if i == PUMP_AMM_USER_INDEX {
            require_keys_eq!(acc.key(), *trader, IntentsError::PumpUserMustBeTrader);
            accounts.push(AccountMeta::new(*trader, true));
        } else if acc.is_writable {
            accounts.push(AccountMeta::new(*acc.key, acc.is_signer));
        } else {
            accounts.push(AccountMeta::new_readonly(*acc.key, acc.is_signer));
        }
    }
    Ok(Instruction {
        program_id: PUMP_AMM_PROGRAM_ID,
        accounts,
        data,
    })
}

fn validate_pump_amm_accounts(
    remaining: &[AccountInfo],
    trader: &Pubkey,
    is_sell: bool,
) -> Result<()> {
    let pool_ai = &remaining[IDX_POOL];
    require_keys_eq!(
        *pool_ai.owner,
        PUMP_AMM_PROGRAM_ID,
        IntentsError::PumpAmmPoolInvalid
    );
    require!(
        pool_ai.data_len() >= 245,
        IntentsError::PumpAmmPoolInvalid
    );
    let pool = pool_ai.try_borrow_data()?;
    // Official Pool (idl/pump_amm.json) after 8-byte disc:
    // bump u8, index u16, creator 32, base_mint 32, quote_mint 32, lp_mint 32,
    // pool_base 32, pool_quote 32, lp_supply u64, coin_creator 32, flags…
    let base_mint = Pubkey::try_from(&pool[43..75]).unwrap();
    let quote_mint = Pubkey::try_from(&pool[75..107]).unwrap();
    let pool_base = Pubkey::try_from(&pool[139..171]).unwrap();
    let pool_quote = Pubkey::try_from(&pool[171..203]).unwrap();
    let coin_creator = Pubkey::try_from(&pool[211..243]).unwrap();
    drop(pool);

    require_keys_eq!(
        remaining[IDX_BASE_MINT].key(),
        base_mint,
        IntentsError::PumpAmmPoolInvalid
    );
    require_keys_eq!(
        remaining[IDX_QUOTE_MINT].key(),
        quote_mint,
        IntentsError::PumpAmmPoolInvalid
    );
    require_keys_eq!(quote_mint, WSOL_MINT, IntentsError::PumpAmmQuoteMustBeWsol);
    require_keys_eq!(
        remaining[IDX_POOL_BASE].key(),
        pool_base,
        IntentsError::PumpPdaMismatch
    );
    require_keys_eq!(
        remaining[IDX_POOL_QUOTE].key(),
        pool_quote,
        IntentsError::PumpPdaMismatch
    );

    let (global_config, _) =
        Pubkey::find_program_address(&[b"global_config"], &PUMP_AMM_PROGRAM_ID);
    require_keys_eq!(
        remaining[IDX_GLOBAL_CONFIG].key(),
        global_config,
        IntentsError::PumpPdaMismatch
    );

    let (ea, _) = Pubkey::find_program_address(&[b"__event_authority"], &PUMP_AMM_PROGRAM_ID);
    require_keys_eq!(
        remaining[IDX_EVENT_AUTHORITY].key(),
        ea,
        IntentsError::PumpPdaMismatch
    );

    let (cva, _) =
        Pubkey::find_program_address(&[b"creator_vault", coin_creator.as_ref()], &PUMP_AMM_PROGRAM_ID);
    require_keys_eq!(
        remaining[IDX_CREATOR_VAULT_AUTH].key(),
        cva,
        IntentsError::PumpPdaMismatch
    );

    let quote_token_program = remaining[IDX_QUOTE_TOKEN_PROGRAM].key();
    require_keys_eq!(
        quote_token_program,
        TOKEN_PROGRAM_ID,
        IntentsError::InvalidTokenProgram
    );
    let (creator_ata, _) = Pubkey::find_program_address(
        &[cva.as_ref(), quote_token_program.as_ref(), quote_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    require_keys_eq!(
        remaining[IDX_CREATOR_VAULT_ATA].key(),
        creator_ata,
        IntentsError::PumpPdaMismatch
    );

    let base_token_program = remaining[IDX_BASE_TOKEN_PROGRAM].key();
    require!(
        base_token_program == TOKEN_2022_PROGRAM_ID || base_token_program == TOKEN_PROGRAM_ID,
        IntentsError::InvalidTokenProgram
    );
    let (user_base, _) = Pubkey::find_program_address(
        &[trader.as_ref(), base_token_program.as_ref(), base_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    require_keys_eq!(
        remaining[IDX_USER_BASE].key(),
        user_base,
        IntentsError::PumpPdaMismatch
    );
    let (user_quote, _) = Pubkey::find_program_address(
        &[
            trader.as_ref(),
            quote_token_program.as_ref(),
            quote_mint.as_ref(),
        ],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    require_keys_eq!(
        remaining[IDX_USER_QUOTE].key(),
        user_quote,
        IntentsError::PumpPdaMismatch
    );

    require_keys_eq!(
        remaining[IDX_SYSTEM].key(),
        anchor_lang::solana_program::system_program::ID,
        IntentsError::PumpPdaMismatch
    );
    require_keys_eq!(
        remaining[IDX_ATA_PROGRAM].key(),
        ASSOCIATED_TOKEN_PROGRAM_ID,
        IntentsError::PumpPdaMismatch
    );

    let (idx_fee_config, idx_fee_program) = if is_sell {
        (IDX_FEE_CONFIG_SELL, IDX_FEE_PROGRAM_SELL)
    } else {
        (IDX_FEE_CONFIG_BUY, IDX_FEE_PROGRAM_BUY)
    };
    require_keys_eq!(
        remaining[idx_fee_program].key(),
        PUMP_AMM_FEE_PROGRAM_ID,
        IntentsError::PumpPdaMismatch
    );

    let (fee_config, _) = Pubkey::find_program_address(
        &[b"fee_config", PUMP_AMM_PROGRAM_ID.as_ref()],
        &PUMP_AMM_FEE_PROGRAM_ID,
    );
    require_keys_eq!(
        remaining[idx_fee_config].key(),
        fee_config,
        IntentsError::PumpPdaMismatch
    );

    policy::require_pump_amm_protocol_fee_recipient(&remaining[IDX_PROTOCOL_FEE_RECIPIENT].key())?;
    let (pfr_ata, _) = Pubkey::find_program_address(
        &[
            remaining[IDX_PROTOCOL_FEE_RECIPIENT].key().as_ref(),
            quote_token_program.as_ref(),
            quote_mint.as_ref(),
        ],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    require_keys_eq!(
        remaining[IDX_PROTOCOL_FEE_RECIPIENT_ATA].key(),
        pfr_ata,
        IntentsError::PumpPdaMismatch
    );

    let (pool_v2, _) =
        Pubkey::find_program_address(&[b"pool-v2", base_mint.as_ref()], &PUMP_AMM_PROGRAM_ID);

    let (uva, _) = Pubkey::find_program_address(
        &[b"user_volume_accumulator", trader.as_ref()],
        &PUMP_AMM_PROGRAM_ID,
    );

    let (break_idx, ata_idx) = if is_sell {
        // Official sell (BREAKING_FEE_RECIPIENT.md): 24 remaining.
        // [21]=pool_v2 [22]=breaking fee [23]=breaking fee quote ATA.
        // No cashback / UVA. Buy 26/27 would shift fee_config.
        require!(
            remaining.len() == PUMP_AMM_SELL_ACCOUNT_COUNT,
            IntentsError::PumpAmmSellAccountCountMismatch
        );
        require_keys_eq!(remaining[21].key(), pool_v2, IntentsError::PumpPdaMismatch);
        (22, 23)
    } else {
        let (gva, _) =
            Pubkey::find_program_address(&[b"global_volume_accumulator"], &PUMP_AMM_PROGRAM_ID);
        require_keys_eq!(
            remaining[IDX_GLOBAL_VOLUME].key(),
            gva,
            IntentsError::PumpPdaMismatch
        );
        require_keys_eq!(
            remaining[IDX_USER_VOLUME].key(),
            uva,
            IntentsError::PumpPdaMismatch
        );
        // Trailing remaining accounts (official BREAKING_FEE_RECIPIENT.md):
        // non-cashback (26): [23]=pool_v2 [24]=breaking fee [25]=breaking fee quote ATA
        // cashback (27):     [23]=UVA quote ATA [24]=pool_v2 [25]=breaking fee [26]=ATA
        if remaining.len() == PUMP_AMM_BUY_ACCOUNT_COUNT_CASHBACK {
            let (uva_wsol, _) = Pubkey::find_program_address(
                &[uva.as_ref(), quote_token_program.as_ref(), quote_mint.as_ref()],
                &ASSOCIATED_TOKEN_PROGRAM_ID,
            );
            require_keys_eq!(remaining[23].key(), uva_wsol, IntentsError::PumpPdaMismatch);
            require_keys_eq!(remaining[24].key(), pool_v2, IntentsError::PumpPdaMismatch);
            (25, 26)
        } else {
            require!(
                remaining.len() == PUMP_AMM_BUY_ACCOUNT_COUNT,
                IntentsError::PumpAmmAccountCountMismatch
            );
            require_keys_eq!(remaining[23].key(), pool_v2, IntentsError::PumpPdaMismatch);
            (24, 25)
        }
    };

    let break_fee = remaining[break_idx].key();
    policy::require_pump_amm_breaking_fee_recipient(&break_fee)?;
    let (break_ata, _) = Pubkey::find_program_address(
        &[
            break_fee.as_ref(),
            quote_token_program.as_ref(),
            quote_mint.as_ref(),
        ],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    require_keys_eq!(
        remaining[ata_idx].key(),
        break_ata,
        IntentsError::PumpPdaMismatch
    );
    let _ = PUMP_AMM_PROGRAM_INDEX;
    Ok(())
}

#[derive(Accounts)]
pub struct PumpAmmTrade<'info> {
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
    /// Program-owned custody. Not debited in pump_amm_buy / pump_amm_sell.
    /// Never PumpSwap `user`.
    #[account(
        mut,
        seeds = [SEED_SPEND_VAULT, grok_account.key().as_ref()],
        bump = spend_vault.bump,
        constraint = spend_vault.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = spend_vault.root == grok_account.root @ IntentsError::UnauthorizedRoot,
    )]
    pub spend_vault: Account<'info, SpendVault>,
    /// Trader is remaining[1] only. Named+remaining duplicate of the same
    /// writable system account makes try_debit UnbalancedInstruction.
    pub system_program: Program<'info, System>,
    /// CHECK: hardcoded allowlist. Only AMM program this adapter CPIs into.
    #[account(address = crate::PUMP_AMM_PROGRAM_ID, executable)]
    pub pump_amm_program: UncheckedAccount<'info>,
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
