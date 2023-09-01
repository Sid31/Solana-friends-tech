use solana_program::{
    account_info::{next_account_info, AccountInfo},
    decode_error::DecodeError,
    entrypoint,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::{clock::Clock, Sysvar},
};
use spl_token::{self, state::Account as SplTokenAccount, instruction as spl_token_instruction};
use borsh::{BorshDeserialize, BorshSerialize};

// Constants for the dual phase pricing algorithm
const DEFAULT_CURRENT_VOLUME: f64 = 10.0;
const DEFAULT_AVERAGE_VOLUME: f64 = 7.0;
const DEFAULT_TIME_SINCE_LAST_TRADE: f64 = 1.0;

#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct ShareAccount {
    pub owner: Pubkey,
    pub balance: u64,
}

pub enum FriendtechError {
    IncorrectOwner,
    InsufficientFunds,
}
impl From<FriendtechError> for ProgramError {
    fn from(e: FriendtechError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

pub enum FriendtechInstruction {
    BuyShares { amount: u64 },
    SellShares { amount: u64 },
}

fn base_price_from_holders(current_holders: u32) -> f64 {
    if current_holders <= 10 {
        0.1 * current_holders as f64
    } else {
        (current_holders as f64 - 10.0) + 1.0
    }
}

fn dual_phase_pricing(current_holders: u32, current_volume: f64, average_volume: f64, time_since_last_trade: f64) -> f64 {
    let volume_adjustment_factor = 0.01;
    let inactivity_adjustment_factor = 0.005;
    let inactivity_threshold = 24.0;

    let base_price = base_price_from_holders(current_holders);

    let volume_ratio = current_volume / average_volume;

    if time_since_last_trade > inactivity_threshold {
        base_price * (1.0 - inactivity_adjustment_factor)
    } else {
        base_price * (1.0 + volume_adjustment_factor * volume_ratio)
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dual_phase_pricing() {
        // For the scenario when current_holders = 5, 
        // current_volume = 10.0, average_volume = 7.0, time_since_last_trade = 1.0
        let base_price = base_price_from_holders(5); // 0.5
        let volume_ratio = 10.0 / 7.0;
        let volume_adjustment_factor = 0.01;
        let expected = base_price * (1.0 + volume_adjustment_factor * volume_ratio);
        assert_eq!(dual_phase_pricing(5, 10.0, 7.0, 1.0), expected);

        // For the scenario with inactivity over the threshold
        assert_eq!(dual_phase_pricing(5, 10.0, 7.0, 25.0), base_price * (1.0 - 0.005));

        // For a scenario with more than 10 current holders
        let base_price_high = base_price_from_holders(15); // 6.0
        let expected_high = base_price_high * (1.0 + volume_adjustment_factor * volume_ratio);
        assert_eq!(dual_phase_pricing(15, 10.0, 7.0, 1.0), expected_high);

        // For a scenario with exactly 10 current holders
        let base_price_exact = base_price_from_holders(10); // 1.0
        let expected_exact = base_price_exact * (1.0 + volume_adjustment_factor * volume_ratio);
        assert_eq!(dual_phase_pricing(10, 10.0, 7.0, 1.0), expected_exact);
    }
}
