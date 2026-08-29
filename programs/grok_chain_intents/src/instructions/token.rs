//! Tight Jupiter v6 adapter (`token_buy` / `token_sell`).
//!
//! Inner program is hardcoded to Jupiter v6:
//! `JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4`.
//! Client remaining_accounts come from Jupiter swap-instructions.
//! Do not take a raw client program id.
//!
//! Pump-trader PDA is the swapper (seeds `[b"pump-trader", grok_account]`).
//! invoke_signed uses trader seeds only — for Jupiter and wrap. Never vault
//! seeds. Never empty inner data.
//!
//! Quote mint may be WSOL, official USDC, or another SPL / Token-2022 mint.
//! Do NOT require WSOL (that is pump_amm's rule).
//!
//! Grant:
//! - paying with native SOL or WSOL → check_grant(in_amount)
//! - paying with USDC / another token already on the trader → check_grant(0)
//! Native SOL is taken from the prefunded trader (root `fund_pump_trader`
//! from SpendVault). In-ix vault debit + wrap/Jupiter is UnbalancedInstruction.
//! Debit SpendVault only conceptually for native SOL (prefund path).
//! Do not debit SpendVault when paying with a token already on the trader.
//!
//! Wrap native SOL onto trader WSOL ATA when `wrap_sol`. Do not unwrap or
//! sweep in this ix. Leftover stays on the trader. Root has withdraw_pump_trader.
//! Adapter does not create ATAs. Client CreateIdempotent first.
//!
//! Old `swap` is unchanged: grant-gated SOL send + min_out. Not Jupiter. Not an AMM.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    system_instruction, system_program,
};

use grok_chain_core::{Grant, GrokAccount, SEED_GRANT, SEED_GROK_ACCOUNT};

use crate::constants::{
    JUPITER_V6_PROGRAM_ID, PUMP_TRADER_SPACE, SEED_PAYMASTER, SEED_PUMP_TRADER, SEED_SPEND_VAULT,
    TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID,
};
use crate::core_cpi;
use crate::errors::IntentsError;
use crate::events::{TokenBought, TokenSold};
use crate::instructions::common;
use crate::policy;
use crate::state::{Paymaster, SpendVault, TokenBuyArgs, TokenSellArgs};

/// SPL Token `SyncNative` (instruction 17).
const TOKEN_SYNC_NATIVE: u8 = 17;

pub fn token_buy_handler(ctx: Context<TokenTrade>, args: TokenBuyArgs) -> Result<()> {
    trade(ctx, &args, false)
}

pub fn token_sell_handler(ctx: Context<TokenTrade>, args: TokenSellArgs) -> Result<()> {
    // TokenSellArgs is the same wire as TokenBuyArgs.
    let buy = TokenBuyArgs {
        in_amount: args.in_amount,
        min_out: args.min_out,
        sponsor_lamports: args.sponsor_lamports,
        input_mint: args.input_mint,
        output_mint: args.output_mint,
        wrap_sol: args.wrap_sol,
        jupiter_data: args.jupiter_data,
    };
    trade(ctx, &buy, true)
}

