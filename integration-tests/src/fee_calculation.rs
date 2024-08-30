#![cfg(test)]

use crate::{oracle::hydradx_run_to_block, polkadot_test_net::*};
use frame_support::assert_ok;
use frame_support::dispatch::DispatchClass;
use frame_support::dispatch::GetDispatchInfo;
use frame_support::weights::constants::RocksDbWeight;
use hydradx_runtime::evm::precompiles::DISPATCH_ADDR;
use hydradx_runtime::{ExtrinsicBaseWeight, Runtime, Tokens, WeightToFee};
use hydradx_runtime::TransactionPayment;
use hydradx_runtime::EVM;
use orml_traits::MultiCurrency;
use pallet_evm::FeeCalculator;
use primitives::constants::currency::UNITS;
use primitives::constants::time::HOURS;
use sp_core::Encode;
use sp_core::U256;
use sp_runtime::{FixedU128, Permill};
use test_utils::assert_eq_approx;
use xcm_emulator::TestExt;
use frame_support::weights::WeightToFee as WeightToFeeTrait;
use sp_io::storage::changes_root;
use primitives::constants::chain::Weight;

pub const SWAP_ENCODED_LEN: u32 = 146; //We use this as this is what the UI send as length when omnipool swap is executed
const HDX_USD_SPOT_PRICE: f64 = 0.0266; //Current HDX price in USD on CoinGecko on 22th Feb, 2024
pub const ETH_USD_SPOT_PRICE: f64 = 2907.92; //Current HDX price in USD on CoinGecko on 22th Feb, 2024

#[ignore]
#[test]
fn check_weight_to_fee() {
	TestNet::reset();
	Hydra::execute_with(|| {
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		let mut base = frame_support::weights::constants::ExtrinsicBaseWeight::get(); //(124414000,0)

		let fee = hydradx_runtime::WeightToFee::weight_to_fee(&base);

		let w = Weight::from_parts(191_035_000, 11322)
			.saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads(24_u64))
			.saturating_add(<Runtime as frame_system::Config>::DbWeight::get().writes(6_u64));

		base.saturating_accrue(w);
		let fee2 = hydradx_runtime::WeightToFee::weight_to_fee(&base); //(1515449000, 11322)
		assert_eq!(fee, fee2)
	});
}

#[ignore]
#[test]
fn min_swap_fee() {
	TestNet::reset();
	Hydra::execute_with(|| {
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		let call = hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
			asset_in: DOT,
			asset_out: 2,
			amount: UNITS,
			min_buy_amount: 0,
		});

		let info = call.get_dispatch_info();
		let info_len = 146;
		let fee = TransactionPayment::compute_fee(info_len, &info, 0);
		let fee_in_cent = FixedU128::from_float(fee as f64 * HDX_USD_SPOT_PRICE).div(UNITS.into());
		let tolerance = FixedU128::from((2, (UNITS / 10_000)));
		println!("Swap tx fee in cents: {fee_in_cent:?}");

		assert_eq_approx!(
			fee_in_cent,
			FixedU128::from_float(0.009_909_846_329_778),
			tolerance,
			"The min fee should be ~0.01$ (1 cent)"
		);
	});
}

#[ignore]
#[test]
fn max_swap_fee() {
	TestNet::reset();
	Hydra::execute_with(|| {
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MaximumMultiplier::get(),
		);

		let call = hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
			asset_in: DOT,
			asset_out: 2,
			amount: UNITS,
			min_buy_amount: 0,
		});

		let info = call.get_dispatch_info();
		let info_len = 146; //We use this as this is what the UI send as length when omnipool swap is executed
		let fee = TransactionPayment::compute_fee(info_len, &info, 0);
		let fee_in_cent = FixedU128::from_float(fee as f64 * HDX_USD_SPOT_PRICE).div(UNITS.into());
		let tolerance = FixedU128::from((2, (UNITS / 10_000)));
		assert_eq_approx!(
			fee_in_cent,
			FixedU128::from_float(10.008_401_718_494_405),
			tolerance,
			"The max fee should be ~1000 cent (10$)"
		);
	});
}

