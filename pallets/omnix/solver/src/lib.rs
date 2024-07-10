use crate::traits::OmniXSolver;
use pallet_omnix::types::{Intent, IntentId, ResolvedIntent};
use sp_runtime::Permill;

pub mod traits;

pub struct OneIntentSolver<T>(sp_std::marker::PhantomData<T>);

pub struct SolverSolution<AssetId> {
	pub intents: Vec<ResolvedIntent>,
	pub sell_prices: Vec<(AssetId, (u128, u128))>,
	pub buy_prices: Vec<(AssetId, (u128, u128))>,
}

pub(crate) fn get_asset_lrna_price<T: pallet_omnipool::Config>(asset: T::AssetId, asset_fee: Permill) -> (u128, u128) {
	let asset_state = pallet_omnipool::Pallet::<T>::load_asset_state(asset).unwrap();
	let r = asset_fee.mul_ceil(asset_state.reserve);
	let h = asset_fee.mul_ceil(asset_state.hub_reserve);
	(asset_state.reserve - r, asset_state.hub_reserve)
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

		let resolved_intents = vec![pallet_omnix::types::ResolvedIntent {
			intent_id: intents[0].0,
			amount: 1_000_000_000_000,
		}];

		let asset_in = intents[0].1.swap.asset_in;
		let asset_out = intents[0].1.swap.asset_out;

		let sell_prices = vec![(asset_in, get_asset_lrna_price::<T>(asset_in.into(), Permill::zero()))];
		let buy_prices = vec![(
			asset_out,
			get_asset_lrna_price::<T>(asset_out.into(), Permill::from_float(0.0025)),
		)];

		Ok(SolverSolution {
			intents: resolved_intents,
			sell_prices,
			buy_prices,
		})
	}
}
