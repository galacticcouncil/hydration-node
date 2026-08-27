use super::*;
use crate::*;

use frame_benchmarking::account;
use frame_system::RawOrigin;
use hydradx_traits::lazy_executor::MAX_FORWARD_DATA;
use ice_support::IntentDataInput;
use ice_support::IntentId;
use ice_support::SwapParams;
use orml_benchmarking::runtime_benchmarks;
use pallet_intent::types::Intent as IntentT;
use pallet_intent::types::IntentInput;
use pallet_intent::types::OnResolved;
use sp_runtime::DispatchResult;

// Worst-case `on_resolved`: a forward carrying the maximum opaque payload.
fn worst_case_forward() -> OnResolved {
	OnResolved::Forward {
		contract: primitives::EvmAddress::repeat_byte(1u8),
		data: sp_runtime::BoundedVec::truncate_from(vec![255u8; MAX_FORWARD_DATA as usize]),
	}
}

const SEED: u32 = 1;

const HDX: AssetId = 0;
const DAI: AssetId = 2;

const TRIL: u128 = 1_000_000_000_000;
const QUINTIL: u128 = 1_000_000_000_000_000_000;

//Intent's deadline, 12hours
const DEADLINE: u64 = 12 * 3_600 * 1_000;

fn fund(to: AccountId, currency: AssetId, amount: Balance) -> DispatchResult {
	Currencies::deposit(currency, &to, amount)
}

runtime_benchmarks! {
	{Runtime, pallet_intent }

	submit_intent {
		let caller: AccountId = account("caller", 0, SEED);

		fund(caller.clone(), HDX, 10_000 * TRIL)?;
		fund(caller.clone(), DAI, 10_000 * QUINTIL)?;

		let intent = IntentInput {
			data: IntentDataInput::Swap(SwapParams {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: 3000 * TRIL,
				amount_out: 10 * QUINTIL,
				partial: false,
			}),
			deadline: Some(DEADLINE),
			on_resolved: Some(worst_case_forward()),
		};

		let intents: Vec<(IntentId, IntentT)> = pallet_intent::Intents::<Runtime>::iter().collect();
		assert_eq!(intents.len() , 0);
	}: _(RawOrigin::Signed(caller), intent)
	verify {
		let intents: Vec<(IntentId, IntentT)> = pallet_intent::Intents::<Runtime>::iter().collect();
		assert_eq!(intents.len() , 1);
	}

	remove_intent {
		let caller: AccountId = account("caller", 0, SEED);

		fund(caller.clone(), HDX, 10_000 * TRIL)?;
		fund(caller.clone(), DAI, 10_000 * QUINTIL)?;

		let intent = IntentInput {
			data: IntentDataInput::Swap(SwapParams {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: 3000 * TRIL,
				amount_out: 10 * QUINTIL,
				partial: false,
			}),
			deadline: Some(DEADLINE),
			on_resolved: Some(worst_case_forward()),
		};

		Intent::submit_intent(RawOrigin::Signed(caller.clone()).into(), intent)?;
		let intents: Vec<(IntentId, IntentT)> = pallet_intent::Intents::<Runtime>::iter().collect();
		assert_eq!(intents.len() , 1);

		let (id, _) = intents[0];
	}: _(RawOrigin::Signed(caller), id)
	verify {
		assert_eq!(Intent::get_intent(id), None);
	}

	cleanup_intent {
		let caller: AccountId = account("caller", 0, SEED);
		let cleaner: AccountId = account("cleaner", 1, SEED);

		//NOTE: treasury need balance otherwise it can't collect fees < ED
		Currencies::update_balance(
			RawOrigin::Root.into(),
			Treasury::account_id(),
			HDX,
			(10_000 * TRIL) as i128,
		)?;

		fund(caller.clone(), HDX, 10_000 * TRIL)?;
		fund(caller.clone(), DAI, 10_000 * QUINTIL)?;

		let intent = IntentInput {
			data: IntentDataInput::Swap(SwapParams {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: 3000 * TRIL,
				amount_out: 10 * QUINTIL,
				partial: false,
			}),
			deadline: Some(DEADLINE),
			on_resolved: Some(worst_case_forward()),
		};

		Intent::submit_intent(RawOrigin::Signed(caller.clone()).into(), intent)?;
		let intents: Vec<(IntentId, IntentT)> = pallet_intent::Intents::<Runtime>::iter().collect();
		assert_eq!(intents.len() , 1);

		let (id, _) = intents[0];

		Timestamp::set_timestamp(DEADLINE + 10);
	}: _(RawOrigin::Signed(cleaner), id)
	verify {
		assert_eq!(Intent::get_intent(id), None);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use orml_benchmarking::impl_benchmark_test_suite;
	use sp_runtime::BuildStorage;

	const LRNA: AssetId = 1;

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<crate::Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_asset_registry::GenesisConfig::<crate::Runtime> {
			registered_assets: vec![
				(
					Some(LRNA),
					Some(b"LRNA".to_vec().try_into().unwrap()),
					1_000u128,
					None,
					None,
					None,
					true,
				),
				(
					Some(DAI),
					Some(b"DAI".to_vec().try_into().unwrap()),
					1_000u128,
					None,
					None,
					None,
					true,
				),
			],
			native_asset_name: b"HDX".to_vec().try_into().unwrap(),
			native_existential_deposit: NativeExistentialDeposit::get(),
			native_decimals: 12,
			native_symbol: b"HDX".to_vec().try_into().unwrap(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		sp_io::TestExternalities::new(t)
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
