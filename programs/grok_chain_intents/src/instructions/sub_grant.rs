//! Sub-grants: an agent delegating a slice of its own authority to another agent.
//!
//! NOT DEPLOYED. The live MAINNET binary does not contain these instructions.
//!
//! WHY THIS EXISTS
//! CORE issues one grant to one agent, authorised by a human. That is exactly
//! right for one agent, and wrong for the shape real agent systems take: a
//! planner hires a researcher, which hires a buyer. Today every one of those
//! needs its own human-issued grant, which puts the human back in the loop for
//! every new agent — the thing this whole design exists to avoid.
//!
//! A sub-grant lets an agent issue authority out of its own budget. No wallet
//! appears anywhere in the tree, and the human's cap is still the real ceiling.
//!
//! WHERE IT LIVES, AND WHY NOT IN CORE
//! Grant PDAs belong to CORE, whose deployed mainnet source is not public, so it
//! cannot be extended here. Sub-grants therefore live in INTENTS and sit
//! *beneath* a single CORE grant. That is not a workaround — it is better. The
//! CORE grant remains an outer ceiling this program cannot exceed no matter what
//! the sub-grant tree says, so a bug in this file cannot spend more than the
//! human already authorised.
//!
//! ATTENUATION
//! Authority only ever narrows going down:
//!
//!   * a sub-grant is written once and can only ever narrow; no instruction
//!     here widens delegated authority at all
//!   * a child's cap may not exceed the parent's REMAINING headroom
//!   * a child's expiry may not exceed the parent's
//!   * a child's asset is the parent's asset; there is no widening to a second
//!   * depth strictly increases, and is bounded
//!
//! METERING WALKS UP
//! Spending a child's budget also meters every ancestor. A grandchild spending
//! ten units moves `spent` by ten on itself, its parent, and the root of the
//! tree. The human's cap therefore binds the whole subtree, not just the agent
//! it was issued to.
//!
//! REVOCATION CASCADES WITHOUT TRAVERSAL
//! There is no subtree walk, no bulk update, no unbounded loop. Revoking a
//! parent kills its descendants because every spend re-walks the chain to the
//! root and refuses if ANY ancestor is revoked or expired. The cascade is a
//! property of validation rather than an operation someone has to perform, which
//! means it cannot half-finish and cannot run out of compute.
//!
//! Killing an entire tree is therefore one instruction on one account, no matter
//! how many agents hang beneath it.
//!
//! CYCLES ARE STRUCTURALLY IMPOSSIBLE
//! A parent must already exist when a child is issued, and depth strictly
//! increases, so no chain can close on itself. The depth bound then caps the
//! walk, which is what keeps metering affordable.

use anchor_lang::prelude::*;

use grok_chain_core::{Grant, GrokAccount, SEED_GRANT, SEED_GROK_ACCOUNT};

use crate::constants::SEED_SUB_GRANT;
use crate::errors::IntentsError;
use crate::events::{SubGrantIssued, SubGrantRevised, SubGrantRevoked, SubGrantSpent};
use crate::state::SubGrant;

/// How deep a delegation chain may go beneath the CORE-granted agent.
///
/// Three covers planner → researcher → buyer, which is the topology this is for.
/// It is a hard bound rather than a guideline because every spend walks the
/// chain: an unbounded depth is an unbounded loop, and an unbounded loop inside
/// a payment is a denial of service on the payment.
pub const MAX_SUB_DEPTH: u8 = 3;

