use super::*;
use crate::*;

use frame_benchmarking::account;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use hydra_dx_math::types::Ratio;
use ice_support::Intent as IntentIce;
use ice_support::IntentData;
use ice_support::IntentDataInput;
use ice_support::IntentId;
use ice_support::Price;
use ice_support::Solution;
use ice_support::SwapData;
use ice_support::SwapParams;
use ice_support::MAX_NUMBER_OF_RESOLVED_INTENTS;
use orml_benchmarking::runtime_benchmarks;
use pallet_intent::types::Intent as IntentT;
use pallet_intent::types::IntentInput;
use pallet_intent::types::OnResolved;
use sp_runtime::DispatchResult;
use sp_std::collections::btree_map::BTreeMap;

const SEED: u32 = 1;

const HDX: AssetId = 0;
const DAI: AssetId = 2;

const TRIL: u128 = 1_000_000_000_000;
const QUINTIL: u128 = 1_000_000_000_000_000_000;

//Intent's deadline, 12hours
const DEADLINE: Option<u64> = Some(12 * 3_600 * 1_000);

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

		let counterparty: AccountId = account("counterparty", 1, SEED);

		fund(caller.clone(), HDX, 10_000 * TRIL)?;
		fund(caller.clone(), DAI, 10_000 * QUINTIL)?;
		fund(counterparty.clone(), DAI, 10_000 * QUINTIL)?;

		// Settlement conserves each asset by flow, not by balance: for every asset
		// `(intent_in + pool_out) - (intent_out + pool_in)` must cover the fee on the
		// matched volume. A lone intent leaves its out-asset with no inflow at all, so
		// the pair below matches directly and each side is paid slightly less than the
		// other put in, leaving the protocol fee in the holding pot.
		let hdx_in = 3000 * TRIL;
		let dai_in = 10 * QUINTIL;
		let hdx_out = hdx_in - IceFee::get().mul_ceil(hdx_in);
		let dai_out = dai_in - IceFee::get().mul_ceil(dai_in);

		let swap_params = SwapParams {
			asset_in: HDX,
			asset_out: DAI,
			amount_in: hdx_in,
			amount_out: dai_out,
			partial: false,
		};
		let counter_params = SwapParams {
			asset_in: DAI,
			asset_out: HDX,
			amount_in: dai_in,
			amount_out: hdx_out,
			partial: false,
		};

		let intent = IntentInput {
			data: IntentDataInput::Swap(swap_params.clone()),
			deadline: DEADLINE,
			on_resolved: Some(OnResolved::Forward {
				contract: primitives::EvmAddress::repeat_byte(1u8),
				data: BoundedVec::truncate_from(vec![255u8; 64]),
			}),
		};
		let counter_intent = IntentInput {
			data: IntentDataInput::Swap(counter_params.clone()),
			deadline: DEADLINE,
			on_resolved: None,
		};

		Intent::submit_intent(RawOrigin::Signed(caller.clone()).into(), intent)?;
		Intent::submit_intent(RawOrigin::Signed(counterparty.clone()).into(), counter_intent)?;
		let intents: Vec<(IntentId, IntentT)> = pallet_intent::Intents::<Runtime>::iter().collect();
		assert_eq!(intents.len() , 2);
		let id = intents.iter().find(|(_, i)| i.data.asset_in() == HDX).map(|(id, _)| *id).unwrap();
		let counter_id = intents.iter().find(|(_, i)| i.data.asset_in() == DAI).map(|(id, _)| *id).unwrap();

		let resolved_intents = vec![
			IntentIce { id, data: IntentData::Swap(SwapData::from(&swap_params)) },
			IntentIce { id: counter_id, data: IntentData::Swap(SwapData::from(&counter_params)) },
		];

		let mut cp: BTreeMap<AssetId, Price> = BTreeMap::new();
		assert!(cp.insert(HDX, Ratio{n: 10000, d: 3}).is_none());
		for i in 1..(MAX_NUMBER_OF_RESOLVED_INTENTS * 2) {
			assert!(cp.insert(i, Ratio{n: 1, d: 3}).is_none());
		}

		let score = 0;
		let s = Solution::new(resolved_intents.try_into().unwrap(), BoundedVec::new(), score);

		assert!(LazyExecutor::call_queue(0).is_none());
		assert!(Intent::get_intent(id).is_some());
		assert!(Intent::get_intent(counter_id).is_some());
	}: { ICE::submit_solution(RawOrigin::None.into(), s)? }
	verify {
		assert!(Intent::get_intent(id).is_none());
		assert!(Intent::get_intent(counter_id).is_none());
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
