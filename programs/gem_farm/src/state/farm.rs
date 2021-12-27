use anchor_lang::prelude::*;

use gem_common::{errors::ErrorCode, *};

use crate::state::*;

pub const LATEST_FARM_VERSION: u16 = 0;

#[repr(C)]
#[derive(Debug, Copy, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct FarmConfig {
    // min time the NFT has to be staked
    pub min_staking_period_sec: u64,

    // time after user decides to unstake before they can actually withdraw
    pub cooldown_period_sec: u64,

    pub unstaking_fee_lamp: u64,
}

#[repr(C)]
#[account]
#[derive(Debug)]
pub struct Farm {
    pub version: u16,

    pub farm_manager: Pubkey,

    // used for collecting any fees earned by the farm
    pub farm_treasury: Pubkey,

    // signs off on any bank operations related to the farm
    pub farm_authority: Pubkey,

    pub farm_authority_seed: Pubkey,

    pub farm_authority_bump_seed: [u8; 1],

    // each farm controls a single bank
    pub bank: Pubkey,

    pub config: FarmConfig,

    // todo make sure all of the below count vars are incr'ed/decr'ed correctly
    // --------------------------------------- farmers
    // total count, including initialized but inactive farmers
    pub farmer_count: u64, //todo what's the use besides analytics?

    // active only
    pub staked_farmer_count: u64, //todo what's the use besides analytics??

    pub gems_staked: u64,

    // --------------------------------------- funders
    pub authorized_funder_count: u64,

    // --------------------------------------- rewards
    pub reward_a: FarmReward,

    pub reward_b: FarmReward,
}

impl Farm {
    pub fn farm_seeds(&self) -> [&[u8]; 2] {
        [
            self.farm_authority_seed.as_ref(),
            &self.farm_authority_bump_seed,
        ]
    }

    pub fn match_reward_by_mint(
        &mut self,
        reward_mint: Pubkey,
    ) -> Result<&mut FarmReward, ProgramError> {
        let reward_a_mint = self.reward_a.reward_mint;
        let reward_b_mint = self.reward_b.reward_mint;

        match reward_mint {
            _ if reward_mint == reward_a_mint => Ok(&mut self.reward_a),
            _ if reward_mint == reward_b_mint => Ok(&mut self.reward_b),
            _ => Err(ErrorCode::UnknownRewardMint.into()),
        }
    }

    pub fn lock_reward_by_mint(&mut self, reward_mint: Pubkey) -> ProgramResult {
        let reward = self.match_reward_by_mint(reward_mint)?;
        reward.lock_reward()
    }

    pub fn fund_reward_by_mint(
        &mut self,
        now_ts: u64,
        reward_mint: Pubkey,
        variable_rate_config: Option<VariableRateConfig>,
        fixed_rate_config: Option<FixedRateConfig>,
    ) -> ProgramResult {
        let reward = self.match_reward_by_mint(reward_mint)?;
        reward.fund_reward_by_type(now_ts, variable_rate_config, fixed_rate_config)
    }

    pub fn cancel_reward_by_mint(
        &mut self,
        now_ts: u64,
        reward_mint: Pubkey,
    ) -> Result<u64, ProgramError> {
        let reward = self.match_reward_by_mint(reward_mint)?;
        reward.cancel_reward_by_type(now_ts)
    }

    pub fn update_rewards(
        &mut self,
        now_ts: u64,
        mut farmer: Option<&mut Account<Farmer>>,
        reenroll: bool, //relevant for fixed only
    ) -> ProgramResult {
        // reward a
        let (farmer_gems_staked, farmer_reward_a) = match farmer {
            Some(ref mut farmer) => (Some(farmer.gems_staked), Some(&mut farmer.reward_a)),
            None => (None, None),
        };

        self.reward_a.update_accrued_reward_by_type(
            now_ts,
            self.gems_staked,
            farmer_gems_staked,
            farmer_reward_a,
            reenroll,
        )?;

        // reward b
        let farmer_reward_b = match farmer {
            Some(ref mut farmer) => Some(&mut farmer.reward_b),
            None => None,
        };

        self.reward_b.update_accrued_reward_by_type(
            now_ts,
            self.gems_staked,
            farmer_gems_staked,
            farmer_reward_b,
            reenroll,
        )
    }