fn trade(ctx: Context<TokenTrade>, args: &TokenBuyArgs, is_sell: bool) -> Result<()> {
    policy::require_token_amounts(args.in_amount)?;
    policy::require_token_mints_distinct(&args.input_mint, &args.output_mint)?;
    policy::require_wrap_mint(args.wrap_sol, &args.input_mint)?;
    common::precheck_sponsor(
        args.sponsor_lamports,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
    )?;
    require_agent_not_fee_payer_ctx(&ctx)?;
    policy::require_jupiter_program(&ctx.accounts.jupiter_program.key())?;
    policy::require_nonempty_jupiter_data(&args.jupiter_data)?;
    policy::require_jupiter_in_amount(&args.jupiter_data, args.in_amount)?;

    let vault = ctx.accounts.spend_vault.key();
    let grok = ctx.accounts.grok_account.key();
    let (trader, bump_u8) =
        Pubkey::find_program_address(&[SEED_PUMP_TRADER, grok.as_ref()], &crate::ID);

    let trader_ai = find_trader_in_remaining(ctx.remaining_accounts, &trader)?;
    common::require_pump_trader_ready(trader_ai)?;
    policy::require_pump_user_not_vault(&trader, &vault)?;
    policy::require_pump_trader_system_owned(trader_ai.owner)?;

    let pm_key = ctx.accounts.paymaster.as_ref().map(|p| p.key());
    common::reject_protected_remaining(ctx.remaining_accounts, &vault, pm_key.as_ref())?;

    let source = find_trader_token_account(
        ctx.remaining_accounts,
        &trader,
        &args.input_mint,
        true,
    )?;
    let dest = find_trader_token_account(
        ctx.remaining_accounts,
        &trader,
        &args.output_mint,
        false,
    )?;
    require!(
        !source.data_is_empty(),
        IntentsError::PumpAtaCreateRequiresFeePayer
    );
    require!(
        !dest.data_is_empty(),
        IntentsError::PumpAtaCreateRequiresFeePayer
    );

    // Grant: SOL/WSOL spent → in_amount. Token already on trader → 0.
    // Native SOL is not raw-debited from SpendVault in this ix (named vault
    // + remaining trader + later wrap/Jupiter CPI is UnbalancedInstruction).
    // Root fund_pump_trader first. Debit SpendVault only for native SOL,
    // and only via that prefund path — never for USDC / other tokens.
    let grant_amount = policy::token_check_grant_amount(&args.input_mint, args.wrap_sol, args.in_amount);
    core_cpi::check_grant(
        ctx.accounts.grok_chain_core_program.to_account_info(),
        ctx.accounts.grok_account.to_account_info(),
        ctx.accounts.grant.to_account_info(),
        ctx.accounts.agent.to_account_info(),
        ctx.accounts.intents_program.to_account_info(),
        grant_amount,
    )?;

    let bump = [bump_u8];
    let trader_signer_seeds: &[&[u8]] = &[SEED_PUMP_TRADER, grok.as_ref(), bump.as_ref()];

    if args.wrap_sol {
        let rent0 = Rent::get()?.minimum_balance(PUMP_TRADER_SPACE);
        policy::require_pump_trader_prefunded(trader_ai.lamports(), args.in_amount, rent0)?;
        let system_ai = ctx
            .remaining_accounts
            .iter()
            .find(|a| a.key() == system_program::ID)
            .ok_or(error!(IntentsError::PumpPdaMismatch))?;
        wrap_sol_to_wsol(
            trader_ai,
            source,
            system_ai,
            args.in_amount,
            trader_signer_seeds,
        )?;
    }

    // Snapshot AFTER any wrap, so a wrapped input counts as available rather
    // than as spend. These two numbers are the only thing that actually bounds
    // a route we did not build.
    let source_before = token_account_amount(source)?;
    let dest_before = token_account_amount(dest)?;

    let data = args.jupiter_data.clone();
    policy::require_nonempty_jupiter_data(&data)?;
    let ix = build_jupiter_ix(ctx.remaining_accounts, &trader, data)?;
    invoke_signed(&ix, ctx.remaining_accounts, &[trader_signer_seeds])?;

    // What the route DID, measured on our own accounts. saturating_sub because a
    // source that somehow grew is not a spend, and a destination that shrank is
    // not a receipt — both are caught by the bounds below.
    let spent = source_before.saturating_sub(token_account_amount(source)?);
    let received = token_account_amount(dest)?.saturating_sub(dest_before);
    policy::enforce_swap_outcome(spent, received, args.in_amount, args.min_out)?;

    // Do not unwrap. Do not sweep trader → vault in this ix.

    common::reimburse_sponsor(
        &ctx.accounts.grant,
        &ctx.accounts.paymaster,
        &ctx.accounts.fee_payer,
        args.sponsor_lamports,
    )?;

    if is_sell {
        emit!(TokenSold {
            vault,
            trader,
            input_mint: args.input_mint,
            output_mint: args.output_mint,
            in_amount: args.in_amount,
            min_out: args.min_out,
            spent,
            received,
            agent: ctx.accounts.agent.key(),
            grant: ctx.accounts.grant.key(),
            generation: ctx.accounts.grant.generation,
        });
    } else {
        emit!(TokenBought {
            vault,
            trader,
            input_mint: args.input_mint,
            output_mint: args.output_mint,
            in_amount: args.in_amount,
            min_out: args.min_out,
            spent,
            received,
            agent: ctx.accounts.agent.key(),
            grant: ctx.accounts.grant.key(),
            generation: ctx.accounts.grant.generation,
        });
    }
    Ok(())
}

