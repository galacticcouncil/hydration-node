use hydradx_runtime::opaque::Block;
use hydration_solver::{omni, HydrationSolver};
use pallet_ice::traits::Solver;
use parking_lot::Mutex;
use primitives::{AccountId, AssetId};
use std::sync::Arc;

#[derive(Clone)]
pub struct SolutionContainer(pub Arc<LocalSolutionStore>);

impl SolutionContainer {
	pub fn new() -> Self {
		SolutionContainer(Arc::new(LocalSolutionStore::new()))
	}

	pub fn solution_store(&self) -> primitives::SolutionPtr {
		self.0.clone()
	}
}

pub struct LocalSolutionStore(Mutex<u32>);

impl LocalSolutionStore {
	fn new() -> Self {
		LocalSolutionStore(Mutex::new(0))
	}
}

impl primitives::SolutionStore for LocalSolutionStore {
	fn get_solution(&self) -> u32 {
		let s = hydration_solver::omni::OmniSolver::<
			AccountId,
			AssetId,
			hydradx_adapters::ice::OmnipoolDataProvider<hydradx_runtime::Runtime>,
		>::solve(vec![]);
		*self.0.lock()
	}
}

impl hydradx_traits::ice::SolverSolution<u32> for LocalSolutionStore {
	fn set_solution(&self, solution: u32) {
		let mut data = self.0.lock();
		*data = solution;
	}
}
