use crate::traits::OmniXSolver;
use hydra_dx_math::omnipool::types::I129;
use orml_traits::get_by_key::GetByKey;
use pallet_omnix::types::{Intent, IntentId, ResolvedIntent};

pub mod traits;

pub struct OneIntentSolver<T>(sp_std::marker::PhantomData<T>);

pub struct SolverSolution<AssetId> {
	pub intents: Vec<ResolvedIntent>,
	pub sell_prices: Vec<(AssetId, (u128, u128))>,
	pub buy_prices: Vec<(AssetId, (u128, u128))>,
}

impl<T: pallet_omnix::Config + pallet_omnipool::Config>
	OmniXSolver<(IntentId, Intent<T::AccountId, <T as pallet_omnix::Config>::AssetId>)> for OneIntentSolver<T>
where
	<T as pallet_omnix::Config>::AssetId: Into<<T as pallet_omnipool::Config>::AssetId>,
{
	type Solution = SolverSolution<<T as pallet_omnix::Config>::AssetId>;
	type Error = ();

	fn solve(
		intents: Vec<(IntentId, Intent<T::AccountId, <T as pallet_omnix::Config>::AssetId>)>,
	) -> Result<Self::Solution, Self::Error> {
		// this solves only one intent
		if intents.len() != 1 {
			return Err(());
		}

		let asset_in = intents[0].1.swap.asset_in;
		let asset_out = intents[0].1.swap.asset_out;
		let amount_in = intents[0].1.swap.amount_in;

		let asset_state = pallet_omnipool::Pallet::<T>::load_asset_state(asset_in.into()).unwrap();

		let state_changes = hydra_dx_math::omnipool::calculate_sell_for_hub_asset_state_changes(
			&(&asset_state).into(),
			amount_in,
			I129::default(),
			0,
		)
		.unwrap();

		let lrna_out = *state_changes.asset.delta_hub_reserve;
		let asset_in_sell_price = (amount_in, *state_changes.asset.delta_hub_reserve);

		let asset_state = pallet_omnipool::Pallet::<T>::load_asset_state(asset_out.into()).unwrap();
		let (asset_fee, _) = <T as pallet_omnipool::Config>::Fee::get(&asset_out.into());

		let state_changes = hydra_dx_math::omnipool::calculate_sell_hub_state_changes(
			&(&asset_state).into(),
			lrna_out,
			asset_fee,
			I129::default(),
			0,
		)
		.unwrap();

		let lrna_in = *state_changes.asset.delta_hub_reserve;
		debug_assert!(
			lrna_in == lrna_out,
			"lrna_in != lrna_out {:?} != {:?}",
			lrna_in,
			lrna_out
		);

		let amount_out = *state_changes.asset.delta_reserve;
		let asset_out_buy_price = (amount_out, lrna_out);

		let asset_in_buy_price = asset_in_sell_price; //TODO: figure out
		let asset_out_sell_price = asset_out_buy_price; //TODO: figure out

		let resolved_intents = vec![pallet_omnix::types::ResolvedIntent {
			intent_id: intents[0].0,
			amount: amount_in,
		}];
		let sell_prices = vec![(asset_in, asset_in_sell_price), (asset_out, asset_out_sell_price)];
		let buy_prices = vec![(asset_out, asset_out_buy_price), (asset_in, asset_in_buy_price)];

		Ok(SolverSolution {
			intents: resolved_intents,
			sell_prices,
			buy_prices,
		})
	}
}
