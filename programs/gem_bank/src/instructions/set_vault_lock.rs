use anchor_lang::prelude::*;
use gem_common::errors::ErrorCode;

use crate::state::*;

#[derive(Accounts)]
pub struct SetVaultLock<'info> {
    // bank
    #[account(has_one = bank_manager)]
    pub bank: Account<'info, Bank>,
    pub bank_manager: Signer<'info>,

    // vault
    // todo can do seeds verification
    #[account(mut, has_one = bank)]
    pub vault: Account<'info, Vault>,
}

pub fn handler(ctx: Context<SetVaultLock>, vault_locked: bool) -> ProgramResult {
    let bank = &ctx.accounts.bank;
    let vault = &mut ctx.accounts.vault;

    if Bank::read_flags(bank.flags)?.contains(BankFlags::FREEZE_VAULTS) {
        return Err(ErrorCode::VaultAccessSuspended.into());
    }

    vault.locked = vault_locked;

    msg!("vault {} lock set to {}", vault.key(), vault_locked);
    Ok(())
}
