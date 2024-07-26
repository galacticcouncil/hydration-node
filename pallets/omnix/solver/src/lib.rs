use crate::traits::OmniXSolver;
use hydradx_traits::router::{AssetPair, RouteProvider, RouterT};
use pallet_omnix::types::{Intent, IntentId, ResolvedIntent};
use std::collections::BTreeMap;

pub mod traits;

pub struct SolverSolution {
	pub intents: Vec<ResolvedIntent>,
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
	type Solution = SolverSolution;
	type Error = ();

	fn solve(
		intents: Vec<(IntentId, Intent<T::AccountId, <T as pallet_omnix::Config>::AssetId>)>,
	) -> Result<Self::Solution, Self::Error> {
		let mut resolved_intents = Vec::new();

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

			let route = RP::get_route(AssetPair::<<T as pallet_omnix::Config>::AssetId>::new(
				1u32.into(),
				asset_out,
			));
			let r = R::calculate_sell_trade_amounts(&route, lrna_out).unwrap();

			let amount_out = r.last().unwrap().amount_out;

			let resolved_intent = ResolvedIntent {
				intent_id: intent.0,
				amount_in,
				amount_out: amount_out,
			};
			resolved_intents.push(resolved_intent);
		}

		Ok(SolverSolution {
			intents: resolved_intents,
		})
	}
}