#[ignore]
#[test]
fn substrate_and_evm_fee_growth_simulator_with_genesis_chain() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let prod_init_multiplier = FixedU128::from_u32(1);

		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(prod_init_multiplier);
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			evm_account(),
			HDX,
			1000000 * UNITS as i128,
		));

		init_omnipool();
		let block_weight = hydradx_runtime::BlockWeights::get()
			.get(DispatchClass::Normal)
			.max_total
			.unwrap();

		for (nonce, b) in (2..HOURS).enumerate() {
			//=HOURS {
			hydradx_run_to_block(b);
			hydradx_runtime::System::set_block_consumed_resources(block_weight, 0);
			let call =
				hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
					asset_in: HDX,
					asset_out: 2,
					amount: 10 * UNITS,
					min_buy_amount: 10000,
				});

			let info = call.get_dispatch_info();
			let fee = TransactionPayment::compute_fee(SWAP_ENCODED_LEN, &info, 0);

			let fee_in_cent = (fee as f64 * HDX_USD_SPOT_PRICE) / 1000000000000.0;
			let fee_in_cent = round(fee_in_cent);

			let evm_fee_in_cent = round(get_evm_fee_in_cent(nonce as u128));
			let next = TransactionPayment::next_fee_multiplier();

			let gas_price = hydradx_runtime::DynamicEvmFee::min_gas_price();

			println!("{b:?} - fee: ${fee_in_cent:?}  - evm_fee: ${evm_fee_in_cent:?} - multiplier: {next:?} - gas {gas_price:?}");
		}
	});
}

#[ignore]
#[test]
fn substrate_and_evm_fee_growth_simulator_with_idle_chain() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//We simulate that the chain has no activity so the MinimumMultiplier kept diverged to absolute minimum
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			evm_account(),
			HDX,
			1000000 * UNITS as i128,
		));

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			evm_account(),
			WETH,
			100000000 * UNITS as i128,
		));

		init_omnipool();
		let block_weight = hydradx_runtime::BlockWeights::get()
			.get(DispatchClass::Normal)
			.max_total
			.unwrap();

		for (nonce, b) in (2..HOURS).enumerate() {
			//=HOURS {
			hydradx_run_to_block(b);
			hydradx_runtime::System::set_block_consumed_resources(block_weight, 0);
			let call =
				hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
					asset_in: HDX,
					asset_out: 2,
					amount: 10 * UNITS,
					min_buy_amount: 10000,
				});

			let info = call.get_dispatch_info();
			let fee = TransactionPayment::compute_fee(SWAP_ENCODED_LEN, &info, 0);

			let fee_in_cent = (fee as f64 * HDX_USD_SPOT_PRICE) / 1000000000000.0;
			let fee_in_cent = round(fee_in_cent);

			let evm_fee_in_cent = round(get_evm_fee_in_cent(nonce as u128));
			let next = TransactionPayment::next_fee_multiplier();

			let gas_price = hydradx_runtime::DynamicEvmFee::min_gas_price();

			println!("{b:?} - fee: {fee:?}");
			//println!("{b:?} - fee: ${fee_in_cent:?}  - evm_fee: ${evm_fee_in_cent:?} - multiplier: {next:?} - gas {gas_price:?}");
		}
	});
}

pub fn get_evm_fee_in_cent(nonce: u128) -> f64 {
	let treasury_eth_balance = Tokens::free_balance(WETH, &Treasury::account_id());

	let omni_sell = hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
		asset_in: HDX,
		asset_out: DAI,
		amount: UNITS,
		min_buy_amount: 0,
	});

	let gas_limit = 1000000;

	let gas_price = hydradx_runtime::DynamicEvmFee::min_gas_price();
	//Execute omnipool via EVM
	assert_ok!(EVM::call(
		evm_signed_origin(evm_address()),
		evm_address(),
		DISPATCH_ADDR,
		omni_sell.encode(),
		U256::from(0),
		gas_limit,
		gas_price.0 * 10,
		None,
		Some(U256::from(nonce)),
		[].into(),
	));

	let new_treasury_eth_balance = Tokens::free_balance(WETH, &Treasury::account_id());
	let fee_weth_evm = new_treasury_eth_balance - treasury_eth_balance;

	let fee_in_cents = ETH_USD_SPOT_PRICE * fee_weth_evm as f64 / 1000000000000000000.0;
	round(fee_in_cents)
}

fn round(fee_in_cent: f64) -> f64 {
	let decimal_places = 6;
	let rounder = 10_f64.powi(decimal_places);
	(fee_in_cent * rounder).round() / rounder
}

fn init_omnipool() {
	let native_price = FixedU128::from_inner(1201500000000000);
	let stable_price = FixedU128::from_inner(45_000_000_000);

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		HDX,
		native_price,
		Permill::from_percent(10),
		hydradx_runtime::Omnipool::protocol_account(),
	));
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DAI,
		stable_price,
		Permill::from_percent(100),
		hydradx_runtime::Omnipool::protocol_account(),
	));
}
