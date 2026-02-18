use super::*;
use crate::*;

use frame_benchmarking::account;
use frame_system::RawOrigin;
use ice_support::IntentData;
use ice_support::IntentId;
use ice_support::SwapData;
use ice_support::SwapType;
use orml_benchmarking::runtime_benchmarks;
use pallet_intent::types::Intent as IntentT;
use pallet_intent::types::MAX_DATA_SIZE;
use sp_runtime::DispatchResult;

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

		//NOTE: it's ok to use junk, we are not really dispatching `cb`
		let cb: Vec<u8> = vec![255; MAX_DATA_SIZE as usize];

		let intent = IntentT {
			data: IntentData::Swap(SwapData {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: 3000 * TRIL,
				amount_out: 10 * QUINTIL,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline: DEADLINE,
			on_success: Some(cb.clone().try_into().unwrap()),
			on_failure: Some(cb.try_into().unwrap()),
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

		//NOTE: it's ok to use junk, we are not really dispatching `cb`
		let cb: Vec<u8> = vec![255; MAX_DATA_SIZE as usize];

		let intent = IntentT {
			data: IntentData::Swap(SwapData {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: 3000 * TRIL,
				amount_out: 10 * QUINTIL,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline: DEADLINE,
			on_success: Some(cb.clone().try_into().unwrap()),
			on_failure: Some(cb.try_into().unwrap()),
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

		//NOTE: it's ok to use junk, we are not really dispatching it.
		let on_success: Vec<u8> = vec![255; MAX_DATA_SIZE as usize];

		//NOTE: this must be valid(decodeable) call otherwise it won't be added to LazyExecutor's
		//queue.
		let on_failure: Vec<u8> = RuntimeCall::Tokens(orml_tokens::Call::transfer{
			dest: caller.clone(),
			currency_id: 5,
			amount: 10 * TRIL
		}).encode();

		let intent = IntentT {
			data: IntentData::Swap(SwapData {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: 3000 * TRIL,
				amount_out: 10 * QUINTIL,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline: DEADLINE,
			on_success: Some(on_success.clone().try_into().unwrap()),
			on_failure: Some(on_failure.clone().try_into().unwrap()),
		};

		Intent::submit_intent(RawOrigin::Signed(caller.clone()).into(), intent)?;
		let intents: Vec<(IntentId, IntentT)> = pallet_intent::Intents::<Runtime>::iter().collect();
		assert_eq!(intents.len() , 1);

		let (id, _) = intents[0];

		Timestamp::set_timestamp(DEADLINE + 10);
	}: _(RawOrigin::Signed(cleaner), id)
	verify {
		assert_eq!(Intent::get_intent(id), None);
		assert!(LazyExecutor::call_queue(0).is_some())
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
