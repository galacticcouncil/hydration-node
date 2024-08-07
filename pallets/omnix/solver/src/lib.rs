use crate::traits::OmniXSolver;
use hydradx_traits::router::{AssetPair, RouteProvider, RouterT};
use pallet_omnix::engine::{BoundedRoute, Instruction};
use pallet_omnix::types::{Balance, Intent, IntentId, ResolvedIntent};
use sp_std::collections::btree_map::BTreeMap;

pub mod traits;

pub struct SolverSolution<AccountId, AssetId> {
	pub intents: Vec<ResolvedIntent>,
	pub instructions: Vec<Instruction<AccountId, AssetId>>,
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
	type Solution = SolverSolution<T::AccountId, T::AssetId>;
	type Error = ();

	fn solve(
		intents: Vec<(IntentId, Intent<T::AccountId, <T as pallet_omnix::Config>::AssetId>)>,
	) -> Result<Self::Solution, Self::Error> {
		let mut resolved_intents = Vec::new();

		let mut transfer_in_instructions: Vec<Instruction<T::AccountId, T::AssetId>> = Vec::new();
		let mut transfer_out_instructions = Vec::new();
		let mut trades_instructions = Vec::new();

		let mut amounts_in: BTreeMap<T::AssetId, Balance> = BTreeMap::new();
		let mut amounts_out: BTreeMap<T::AssetId, Balance> = BTreeMap::new();

		for (intent_id, intent) in intents {
			let asset_in = intent.swap.asset_in;
			let asset_out = intent.swap.asset_out;
			let amount_in = intent.swap.amount_in;
			let amount_out = intent.swap.amount_in;

			transfer_in_instructions.push(Instruction::TransferIn {
				who: intent.who.clone(),
				asset_id: asset_in,
				amount: amount_in,
			});

			transfer_out_instructions.push(Instruction::TransferOut {
				who: intent.who.clone(),
				asset_id: asset_out,
				amount: amount_out,
			});

			amounts_in
				.entry(asset_in)
				.and_modify(|e| *e += amount_in)
				.or_insert(amount_in);
			amounts_out
				.entry(asset_out)
				.and_modify(|e| *e += amount_out)
				.or_insert(amount_out);

			/*
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

			*/

			let resolved_intent = ResolvedIntent {
				intent_id,
				amount_in,
				amount_out,
			};
			resolved_intents.push(resolved_intent);
		}

		// Sell all for lrna
		for (asset_id, amount) in amounts_in.iter() {
			let amount_out = *amounts_out.get(&asset_id).unwrap_or(&0u128);

			if *amount > amount_out {
				let route = RP::get_route(AssetPair::<<T as pallet_omnix::Config>::AssetId>::new(
					*asset_id,
					1u32.into(),
				));
				trades_instructions.push(Instruction::SwapExactIn {
					asset_in: *asset_id,
					asset_out: 1u32.into(),                       // LRNA
					amount_in: amount.saturating_sub(amount_out), //Swap only difference
					amount_out: 0,
					route: BoundedRoute::try_from(route).unwrap(),
				});
			}
		}

		for (asset_id, amount) in amounts_out {
			let amount_in = *amounts_in.get(&asset_id).unwrap_or(&0u128);

			if amount > amount_in {
				let route = RP::get_route(AssetPair::<<T as pallet_omnix::Config>::AssetId>::new(
					1u32.into(),
					asset_id,
				));
				trades_instructions.push(Instruction::SwapExactOut {
					asset_in: 1u32.into(), // LRNA
					asset_out: asset_id,
					amount_in: u128::MAX,                         //TODO: what limit here
					amount_out: amount.saturating_sub(amount_in), //Swap only difference
					route: BoundedRoute::try_from(route).unwrap(),
				});
			}
		}

		let mut instructions = Vec::new();
		instructions.extend(transfer_in_instructions);
		instructions.extend(trades_instructions);
		instructions.extend(transfer_out_instructions);

		Ok(SolverSolution {
			intents: resolved_intents,
			instructions,
		})
	}
}
