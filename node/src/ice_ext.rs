use hydration_solver::omni;
use pallet_ice::traits::Solver;
use primitives::{AccountId, AssetId};
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
	fn get_solution(&self) -> u32 {
		let s = hydration_solver::omni::OmniSolver::<
			AccountId,
			AssetId,
			hydradx_adapters::ice::OmnipoolDataProvider<hydradx_runtime::Runtime>,
		>::solve(vec![]);
		234u32
	}
}
