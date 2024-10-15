use crate::traits::ICESolver;
use crate::SolverSolution;
use hydra_dx_math::ratio::Ratio;
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::{AssetPair, RouteProvider, RouterT};
use pallet_ice::types::{Balance, BoundedRoute, Intent, IntentId, ResolvedIntent, TradeInstruction};
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::{FixedU128, SaturatedConversion, Saturating};
use sp_std::collections::btree_map::BTreeMap;
use std::collections::HashMap;

use clarabel::algebra::*;
use clarabel::solver::*;

fn calculate_tau_phi<T: pallet_ice::Config>(
	intents: &[(IntentId, Intent<T::AccountId, <T as pallet_ice::Config>::AssetId>)],
	asset_ids: &[T::AssetId],
	scaling: &HashMap<T::AssetId, f64>,
) -> (CscMatrix, CscMatrix)
where
	T::AssetId: From<u32> + sp_std::hash::Hash,
{
	let n = asset_ids.len();
	let m = intents.len();
	let mut tau = CscMatrix::zeros((n, m));
	let mut phi = CscMatrix::zeros((n, m));
	for (j, intent) in intents.iter().enumerate() {
		let sell_i = asset_ids.iter().position(|&tkn| tkn == intent.1.swap.asset_in).unwrap();
		let buy_i = asset_ids
			.iter()
			.position(|&tkn| tkn == intent.1.swap.asset_out)
			.unwrap();

		if let Some(scaling) = scaling.get(&intent.1.swap.asset_in) {
			tau.set_entry((sell_i, j), *scaling);
			phi.set_entry((buy_i, j), *scaling);
		} else {
			tau.set_entry((sell_i, j), 1.);
			phi.set_entry((buy_i, j), 1.);
		}
	}
	(tau, phi)
}

fn round_solution<T: pallet_ice::Config>(
	intents: &[Intent<T::AccountId, <T as pallet_ice::Config>::AssetId>],
	intent_deltas: Vec<f64>,
	tolerance: f64,
) -> Vec<f64> {
	let mut deltas = Vec::new();
	for i in 0..intents.len() {
		// don't leave dust in intent due to rounding error
		if intents[i].swap.amount_in as f64 + intent_deltas[i] < tolerance * intents[i].swap.amount_in as f64 {
			deltas.push(-(intents[i].swap.amount_in as f64));
		// don't trade dust amount due to rounding error
		} else if -intent_deltas[i] <= tolerance * intents[i].swap.amount_in as f64 {
			deltas.push(0.);
		} else {
			deltas.push(intent_deltas[i]);
		}
	}
	deltas
}

fn add_buy_deltas<T: pallet_ice::Config>(
	intents: &[Intent<T::AccountId, <T as pallet_ice::Config>::AssetId>],
	sell_deltas: Vec<f64>,
) -> Vec<(f64, f64)> {
	let mut deltas = Vec::new();
	for i in 0..intents.len() {
		let b = (-sell_deltas[i] * intents[i].swap.amount_out as f64 / intents[i].swap.amount_in as f64);
		deltas.push((sell_deltas[i], b));
	}
	deltas
}

fn diags(n: usize, m: usize, data: Vec<f64>) -> CscMatrix {
	let mut res = CscMatrix::zeros((n, m));
	for i in 0..n {
		res.set_entry((i, i), data[i]);
	}
	res
}

pub struct CVXSolver<T, R, RP, PP>(sp_std::marker::PhantomData<(T, R, RP, PP)>);

impl<T: pallet_ice::Config, R, RP, PP> ICESolver<(IntentId, Intent<T::AccountId, <T as pallet_ice::Config>::AssetId>)>
	for CVXSolver<T, R, RP, PP>
