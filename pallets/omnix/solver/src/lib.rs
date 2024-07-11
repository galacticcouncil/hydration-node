use crate::traits::OmniXSolver;
use hydradx_traits::router::{AssetPair, RouteProvider, RouterT};
use pallet_omnix::types::{Intent, IntentId, ResolvedIntent};

pub mod traits;

pub struct OneIntentSolver<T, R, RP>(sp_std::marker::PhantomData<(T, R, RP)>);

pub struct SolverSolution<AssetId> {
	pub intents: Vec<ResolvedIntent>,
	pub sell_prices: Vec<(AssetId, (u128, u128))>,
	pub buy_prices: Vec<(AssetId, (u128, u128))>,
}

impl<T: pallet_omnix::Config, R, RP> OmniXSolver<(IntentId, Intent<T::AccountId, <T as pallet_omnix::Config>::AssetId>)>
	for OneIntentSolver<T, R, RP>
where
	<T as pallet_omnix::Config>::AssetId: From<u32>,
	R: RouterT<
		T::RuntimeOrigin,
		<T as pallet_omnix::Config>::AssetId,
		u128,
		hydradx_traits::router::Trade<<T as pallet_omnix::Config>::AssetId>,
		hydradx_traits::router::AmountInAndOut<u128>,
	>,
	RP: RouteProvider<<T as pallet_omnix::Config>::AssetId>,
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

		let route = RP::get_route(AssetPair::<<T as pallet_omnix::Config>::AssetId>::new(
			asset_in,
			1u32.into(),
		));

		let r = R::calculate_sell_trade_amounts(&route, amount_in).unwrap();
		let lrna_out = r.last().unwrap().amount_out;
		let asset_in_sell_price = (amount_in, lrna_out);

		let route = RP::get_route(AssetPair::<<T as pallet_omnix::Config>::AssetId>::new(
			1u32.into(),
			asset_out,
		));
		let r = R::calculate_sell_trade_amounts(&route, lrna_out).unwrap();

		let amount_out = r.last().unwrap().amount_out;
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
