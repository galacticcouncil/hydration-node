use frame_support::weights::Weight;
use pallet_evm::GasWeightMapping;
use sp_core::Get;

pub struct FixedHydraGasWeightMapping<T>(core::marker::PhantomData<T>);
impl<T: pallet_evm::Config> GasWeightMapping for FixedHydraGasWeightMapping<T> {
	fn gas_to_weight(gas: u64, without_base_weight: bool) -> Weight {
		//We use this base weight as we don't wanna include the swap weights of normal substrate transactions
		//Otherwise transactions with weight smaller than swap would fail with OutOfGas error, during the tx execution
		let base_weight = frame_support::weights::constants::ExtrinsicBaseWeight::get();
		let mut weight = T::WeightPerGas::get().saturating_mul(gas);
		if without_base_weight {
			weight = weight.saturating_sub(base_weight);
		}
		// Apply a gas to proof size ratio based on BlockGasLimit
		let ratio = T::GasLimitPovSizeRatio::get();
		if ratio > 0 {
			let proof_size = gas.saturating_div(ratio);
			*weight.proof_size_mut() = proof_size;
		}

		weight
	}
	fn weight_to_gas(weight: Weight) -> u64 {
		weight.div(T::WeightPerGas::get().ref_time()).ref_time()
	}
}