    pub fn begin_staking(
        &mut self,
        now_ts: u64,
        gems_in_vault: u64,
        farmer: &mut Account<Farmer>,
    ) -> ProgramResult {
        // update farmer
        farmer.begin_staking(self.config.min_staking_period_sec, now_ts, gems_in_vault)?;

        // update farm
        self.staked_farmer_count.try_add_assign(1)?;
        self.gems_staked.try_add_assign(gems_in_vault)?;

        // fixed-rate only - we need to do some extra book-keeping
        if self.reward_a.reward_type == RewardType::Fixed {
            self.reward_a.fixed_rate.enroll_farmer(
                now_ts,
                &mut self.reward_a.times,
                &mut self.reward_a.funds,
                farmer.gems_staked,
                &mut farmer.reward_a,
                None,
            )?;
        }

        if self.reward_b.reward_type == RewardType::Fixed {
            self.reward_b.fixed_rate.enroll_farmer(
                now_ts,
                &mut self.reward_b.times,
                &mut self.reward_b.funds,
                farmer.gems_staked,
                &mut farmer.reward_b,
                None,
            )?;
        }

        Ok(())
    }

    pub fn end_staking(&mut self, now_ts: u64, farmer: &mut Account<Farmer>) -> ProgramResult {
        match farmer.state {
            FarmerState::Unstaked => Ok(msg!("already unstaked!")),
            FarmerState::Staked => {
                // fixed-rate only - we need to do some extra book-keeping
                // (!) MUST COME BEFORE FARMER IS UPDATED - WE NEED CURRENT GEMS AMOUNT
                if self.reward_a.reward_type == RewardType::Fixed {
                    self.reward_a.fixed_rate.graduate_farmer(
                        now_ts,
                        farmer.gems_staked,
                        &mut farmer.reward_a,
                    )?;
                }

                if self.reward_b.reward_type == RewardType::Fixed {
                    self.reward_b.fixed_rate.graduate_farmer(
                        now_ts,
                        farmer.gems_staked,
                        &mut farmer.reward_b,
                    )?;
                }

                // update farmer
                let gems_unstaked =
                    farmer.end_staking_begin_cooldown(now_ts, self.config.cooldown_period_sec)?;

                // update farm
                self.staked_farmer_count.try_sub_assign(1)?;
                self.gems_staked.try_sub_assign(gems_unstaked)?;

                Ok(())
            }
            FarmerState::PendingCooldown => farmer.end_cooldown(now_ts),
        }
    }

    pub fn stake_extra_gems(
        &mut self,
        now_ts: u64,
        gems_in_vault: u64,
        extra_gems: u64,
        farmer: &mut Account<Farmer>,
    ) -> ProgramResult {
        if self.gems_staked.try_add(extra_gems)? != gems_in_vault {
            return Err(ErrorCode::AmountMismatch.into());
        }

        // update farmer
        farmer.begin_staking(self.config.min_staking_period_sec, now_ts, gems_in_vault)?;

        // update farm
        self.gems_staked.try_add_assign(extra_gems)
    }
}

// --------------------------------------- reward tracker

#[repr(C)]
#[derive(Debug, Copy, Clone, AnchorSerialize, AnchorDeserialize, PartialEq)]
pub enum RewardType {
    Variable,
    Fixed,
}

// these numbers should only ever go up
#[repr(C)]
#[derive(Debug, Copy, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct FundsTracker {
    pub total_funded: u64,

    pub total_refunded: u64,

    pub total_accrued_to_stakers: u64,
}

