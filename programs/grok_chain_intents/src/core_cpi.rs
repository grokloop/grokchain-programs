//! Thin CPI into CORE `check_grant`. Only CPI we make.
//! Wire matches CORE SPEC.md §6.5 / §13. Do not catch-and-continue.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
};

use crate::constants::CHECK_GRANT_DISCRIMINATOR;
use crate::errors::IntentsError;

/// Account order is NORMATIVE (CORE SPEC.md §13):
/// 0 grok_account   mut=false signer=false
/// 1 grant          mut=true  signer=false
/// 2 agent          mut=false signer=true  (must sign the outer tx)
/// 3 target_program mut=false signer=false (this program id; must be executable)
///
/// `program_id` of the CPI is `grok_chain_core`.
/// Data: 8-byte disc + Borsh u64 `amount_lamports`.
pub fn check_grant<'info>(
    core_program: AccountInfo<'info>,
    grok_account: AccountInfo<'info>,
    grant: AccountInfo<'info>,
    agent: AccountInfo<'info>,
    target_program: AccountInfo<'info>,
    amount_lamports: u64,
) -> Result<()> {
    require_keys_eq!(
        *core_program.key,
        grok_chain_core::ID,
        IntentsError::InvalidCoreProgram
    );

    let mut data = Vec::with_capacity(16);
    data.extend_from_slice(&CHECK_GRANT_DISCRIMINATOR);
    data.extend_from_slice(&amount_lamports.to_le_bytes());

    let ix = Instruction {
        program_id: grok_chain_core::ID,
        accounts: vec![
            AccountMeta::new_readonly(*grok_account.key, false),
            AccountMeta::new(*grant.key, false),
            AccountMeta::new_readonly(*agent.key, true),
            AccountMeta::new_readonly(*target_program.key, false),
        ],
        data,
    };

    invoke(
        &ix,
        &[grok_account, grant, agent, target_program, core_program],
    )?;
    Ok(())
}