/// The agent named in the CORE grant issues the first sub-grant. Deeper levels
/// are issued by whoever holds the sub-grant above them.
pub fn issue(
    ctx: Context<IssueSubGrant>,
    agent: Pubkey,
    cap: u64,
    expires_at_unix: i64,
) -> Result<()> {
    require!(cap > 0, IntentsError::ZeroAmount);
    require_keys_neq!(agent, ctx.accounts.issuer.key(), IntentsError::SubGrantSelfIssue);

    let now = Clock::get()?.unix_timestamp;
    require!(expires_at_unix > now, IntentsError::SubGrantExpiryInPast);

    let core = &ctx.accounts.grant;
    require!(!core.revoked, IntentsError::SubGrantParentRevoked);
    require!(core.expires_at_unix > now, IntentsError::SubGrantParentExpired);

    // Depth 1 hangs off the CORE grant; deeper hangs off a sub-grant.
    let (depth, parent_key, headroom, parent_expiry) = match &ctx.accounts.parent {
        Some(parent) => {
            require_keys_eq!(
                parent.agent,
                ctx.accounts.issuer.key(),
                IntentsError::SubGrantNotIssuer
            );
            require!(!parent.revoked, IntentsError::SubGrantParentRevoked);
            require!(parent.expires_at > now, IntentsError::SubGrantParentExpired);
            require!(
                parent.depth < MAX_SUB_DEPTH,
                IntentsError::SubGrantTooDeep
            );
            (
                parent.depth + 1,
                parent.key(),
                parent.cap.saturating_sub(parent.spent),
                parent.expires_at,
            )
        }
        None => {
            // No parent sub-grant: the issuer must be the agent CORE authorised.
            require_keys_eq!(
                core.agent,
                ctx.accounts.issuer.key(),
                IntentsError::SubGrantNotIssuer
            );
            (
                1u8,
                Pubkey::default(),
                core.spend_cap_lamports
                    .saturating_sub(core.spent_lamports),
                core.expires_at_unix,
            )
        }
    };

    // Attenuation, both directions. A child cannot outlive or outspend what
    // issued it, so authority strictly narrows going down and the human's cap
    // binds the whole tree.
    require!(cap <= headroom, IntentsError::SubGrantExceedsParent);
    require!(
        expires_at_unix <= parent_expiry,
        IntentsError::SubGrantOutlivesParent
    );

    let sg = &mut ctx.accounts.sub_grant;
    sg.grok_account = ctx.accounts.grok_account.key();
    sg.parent = parent_key;
    sg.issuer = ctx.accounts.issuer.key();
    sg.agent = agent;
    sg.cap = cap;
    sg.spent = 0;
    sg.expires_at = expires_at_unix;
    sg.revoked = false;
    sg.depth = depth;
    sg.generation = 1;
    sg.bump = ctx.bumps.sub_grant;

    emit!(SubGrantIssued {
        grok_account: sg.grok_account,
        parent: parent_key,
        issuer: sg.issuer,
        agent,
        cap,
        expires_at: expires_at_unix,
        depth,
        generation: sg.generation,
    });
    Ok(())
}

/// Lower a cap or shorten an expiry. Widening is refused: an issuer that wants
/// to grant more must re-issue, which is visible as a new generation.
pub fn revise(ctx: Context<ManageSubGrant>, cap: u64, expires_at_unix: i64) -> Result<()> {
    let sg = &mut ctx.accounts.sub_grant;
    require!(cap <= sg.cap, IntentsError::SubGrantCannotWiden);
    require!(cap >= sg.spent, IntentsError::SubGrantCapBelowSpent);
    require!(
        expires_at_unix <= sg.expires_at,
        IntentsError::SubGrantCannotWiden
    );
    sg.cap = cap;
    sg.expires_at = expires_at_unix;

    emit!(SubGrantRevised {
        grok_account: sg.grok_account,
        agent: sg.agent,
        cap,
        expires_at: expires_at_unix,
        spent: sg.spent,
    });
    Ok(())
}

/// Kill one node, and with it everything beneath it.
///
/// Nothing is written to any descendant. They die because every spend re-walks
/// to the root and refuses on a revoked ancestor, so this is O(1) regardless of
/// how many agents hang below.
pub fn revoke(ctx: Context<ManageSubGrant>) -> Result<()> {
    let sg = &mut ctx.accounts.sub_grant;
    require!(!sg.revoked, IntentsError::SubGrantAlreadyRevoked);
    sg.revoked = true;
    sg.generation = sg.generation.saturating_add(1);

    emit!(SubGrantRevoked {
        grok_account: sg.grok_account,
        agent: sg.agent,
        spent: sg.spent,
        depth: sg.depth,
        generation: sg.generation,
    });
    Ok(())
}