fn require_agent_not_fee_payer_ctx(ctx: &Context<TokenTrade>) -> Result<()> {
    let fp = ctx.accounts.fee_payer.as_ref().map(|f| f.key());
    policy::require_agent_not_fee_payer(&ctx.accounts.agent.key(), fp.as_ref())
}

fn find_trader_in_remaining<'a, 'info>(
    remaining: &'a [AccountInfo<'info>],
    trader: &Pubkey,
) -> Result<&'a AccountInfo<'info>> {
    for acc in remaining {
        if acc.key() == *trader {
            return Ok(acc);
        }
    }
    err!(IntentsError::PumpUserMustBeTrader)
}
/// Raw token amount at offset 64..72 of an SPL / Token-2022 account.
fn token_account_amount(info: &AccountInfo) -> Result<u64> {
    let data = info.try_borrow_data()?;
    require!(data.len() >= 72, IntentsError::JupiterSourceOwnerNotTrader);
    Ok(u64::from_le_bytes(data[64..72].try_into().unwrap()))
}

fn find_trader_token_account<'a, 'info>(
    remaining: &'a [AccountInfo<'info>],
    trader: &Pubkey,
    mint: &Pubkey,
    is_source: bool,
) -> Result<&'a AccountInfo<'info>> {
    for acc in remaining {
        if acc.data_len() < 165 {
            continue;
        }
        if *acc.owner != TOKEN_PROGRAM_ID && *acc.owner != TOKEN_2022_PROGRAM_ID {
            continue;
        }
        let data = acc.try_borrow_data()?;
        let acc_mint = Pubkey::try_from(&data[0..32]).unwrap();
        let acc_owner = Pubkey::try_from(&data[32..64]).unwrap();
        drop(data);
        if acc_mint == *mint && acc_owner == *trader {
            return Ok(acc);
        }
    }
    if is_source {
        err!(IntentsError::JupiterSourceOwnerNotTrader)
    } else {
        err!(IntentsError::JupiterDestOwnerNotTrader)
    }
}

fn wrap_sol_to_wsol<'info>(
    trader: &AccountInfo<'info>,
    wsol_ata: &AccountInfo<'info>,
    system_program_ai: &AccountInfo<'info>,
    amount: u64,
    trader_signer_seeds: &[&[u8]],
) -> Result<()> {
    require_keys_eq!(
        *wsol_ata.owner,
        TOKEN_PROGRAM_ID,
        IntentsError::InvalidTokenProgram
    );
    invoke_signed(
        &system_instruction::transfer(trader.key, wsol_ata.key, amount),
        &[trader.clone(), wsol_ata.clone(), system_program_ai.clone()],
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

fn build_jupiter_ix(remaining: &[AccountInfo], trader: &Pubkey, data: Vec<u8>) -> Result<Instruction> {
    require!(!data.is_empty(), IntentsError::JupiterEmptyDataForbidden);
    policy::require_nonempty_jupiter_data(&data)?;
    let mut accounts = Vec::with_capacity(remaining.len());
    let mut saw_trader = false;
    for acc in remaining.iter() {
        if acc.key() == *trader {
            saw_trader = true;
            accounts.push(AccountMeta::new(*trader, true));
        } else if acc.is_writable {
            accounts.push(AccountMeta::new(*acc.key, acc.is_signer));
        } else {
            accounts.push(AccountMeta::new_readonly(*acc.key, acc.is_signer));
        }
    }
    require!(saw_trader, IntentsError::PumpUserMustBeTrader);
    Ok(Instruction {
        program_id: JUPITER_V6_PROGRAM_ID,
        accounts,
        data,
    })
}

#[derive(Accounts)]
pub struct TokenTrade<'info> {
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
    /// Program-owned custody. Not raw-debited in token_buy / token_sell.
    /// Native SOL is prefunded onto the trader. Never Jupiter user.
    #[account(
        mut,
        seeds = [SEED_SPEND_VAULT, grok_account.key().as_ref()],
        bump = spend_vault.bump,
        constraint = spend_vault.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = spend_vault.root == grok_account.root @ IntentsError::UnauthorizedRoot,
    )]
    pub spend_vault: Account<'info, SpendVault>,
    pub system_program: Program<'info, System>,
    /// CHECK: hardcoded Jupiter v6. Only inner program this adapter CPIs into.
    #[account(address = crate::JUPITER_V6_PROGRAM_ID, executable)]
    pub jupiter_program: UncheckedAccount<'info>,
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
