// pub use * is required: Anchor 0.30 #[program] resolves __client_accounts_* via crate-root globs.
pub mod call;
pub mod common;
pub mod deploy;
pub mod pay;
pub mod paymaster;
pub mod spend_vault;
pub mod swap;

pub use call::*;
pub use deploy::*;
pub use pay::*;
pub use paymaster::*;
pub use spend_vault::*;
pub use swap::*;

use anchor_lang::prelude::*;

use crate::errors::IntentsError;

/// Debit a program-owned PDA and credit `to`. Leaves `min_from` lamports on `from`.
/// Returns false if `from` cannot pay `amount` and still hold `min_from`.
/// System transfer cannot debit a non-system owner; this is the PDA-signed equivalent.
pub fn try_debit_program_owned<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    amount: u64,
    min_from: u64,
) -> Result<bool> {
    let Some(after) = from.lamports().checked_sub(amount) else {
        return Ok(false);
    };
    if after < min_from {
        return Ok(false);
    }
    let dest = to
        .lamports()
        .checked_add(amount)
        .ok_or(error!(IntentsError::LamportOverflow))?;
    **from.try_borrow_mut_lamports()? = after;
    **to.try_borrow_mut_lamports()? = dest;
    Ok(true)
}