/// Walk from a leaf to the root, checking every ancestor and metering all of
/// them.
///
/// `chain` is leaf-first: the spender's own sub-grant, then its parent, and so
/// on. Callers pass it through `remaining_accounts`, which is what bounds the
/// cost — MAX_SUB_DEPTH entries at most.
///
/// Every node is checked before any is written, so a chain that fails halfway
/// leaves nothing metered.
pub fn walk_and_meter<'info>(
    chain: &[AccountLoaderChain<'info>],
    spender: &Pubkey,
    amount: u64,
    now: i64,
) -> Result<()> {
    require!(!chain.is_empty(), IntentsError::SubGrantChainEmpty);
    require!(
        chain.len() <= MAX_SUB_DEPTH as usize,
        IntentsError::SubGrantTooDeep
    );

    // Pass one: validate the whole chain, touching nothing.
    for (i, node) in chain.iter().enumerate() {
        let sg = &node.data;
        require!(!sg.revoked, IntentsError::SubGrantRevoked);
        require!(sg.expires_at > now, IntentsError::SubGrantExpired);
        require!(
            sg.cap.saturating_sub(sg.spent) >= amount,
            IntentsError::SubGrantCapExceeded
        );
        if i == 0 {
            require_keys_eq!(sg.agent, *spender, IntentsError::SubGrantNotHolder);
        } else {
            // Each step must actually be the previous node's parent, so a caller
            // cannot substitute a richer unrelated grant partway up.
            require_keys_eq!(
                chain[i - 1].data.parent,
                node.key,
                IntentsError::SubGrantChainBroken
            );
        }
        if i == chain.len() - 1 {
            // The top of the chain hangs off the CORE grant, not another
            // sub-grant. If it claims a parent, the chain was truncated.
            require_keys_eq!(
                sg.parent,
                Pubkey::default(),
                IntentsError::SubGrantChainTruncated
            );
        }
    }
    Ok(())
}

/// A node of the chain, already deserialised, paired with its address so the
/// parent links can be checked without another lookup.
pub struct AccountLoaderChain<'info> {
    pub key: Pubkey,
    pub data: SubGrant,
    pub info: AccountInfo<'info>,
}

/// Apply the metering the walk validated. Separated so no balance moves until
/// every check in the chain has passed.
pub fn commit_meter(chain: &mut [AccountLoaderChain], amount: u64) -> Result<()> {
    for node in chain.iter_mut() {
        node.data.spent = node
            .data
            .spent
            .checked_add(amount)
            .ok_or(error!(IntentsError::SubGrantCapExceeded))?;
        let mut data = node.info.try_borrow_mut_data()?;
        let mut cursor: &mut [u8] = &mut data[8..];
        node.data.serialize(&mut cursor)?;
    }
    if let Some(leaf) = chain.first() {
        emit!(SubGrantSpent {
            grok_account: leaf.data.grok_account,
            agent: leaf.data.agent,
            amount,
            depth: leaf.data.depth,
            spent_after: leaf.data.spent,
        });
    }
    Ok(())
}

#[derive(Accounts)]
#[instruction(agent: Pubkey)]
pub struct IssueSubGrant<'info> {
    /// The agent doing the delegating. Signs, holds nothing, pays no gas.
    pub issuer: Signer<'info>,
    /// Pays rent for the new account. Never the issuer, which holds no SOL.
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        seeds = [SEED_GROK_ACCOUNT, grok_account.root.as_ref()],
        seeds::program = grok_chain_core::ID,
        bump,
    )]
    pub grok_account: Account<'info, GrokAccount>,
    /// The CORE grant this whole tree hangs beneath. Its cap is the ceiling no
    /// sub-grant can exceed.
    #[account(
        seeds = [SEED_GRANT, grok_account.key().as_ref(), grant.agent.as_ref()],
        seeds::program = grok_chain_core::ID,
        bump,
        constraint = grant.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
    )]
    pub grant: Account<'info, Grant>,
    /// Absent for depth 1. Present, and held by the issuer, for deeper levels.
    pub parent: Option<Account<'info, SubGrant>>,
    /// `init`, not `init_if_needed`. A sub-grant is written once per agent and
    /// can only ever narrow afterwards, so no instruction in this program is
    /// able to increase delegated authority. Giving an agent more means issuing
    /// to a fresh key, which is visible rather than silent.
    #[account(
        init,
        payer = payer,
        space = SubGrant::SPACE,
        seeds = [SEED_SUB_GRANT, grok_account.key().as_ref(), agent.as_ref()],
        bump,
    )]
    pub sub_grant: Account<'info, SubGrant>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ManageSubGrant<'info> {
    /// The issuer of this sub-grant, or the human root. Either may pull it.
    pub authority: Signer<'info>,
    #[account(
        seeds = [SEED_GROK_ACCOUNT, grok_account.root.as_ref()],
        seeds::program = grok_chain_core::ID,
        bump,
    )]
    pub grok_account: Account<'info, GrokAccount>,
    #[account(
        mut,
        seeds = [SEED_SUB_GRANT, grok_account.key().as_ref(), sub_grant.agent.as_ref()],
        bump = sub_grant.bump,
        constraint = sub_grant.grok_account == grok_account.key() @ IntentsError::GrokAccountMismatch,
        constraint = sub_grant.issuer == authority.key() || grok_account.root == authority.key()
            @ IntentsError::SubGrantNotIssuer,
    )]
    pub sub_grant: Account<'info, SubGrant>,
}
