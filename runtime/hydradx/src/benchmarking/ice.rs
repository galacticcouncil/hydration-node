use super::*;
use crate::*;

use frame_benchmarking::account;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use hydradx_traits::price::PriceProvider;
use ice_support::Intent as IntentIce;
use ice_support::IntentData;
use ice_support::IntentDataInput;
use ice_support::IntentId;
use ice_support::RoutingState;
use ice_support::RoutingTarget;
use ice_support::Solution;
use ice_support::SolverMode;
use ice_support::SwapData;
use ice_support::SwapParams;
use orml_benchmarking::runtime_benchmarks;
use pallet_intent::types::Intent as IntentT;
use pallet_intent::types::IntentInput;
use pallet_intent::types::OnResolved;
use sp_runtime::DispatchError;
use sp_runtime::DispatchResult;
use sp_runtime::FixedU128;
use sp_runtime::Permill;

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

fn register_asset(id: AssetId) -> Result<AssetId, DispatchError> {
	AssetRegistry::register(
		RawOrigin::Root.into(),
		Some(id),
		// Names are unique in the registry, so they carry the id.
		Some(
			alloc::format!("BENCH{id}")
				.into_bytes()
				.try_into()
				.map_err(|_| "name")?,
		),
		pallet_asset_registry::AssetType::Token,
		Some(1_000),
		None,
		None,
		None,
		None,
		true,
	)?;
	Ok(id)
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

		// The holding pot must already hold HDX: `move_locked_funds` repatriates the
		// intent input into it, and `pallet_balances` refuses to create the
		// beneficiary (`DeadAccount`).
		fund(ICE::get_pallet_account(), HDX, 10_000 * TRIL)?;

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

	// A fee equal to `T::MatchedFee` kills the entry instead of writing it, so the
	// benchmarked value must differ from the default to measure the write.
	set_protocol_fee {
		let default_fee = <Runtime as pallet_ice::Config>::MatchedFee::get();
		let fee = if default_fee == Permill::from_percent(1) {
			Permill::from_percent(2)
		} else {
			Permill::from_percent(1)
		};
		assert_eq!(ICE::protocol_fee(), default_fee);
	}: { ICE::set_protocol_fee(RawOrigin::Root.into(), fee)? }
	verify {
		assert_eq!(ICE::protocol_fee(), fee);
	}

	// `SolverMode::default()` kills the entry; benchmark a non-default mode so the
	// write is measured.
	set_solver_mode {
		let mode = SolverMode::Disabled;
		assert_eq!(ICE::solver_mode(), SolverMode::default());
	}: { ICE::set_solver_mode(RawOrigin::Root.into(), mode)? }
	verify {
		assert_eq!(ICE::solver_mode(), SolverMode::Disabled);
	}

	// One weight covers every target, so the benchmark writes the largest key there
	// is: a full batch of the widest member type.
	// The per-intent cost of deriving a DCA's oracle floor.
	//
	// `submit_solution` already multiplies its base weight by the resolved-intent
	// count, so charging this the same way covers a solution made entirely of DCA
	// intents that all take the graph path. It over-charges a solution of plain
	// swaps, which never price at all — the safe direction.
	//
	// The graph is built here rather than measured on genesis on purpose: the cost
	// is a function of how many venues exist, and a benchmark chain has none. The
	// setup below is deliberately larger than mainnet (13 Omnipool assets and 26
	// Aave wraps as of 2026-09), so the number stays conservative as the chain
	// grows. Uniswap pools are the exception — they cost three EVM view calls each
	// and cannot be deployed here, so a chain with many registered pools needs this
	// re-measured.
	price_derivation {
		crate::benchmarking::omnipool::init()?;

		let omnipool_assets = 24u32;
		let mut graph_assets: Vec<AssetId> = Vec::new();
		let mut candidate: AssetId = 500_000;
		while graph_assets.len() < omnipool_assets as usize {
			// The benchmark genesis already holds assets; take the next free ids
			// rather than assuming a range is clear.
			if pallet_asset_registry::Assets::<Runtime>::get(candidate).is_none() {
				let asset = register_asset(candidate)?;
				graph_assets.push(asset);
			}
			candidate += 1;
		}
		for asset in graph_assets.iter().copied() {
			let acc = Omnipool::protocol_account();
			Currencies::update_balance(RawOrigin::Root.into(), acc.clone(), asset, (1_000 * QUINTIL) as i128)?;
			Omnipool::add_token(
				RawOrigin::Root.into(),
				asset,
				FixedU128::from(1),
				Permill::from_percent(100),
				acc,
			)?;
		}

		// Wraps are plain storage for the pricing graph, so a realistic count is free
		// to set up.
		let wraps: Vec<(AssetId, AssetId)> = (0..ice_support::MAX_ROUTING_BATCH)
			.map(|i| (3_000 + i, 4_000 + i))
			.collect();
		ICE::update_routing(
			RawOrigin::Root.into(),
			RoutingTarget::AaveWraps(wraps.try_into().unwrap()),
			Some(RoutingState::Included),
		)?;

		// Worst case: both legs are in the graph, so the fast path is skipped and the
		// search runs, but nothing prices, so every candidate is tried before the
		// floor is given up on.
		let (asset_in, asset_out) = (graph_assets[0], graph_assets[graph_assets.len() - 1]);
		assert!(pallet_omnipool::Assets::<Runtime>::contains_key(asset_in));
		assert!(pallet_omnipool::Assets::<Runtime>::contains_key(asset_out));
	}: {
		assert!(crate::ice_oracle_routes::DerivedRouteShortPrice::get_price(asset_in, asset_out).is_none());
	}

	update_routing {
		let pools: Vec<sp_core::H160> = (0..ice_support::MAX_ROUTING_BATCH)
			.map(|i| sp_core::H160::repeat_byte(i as u8))
			.collect();
		let target = RoutingTarget::UniswapV3Pools(pools.try_into().unwrap());
		assert_eq!(ICE::routing(&target), None);
	}: { ICE::update_routing(RawOrigin::Root.into(), target.clone(), Some(RoutingState::Included))? }
	verify {
		assert_eq!(ICE::routing(&target), Some(RoutingState::Included));
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
