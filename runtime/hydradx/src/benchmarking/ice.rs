use super::*;
use crate::*;

use frame_benchmarking::account;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use hydra_dx_math::types::Ratio;
use ice_support::Intent as IntentIce;
use ice_support::IntentData;
use ice_support::IntentId;
use ice_support::Price;
use ice_support::Solution;
use ice_support::SwapData;
use ice_support::SwapType;
use ice_support::MAX_NUMBER_OF_RESOLVED_INTENTS;
use orml_benchmarking::runtime_benchmarks;
use pallet_intent::types::Intent as IntentT;
use sp_runtime::DispatchResult;
use sp_std::collections::btree_map::BTreeMap;

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
	{Runtime, pallet_ice }

	submit_solution {
		let caller: AccountId = account("caller", 0, SEED);

		//NOTE: treasury need balance otherwise it can't collect fees < ED
		Currencies::update_balance(
			RawOrigin::Root.into(),
			Treasury::account_id(),
			HDX,
			(10_000 * TRIL) as i128,
		)?;

		//NOTE: fund ICE's account so we can resolve intent without trade or another intent
		Currencies::update_balance(
			RawOrigin::Root.into(),
			ICE::get_pallet_account(),
			DAI,
			(10 * QUINTIL) as i128,
		)?;


		fund(caller.clone(), HDX, 10_000 * TRIL)?;
		fund(caller.clone(), DAI, 10_000 * QUINTIL)?;

		let cb: Vec<u8> = RuntimeCall::Tokens(orml_tokens::Call::transfer{
			dest: caller.clone(),
			currency_id: 5,
			amount: 10 * TRIL
		}).encode();

		let intent_data =  IntentData::Swap(SwapData {
			asset_in: HDX,
			asset_out: DAI,
			amount_in: 3000 * TRIL,
			amount_out: 10 * QUINTIL,
			swap_type: SwapType::ExactIn,
			partial: false,
		});

		let intent = IntentT {
			data: intent_data.clone(),
			deadline: DEADLINE,
			on_success: Some(cb.clone().try_into().unwrap()),
			on_failure: Some(cb.clone().try_into().unwrap()),
		};

		Intent::submit_intent(RawOrigin::Signed(caller.clone()).into(), intent)?;
		let intents: Vec<(IntentId, IntentT)> = pallet_intent::Intents::<Runtime>::iter().collect();
		assert_eq!(intents.len() , 1);
		let (id, _) = intents[0];

		let resolved_intents = vec![IntentIce {
			id,
			data: intent_data,
		}];

		let mut cp: BTreeMap<AssetId, Price> = BTreeMap::new();
		assert!(cp.insert(HDX, Ratio{n: 10000, d: 3}).is_none());
		for i in 1..(MAX_NUMBER_OF_RESOLVED_INTENTS * 2) {
			assert!(cp.insert(i, Ratio{n: 1, d: 3}).is_none());
		}

		let score = 0;
		let s = Solution {
			resolved_intents: resolved_intents.try_into().unwrap(),
			trades: BoundedVec::new(),
			clearing_prices: cp,
			score,
		};

		assert!(LazyExecutor::call_queue(0).is_none());
		assert!(Intent::get_intent(id).is_some());
	}: { ICE::submit_solution(RawOrigin::None.into(), s, 1)? }
	verify {
		assert!(Intent::get_intent(id).is_none());
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
