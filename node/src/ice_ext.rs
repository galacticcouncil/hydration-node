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
	fn get_solution(
		&self,
		intents: Vec<pallet_ice::api::IntentRepr>,
		data: Vec<pallet_ice::api::DataRepr>,
	) -> Vec<pallet_ice::types::ResolvedIntent> {
		// convert to the format that the solver expects
		let data = hydration_solver::types::convert_data_repr(data);
		let intents = hydration_solver::types::convert_intent_repr(intents);

		let s = hydration_solver::v3::SolverV3::solve(intents, data);

		if let Ok(solution) = s {
			solution
				.resolved_intents
				.iter()
				.map(|v| pallet_ice::types::ResolvedIntent {
					intent_id: v.intent_id,
					amount_in: v.amount_in,
					amount_out: v.amount_out,
				})
				.collect()
		} else {
			vec![]
		}
	}
}
