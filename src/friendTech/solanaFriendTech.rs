use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    program::{invoke},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
};
use spl_token::{self, state::Account as SplTokenAccount, instruction as spl_token_instruction};
use borsh::{BorshDeserialize, BorshSerialize};

// Constants for the dual-phase pricing algorithm.
const DEFAULT_CURRENT_VOLUME: f64 = 10.0;
const DEFAULT_AVERAGE_VOLUME: f64 = 7.0;
const DEFAULT_TIME_SINCE_LAST_TRADE: f64 = 1.0;

/// Represents a shareholder account with ownership and balance details.
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct ShareAccount {
    pub owner: Pubkey,
    pub balance: u64,
}

/// Custom errors to represent specific failure reasons in the FriendTech program.
pub enum FriendtechError {
    IncorrectOwner,
    InsufficientFunds,
}
impl From<FriendtechError> for ProgramError {
    fn from(e: FriendtechError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

/// Instructions supported by the FriendTech program, including buying and selling of shares.
pub enum FriendtechInstruction {
    BuyShares { amount: u64 },
    SellShares { amount: u64 },
}

/// Calculate base price derived from the number of current holders.
fn base_price_from_holders(current_holders: u32) -> f64 {
    if current_holders <= 10 {
        0.1 * current_holders as f64
    } else {
        (current_holders as f64 - 10.0) + 1.0
    }
}

/// Dual-phase pricing algorithm considering trading volume, 
/// number of current holders, and the time elapsed since the last trade.
fn dual_phase_pricing(current_holders: u32, current_volume: f64, average_volume: f64, time_since_last_trade: f64) -> f64 {
    const VOLUME_ADJUSTMENT_FACTOR: f64 = 0.01;
    const INACTIVITY_ADJUSTMENT_FACTOR: f64 = 0.005;
    const INACTIVITY_THRESHOLD: f64 = 24.0;

    let base_price = base_price_from_holders(current_holders);
    let volume_ratio = current_volume / average_volume;

    if time_since_last_trade > INACTIVITY_THRESHOLD {
        base_price * (1.0 - INACTIVITY_ADJUSTMENT_FACTOR)
    } else {
        base_price * (1.0 + VOLUME_ADJUSTMENT_FACTOR * volume_ratio)
    }
}

/// Main entry point for processing instructions related to the FriendTech program.
entrypoint!(process_instruction);
fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let accounts_iter = &mut accounts.iter();
    let account = next_account_info(accounts_iter)?;
    let token_account = next_account_info(accounts_iter)?;

    if account.owner != program_id {
        return Err(FriendtechError::IncorrectOwner.into());
    }

    let instruction = FriendtechInstruction::try_from_slice(instruction_data)?;

    match instruction {
        FriendtechInstruction::BuyShares { amount } => {
            let mut share_account = ShareAccount::unpack(&account.data.borrow())?;

            let price_per_share = dual_phase_pricing(
                share_account.balance as u32,
                DEFAULT_CURRENT_VOLUME,
                DEFAULT_AVERAGE_VOLUME,
                DEFAULT_TIME_SINCE_LAST_TRADE,
            );
            let total_price = (price_per_share * amount as f64) as u64;

            let user_spl_token_account = SplTokenAccount::unpack(&token_account.data.borrow())?;
            if user_spl_token_account.amount < total_price {
                return Err(FriendtechError::InsufficientFunds.into());
            }

            let ix = spl_token_instruction::transfer(
                &spl_token::id(),
                &token_account.key,
                &token_account.key,
                &account.owner,
                &[],
                total_price,
            );
            invoke(&ix, &[token_account.clone(), account.clone()])?;

            share_account.balance += amount;
            ShareAccount::pack(share_account, &mut account.data.borrow_mut())?;
        }
        FriendtechInstruction::SellShares { amount } => {
            let mut share_account = ShareAccount::unpack(&account.data.borrow())?;

            if share_account.balance < amount {
                return Err(FriendtechError::InsufficientFunds.into());
            }

            let total_price = (base_price_from_holders(share_account.balance as u32) * amount as f64) as u64;
            let ix = spl_token_instruction::transfer(
                &spl_token::id(),
                &token_account.key,
                &token_account.key,
                &account.owner,
                &[],
                total_price,
            );
            invoke(&ix, &[token_account.clone(), account.clone()])?;

            share_account.balance -= amount;
            ShareAccount::pack(share_account, &mut account.data.borrow_mut())?;
        }
    }

    Ok(())
}

/// Tests to validate the dual-phase pricing algorithm's logic and outcomes.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dual_phase_pricing() {
        // Test the pricing algorithm with a set of predefined scenarios.

        let base_price = base_price_from_holders(5); // Expected to be 0.5
        let volume_ratio = 10.0 / 7.0;
        let expected = base_price * (1.0 + 0.01 * volume_ratio);
        assert_eq!(dual_phase_pricing(5, 10.0, 7.0, 1.0), expected);

        assert_eq!(dual_phase_pricing(5, 10.0, 7.0, 25.0), base_price * (1.0 - 0.005));

        let base_price_high = base_price_from_holders(15); // Expected to be 6.0
        let expected_high = base_price_high * (1.0 + 0.01 * volume_ratio);
        assert_eq!(dual_phase_pricing(15, 10.0, 7.0, 1.0), expected_high);

        let base_price_exact = base_price_from_holders(10); // Expected to be 1.0
        let expected_exact = base_price_exact * (1.0 + 0.01 * volume_ratio);
        assert_eq!(dual_phase_pricing(10, 10.0, 7.0, 1.0), expected_exact);
    }
}
