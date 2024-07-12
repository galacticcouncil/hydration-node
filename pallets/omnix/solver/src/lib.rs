use crate::traits::OmniXSolver;
use hydradx_traits::router::{AssetPair, RouteProvider, RouterT};
use pallet_omnix::types::{Intent, IntentId, ResolvedIntent};
use std::collections::BTreeMap;

pub mod traits;

pub struct SolverSolution<AssetId> {
	pub intents: Vec<ResolvedIntent>,
	pub sell_prices: Vec<(AssetId, (u128, u128))>,
	pub buy_prices: Vec<(AssetId, (u128, u128))>,
}

pub struct SimpleSolver<T, R, RP>(sp_std::marker::PhantomData<(T, R, RP)>);

impl<T: pallet_omnix::Config, R, RP> OmniXSolver<(IntentId, Intent<T::AccountId, <T as pallet_omnix::Config>::AssetId>)>
	for SimpleSolver<T, R, RP>
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
		let mut resolved_intents = Vec::new();

		let mut sell_prices = BTreeMap::new();
		let mut buy_prices = BTreeMap::new();

		for intent in intents {
			//TODO: handle exact in and exact out
			let asset_in = intent.1.swap.asset_in;
			let asset_out = intent.1.swap.asset_out;
			let amount_in = intent.1.swap.amount_in;

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

			let resolved_intent = ResolvedIntent {
				intent_id: intent.0,
				amount: amount_in,
			};
			sell_prices.entry(asset_in).or_insert(asset_in_sell_price);
			sell_prices.entry(asset_out).or_insert(asset_out_sell_price);
			buy_prices.entry(asset_out).or_insert(asset_out_buy_price);
			buy_prices.entry(asset_in).or_insert(asset_in_buy_price);
			resolved_intents.push(resolved_intent);
		}

		Ok(SolverSolution {
			intents: resolved_intents,
			sell_prices: sell_prices.into_iter().collect(),
			buy_prices: buy_prices.into_iter().collect(),
		})
	}
}
