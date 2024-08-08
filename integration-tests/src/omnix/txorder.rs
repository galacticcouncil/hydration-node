use super::*;
use frame_support::dispatch::{DispatchInfo, GetDispatchInfo};
use frame_support::weights::Weight;
use hydradx_runtime::{CheckedExtrinsic, Executive, SignedExtra};
use sp_runtime::traits::SignedExtension;

/*
#[test]
fn tx_priority_should_be_correct() {
	Hydra::execute_with(|| {
		let callr = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: BTC },
		);

		let add_call =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::add_liquidity { asset: 10, amount: 100 });
		let sell_call = hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::sell {
			asset_in: 10,
			asset_out: 20,
			amount: 100,
			min_buy_amount: 0,
		});

		let add_info = callr.get_dispatch_info();

		dbg!(&add_info);

		let len: usize = 100;

		let tip0: u128 = 0;
		let tip1: u128 = 1_000_000_000_000;
		let tip2: u128 = 10_000_000_000_000;

		let call = add_call;
		let r = pallet_omnix::order::SetPriority::<hydradx_runtime::Runtime>::new()
			.validate(&AccountId::from(BOB), &call, &call.get_dispatch_info(), len)
			.unwrap();

		dbg!(&r);

		let r0 = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(tip0)
			.validate(&AccountId::from(BOB), &call, &call.get_dispatch_info(), len)
			.unwrap();

		dbg!(r.combine_with(r0).priority);

		let call = sell_call;
		let r = pallet_omnix::order::SetPriority::<hydradx_runtime::Runtime>::new()
			.validate(&AccountId::from(BOB), &call, &call.get_dispatch_info(), len)
			.unwrap();

		dbg!(&r);

		let r0 = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(tip0)
			.validate(&AccountId::from(BOB), &call, &call.get_dispatch_info(), len)
			.unwrap();

		dbg!(r.combine_with(r0).priority);
		/*
		let r1 = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(tip1).validate(
			&AccountId::from(BOB),
			&call,
			&info,
			len,
		).unwrap();

		let r2 = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(tip2).validate(
			&AccountId::from(BOB),
			&call,
			&info,
			len,
		).unwrap();

		dbg!(r1.priority);
		dbg!(r2.priority);
		 */

		//Executive::apply_extrinsic()
		//CheckedExtrinsic::validate()
	});
}

 */
