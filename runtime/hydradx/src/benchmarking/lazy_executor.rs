use super::*;
use crate::*;

use frame_benchmarking::account;
use frame_system::RawOrigin;
use hydradx_traits::lazy_executor::Source;
use orml_benchmarking::runtime_benchmarks;
use sp_runtime::DispatchResult;

const SEED: u32 = 1;

const HDX: AssetId = 0;

const TRIL: u128 = 1_000_000_000_000;

fn fund(to: AccountId, currency: AssetId, amount: Balance) -> DispatchResult {
	Currencies::deposit(currency, &to, amount)
}

runtime_benchmarks! {
	{Runtime, pallet_lazy_executor }

	dispatch_top_base_weight {

		//NOTE: treasury need balance otherwise it can't collect fees < ED
		Currencies::update_balance(
			RawOrigin::Root.into(),
			Treasury::account_id(),
			HDX,
			(10_000 * TRIL) as i128,
		)?;

		let acc = account::<AccountId>("origin", 0, SEED);
		fund(acc.clone(), HDX, 10_000 * TRIL)?;
		let call: Vec<u8> = RuntimeCall::Balances(pallet_balances::Call::transfer_keep_alive{
			dest: acc.clone(),
			value: 0
		}).encode();

		LazyExecutor::add_to_queue(Source::ICE(1_u128), acc, call.try_into().unwrap())?;

		assert!(LazyExecutor::call_queue(0).is_some());
	}: { LazyExecutor::dispatch_top(RawOrigin::None.into())? }
	verify {
		assert!(LazyExecutor::call_queue(0).is_none());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use orml_benchmarking::impl_benchmark_test_suite;
	use sp_runtime::BuildStorage;

	const LRNA: AssetId = 1;
	const DAI: AssetId = 2;

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<crate::Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_asset_registry::GenesisConfig::<crate::Runtime> {
			registered_assets: vec![
				(
					Some(LRNA),
					Some(b"LRNA".to_vec().try_into().unwrap()),
					1_000u128,
					None,
					None,
					None,
					true,
				),
				(
					Some(DAI),
					Some(b"DAI".to_vec().try_into().unwrap()),
					1_000u128,
					None,
					None,
					None,
					true,
				),
			],
			native_asset_name: b"HDX".to_vec().try_into().unwrap(),
			native_existential_deposit: NativeExistentialDeposit::get(),
			native_decimals: 12,
			native_symbol: b"HDX".to_vec().try_into().unwrap(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		sp_io::TestExternalities::new(t)
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
