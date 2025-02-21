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
		println!("data {:?}", data.len());
		// convert to the format that the solver expects
		let data: Vec<hydration_solver::types::Asset> = data
			.into_iter()
			.map(|v| {
				let (c, asset_id, reserve, hub_reserve, decimals, fee, hub_fee) = v;
				match c {
					0 => hydration_solver::types::Asset::Omnipool(hydration_solver::types::OmnipoolAsset {
						asset_id,
						decimals,
						reserve,
						hub_reserve,
						fee,
						hub_fee,
					}),
					1 => hydration_solver::types::Asset::StableSwap(hydration_solver::types::StableSwapAsset {
						asset_id,
						decimals,
						reserve,
						fee,
					}),
					_ => {
						panic!("unsupported pool asset!")
					}
				}
			})
			.collect();

		let s = hydration_solver::v3::SolverV3::solve(vec![], data);
		vec![pallet_ice::types::ResolvedIntent {
			intent_id: 0,
			amount_in: 123,
			amount_out: 123,
		}]
	}
}
