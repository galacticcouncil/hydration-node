use crate::AccountId;
use hydradx_runtime::RuntimeCall;

pub struct StakingInitialState;

impl StakingInitialState {
	pub fn get_native_endowed_accounts(&self) -> Vec<(AccountId, u128)> {
		let staking_account = pallet_staking::Pallet::<hydradx_runtime::Runtime>::pot_account_id();

		vec![(staking_account, 10_000_000_000_000)]
	}
	pub fn calls(&self) -> Vec<RuntimeCall> {
		vec![RuntimeCall::Staking(pallet_staking::Call::initialize_staking {})]
	}
}

pub fn staking_state() -> StakingInitialState {
	StakingInitialState {}
}