impl FundsTracker {
    pub fn pending_amount(&self) -> Result<u64, ProgramError> {
        self.total_funded
            .try_sub(self.total_refunded)?
            .try_sub(self.total_accrued_to_stakers)
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct TimeTracker {
    pub duration_sec: u64,

    pub reward_end_ts: u64,

    pub lock_end_ts: u64,
}

impl TimeTracker {
    pub fn reward_begin_ts(&self) -> Result<u64, ProgramError> {
        self.reward_end_ts.try_sub(self.duration_sec)
    }

    pub fn remaining_duration(&self, now_ts: u64) -> Result<u64, ProgramError> {
        if now_ts >= self.reward_end_ts {
            return Ok(0);
        }

        self.reward_end_ts.try_sub(now_ts)
    }

    pub fn passed_duration(&self, now_ts: u64) -> Result<u64, ProgramError> {
        self.duration_sec.try_sub(self.remaining_duration(now_ts)?)
    }

    pub fn end_reward(&mut self, now_ts: u64) -> ProgramResult {
        self.duration_sec
            .try_sub_assign(self.remaining_duration(now_ts)?)?;
        self.reward_end_ts = now_ts;

        Ok(())
    }

    /// returns whichever comes first - now or the end of the reward
    pub fn reward_upper_bound(&self, now_ts: u64) -> u64 {
        std::cmp::min(self.reward_end_ts, now_ts)
    }

    /// returns whichever comes last - beginning of the reward, or beginning of farmer's staking
    pub fn reward_lower_bound(&self, farmer_begin_staking_ts: u64) -> Result<u64, ProgramError> {
        Ok(std::cmp::max(
            self.reward_begin_ts()?,
            farmer_begin_staking_ts,
        ))
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct FarmReward {
    // todo in v0 the next 3 fields (mint, pot type) are set ONLY once, at farm init
    //  and can't ever be changed for security reasons
    //  potentially in v1++ might find a way around it, but for now just use a new farm
    pub reward_mint: Pubkey,

    pub reward_pot: Pubkey,

    pub reward_type: RewardType,

    // only one of the two will actually be used
    pub fixed_rate: FixedRateReward,

    pub variable_rate: VariableRateReward,

    pub funds: FundsTracker,

    pub times: TimeTracker,
}

impl FarmReward {
    /// (!) THIS OPERATION IS IRREVERSIBLE
    /// locking ensures the committed reward cannot be withdrawn/changed by a malicious farm operator
    /// once locked, any funding / cancellation ixs become non executable until reward_ned_ts is reached
    fn lock_reward(&mut self) -> ProgramResult {
        self.times.lock_end_ts = self.times.reward_end_ts;

        msg!("locked reward up to {}", self.times.reward_end_ts);
        Ok(())
    }

    fn is_locked(&self, now_ts: u64) -> bool {
        now_ts < self.times.lock_end_ts
    }

    fn fund_reward_by_type(
        &mut self,
        now_ts: u64,
        variable_rate_config: Option<VariableRateConfig>,
        fixed_rate_config: Option<FixedRateConfig>,
    ) -> ProgramResult {
        if self.is_locked(now_ts) {
            return Err(ErrorCode::RewardLocked.into());
        }

        match self.reward_type {
            RewardType::Variable => self.variable_rate.fund_reward(
                now_ts,
                &mut self.times,
                &mut self.funds,
                variable_rate_config.unwrap(),
            ),
            RewardType::Fixed => self.fixed_rate.fund_reward(
                now_ts,
                &mut self.times,
                &mut self.funds,
                fixed_rate_config.unwrap(),
            ),
        }
    }

    fn cancel_reward_by_type(&mut self, now_ts: u64) -> Result<u64, ProgramError> {
        if self.is_locked(now_ts) {
            return Err(ErrorCode::RewardLocked.into());
        }

        match self.reward_type {
            RewardType::Variable => {
                self.variable_rate
                    .cancel_reward(now_ts, &mut self.times, &mut self.funds)
            }
            RewardType::Fixed => {
                self.fixed_rate
                    .cancel_reward(now_ts, &mut self.times, &mut self.funds)
            }
        }
    }

    fn update_accrued_reward_by_type(
        &mut self,
        now_ts: u64,
        farm_gems_staked: u64,
        farmer_gems_staked: Option<u64>,
        farmer_reward: Option<&mut FarmerReward>,
        reenroll: bool,
    ) -> ProgramResult {
        match self.reward_type {
            RewardType::Variable => self.variable_rate.update_accrued_reward(
                now_ts,
                &self.times,
                &mut self.funds,
                farm_gems_staked,
                farmer_gems_staked,
                farmer_reward,
            ),
            RewardType::Fixed => {
                // for fixed rewards we only update if Farmer has been passed
                if farmer_reward.is_none() {
                    return Ok(());
                }

                self.fixed_rate.update_accrued_reward(
                    now_ts,
                    &mut self.times,
                    &mut self.funds,
                    farmer_gems_staked.unwrap(),
                    farmer_reward.unwrap(),
                    reenroll,
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_tracker() {
        let times = TimeTracker {
            duration_sec: 100,
            reward_end_ts: 200,
            lock_end_ts: 0,
        };

        assert_eq!(70, times.remaining_duration(130).unwrap());
        assert_eq!(0, times.remaining_duration(9999).unwrap());
        assert_eq!(30, times.passed_duration(130).unwrap());
        assert_eq!(199, times.reward_upper_bound(199));
        assert_eq!(200, times.reward_upper_bound(201));
        assert_eq!(100, times.reward_begin_ts().unwrap());
        assert_eq!(110, times.reward_lower_bound(110).unwrap());
    }

    #[test]
    fn test_time_tracker_end_reward() {
        let mut times = TimeTracker {
            duration_sec: 80,
            reward_end_ts: 200,
            lock_end_ts: 0,
        };

        times.end_reward(140).unwrap();
        assert_eq!(times.duration_sec, 20);
        assert_eq!(times.reward_end_ts, 140);
    }

    #[test]
    fn test_funds_tracker() {
        let funds = FundsTracker {
            total_funded: 100,
            total_refunded: 50,
            total_accrued_to_stakers: 30,
        };

        assert_eq!(20, funds.pending_amount().unwrap());
    }
}
