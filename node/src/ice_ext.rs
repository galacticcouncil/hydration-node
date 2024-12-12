use hydradx_adapters::ice::OmnipoolDataProvider;
use pallet_ice::traits::OmnipoolInfo;
use sp_runtime::{PerThing, Permill};
use std::sync::Arc;

#[derive(Clone)]
pub struct SolverProvider(pub Arc<IceSolver>);

impl SolverProvider {
	pub fn new() -> Self {
		SolverProvider(Arc::new(IceSolver::new()))
	}

	pub fn solver_ptr(&self) -> pallet_ice::api::SolverPtr {
		self.0.clone()
	}
}

pub struct IceSolver;

impl IceSolver {
	fn new() -> Self {
		IceSolver {}
	}
}

impl pallet_ice::api::SolutionProvider for IceSolver {
	fn get_solution(&self) -> Vec<pallet_ice::types::ResolvedIntent> {
		println!("getting data");
		let data = OmnipoolDataProvider::<hydradx_runtime::Runtime>::assets(None);
		println!("data {:?}", data.len());
		// convert to the format that the solver expects
		let data = data
			.into_iter()
			.map(|v| hydration_solver::traits::OmnipoolAssetInfo {
				asset_id: v.asset_id,
				decimals: v.decimals,
				reserve: v.reserve,
				hub_reserve: v.hub_reserve,
				fee: (v.fee.deconstruct(), 1_000_000),
				hub_fee: (v.hub_fee.deconstruct(), 1_000_000),
			})
			.collect();

		let s = hydration_solver::v3::SolverV3::solve(vec![], data);
		vec![pallet_ice::types::ResolvedIntent{
			intent_id: 0,
			amount_in: 123,
			amount_out: 123,
		}]
	}
}