where
	<T as pallet_ice::Config>::AssetId: From<u32> + sp_std::hash::Hash,
	R: RouterT<
		T::RuntimeOrigin,
		<T as pallet_ice::Config>::AssetId,
		u128,
		hydradx_traits::router::Trade<<T as pallet_ice::Config>::AssetId>,
		hydradx_traits::router::AmountInAndOut<u128>,
	>,
	RP: RouteProvider<<T as pallet_ice::Config>::AssetId>,
	PP: PriceProvider<<T as pallet_ice::Config>::AssetId, Price = Ratio>,
{
	type Solution = SolverSolution<T::AssetId>;
	type Error = ();

	fn solve(
		intents: Vec<(IntentId, Intent<T::AccountId, <T as pallet_ice::Config>::AssetId>)>,
	) -> Result<Self::Solution, Self::Error> {
		let asset_ids: [T::AssetId; 3] = [0u32.into(), 20u32.into(), 2u32.into()];
		let tkns: [T::AssetId; 4] = [1u32.into(), 0u32.into(), 20u32.into(), 2u32.into()];
		let asset_reserves: [f64; 3] = [100000000., 10000000., 10000000. / 7.5];
		let hub_reserves: [f64; 3] = [1000000., 10000000., 10000000.];
		let reserve_map = asset_ids.iter().zip(asset_reserves.iter()).collect::<BTreeMap<_, _>>();

		let fees = [0.0025, 0.0025, 0.0025];
		let lrna_fees = [0.0005, 0.0005, 0.0005];

		let n = asset_ids.len();
		let m = intents.len();
		let k = 4 * n + m;

		//calculate tau, phi
		let mut scaling: HashMap<T::AssetId, f64> = asset_ids
			.iter()
			.map(|&tkn| (tkn, **reserve_map.get(&tkn).unwrap()))
			.collect();
		scaling.insert(1u32.into(), hub_reserves.sum());
		let (tau, phi) = calculate_tau_phi::<T>(&intents, &tkns, &scaling);

		//#----------------------------#
		//#          OBJECTIVE         #
		//#----------------------------#
		let P: CscMatrix<f64> = CscMatrix::zeros((k, k));

		let delta_lrna_coefs = hub_reserves.clone();
		let lambda_lrna_coefs = lrna_fees
			.iter()
			.enumerate()
			.map(|(i, l)| hub_reserves[i] * (l - 1.))
			.collect::<Vec<_>>();
		let zero_coefs = vec![0.; 2 * n];
		let mut d_coefs = Vec::new();
		for i in 0..m {
			let v = tau.get_entry((0, i)).unwrap_or(0.);
			d_coefs.push(-v);
		}
		let mut q = Vec::new();
		q.extend(delta_lrna_coefs);
		q.extend(lambda_lrna_coefs);
		q.extend(zero_coefs);
		q.extend(d_coefs);

		//#----------------------------#
		//#        CONSTRAINTS         #
		//#----------------------------#

		let mut A1: CscMatrix<f64> = CscMatrix::identity(k);
		A1.negate();
		let b1: Vec<f64> = vec![0.; k];
		let cone1 = NonnegativeConeT(k);

		// intents cannot sell more than they have
		let amm_coefs = CscMatrix::zeros((m, 4 * n));
		let d_coefs = CscMatrix::identity(m);
		let A2 = CscMatrix::hcat(&amm_coefs, &d_coefs);
		let b2 = intents
			.iter()
			.map(|intent| (intent.1.swap.amount_in as f64, intent.1.swap.asset_in)) //TODO: amohn tto f64 using amoutn and asset decimenals
			.map(|(amount, asset_in)| amount / scaling[&asset_in])
			.collect::<Vec<_>>();
		let cone2 = NonnegativeConeT(m);

		//leftover must be higher than required fees
		let A30 = CscMatrix::from(&[q.clone()]);
		let b30 = vec![0.];

		// other assets
		let intent_prices = intents
			.iter()
			.map(|intent| FixedU128::from_rational(intent.1.swap.amount_out, intent.1.swap.amount_in).to_float())
			.collect::<Vec<_>>();
		let lrna_coefs = CscMatrix::zeros((n, 2 * n));
		let delta_coefs = diags(n, n, asset_reserves.to_vec());
		let lambda_coefs = diags(
			n,
			n,
			asset_reserves
				.iter()
				.enumerate()
				.map(|(i, f)| f * (fees[i] - 1.))
				.collect(),
		);

		// d_coefs = sparse.csc_matrix([[phi[i,j]*intent_prices[j] - tau[i, j] for j in range(m)] for i in range(1,n+1)])

		let d = (1..n + 1)
			.map(|i| {
				(0..m)
					.map(|j| {
						phi.get_entry((i, j)).unwrap_or(0.) * intent_prices[j] - tau.get_entry((i, j)).unwrap_or(0.)
					})
					.collect::<Vec<_>>()
			})
			.collect::<Vec<_>>();
		let d_coefs = CscMatrix::from(&d);
		let A31 = CscMatrix::hcat(&lrna_coefs, &delta_coefs);
		let A31 = CscMatrix::hcat(&A31, &lambda_coefs);
		let A31 = CscMatrix::hcat(&A31, &d_coefs);
		let b31 = vec![0.; n];
		let A3 = CscMatrix::vcat(&A30, &A31);
		let cone3 = NonnegativeConeT(n + 1);

		// AMM invariants must not go down
		let mut A4 = CscMatrix::zeros((3 * n, k));
		let b4 = vec![1.; 3 * n];
		let mut cones4 = Vec::new();
		for i in 0..n {
			A4.set_entry((3 * i, i), -1.);
			A4.set_entry((3 * i, n + i), 1.);
			A4.set_entry((3 * i + 1, 2 * n + i), -1.);
			A4.set_entry((3 * i + 1, 3 * n + i), 1.);
			cones4.push(PowerConeT(0.5));
		}
		let A = CscMatrix::vcat(&A1, &A2);
		let A = CscMatrix::vcat(&A, &A3);
		let A = CscMatrix::vcat(&A, &A4);

		let mut b = Vec::new();
		b.extend(b1);
		b.extend(b2);
		b.extend(b30);
		b.extend(b31);
		b.extend(b4);

		let mut cones = vec![cone1, cone2, cone3];
		cones.extend(cones4);

		let settings = DefaultSettings::default();
		let mut solver = DefaultSolver::new(&P, &q, &A, &b, &cones, settings);
		solver.solve();

		let x = solver.solution.x;

		let mut new_amm_deltas = HashMap::new();
		let mut exec_intent_deltas = vec![0.; intents.len()];
		for i in 0..n {
			let tkn = asset_ids[i];
			new_amm_deltas.insert(tkn, (x[2 * n + i] - x[3 * n + i]) * asset_reserves[i]);
		}
		for i in 0..intents.len() {
			exec_intent_deltas[i] = -x[4 * n + i] * scaling[&intents[i].1.swap.asset_in];
		}

		let sell_deltas = round_solution::<T>(
			&intents.iter().map(|(_, intent)| intent.clone()).collect::<Vec<_>>(),
			exec_intent_deltas,
			0.0001,
		);

		let intent_deltas = add_buy_deltas::<T>(
			&intents.iter().map(|(_, intent)| intent.clone()).collect::<Vec<_>>(),
			sell_deltas,
		);

		assert_eq!(
			intent_deltas,
			vec![(-100., 700.), (-1500., 100000.), (-400., 50.), (0., -0.)]
		);

		Err(())
	}
}
