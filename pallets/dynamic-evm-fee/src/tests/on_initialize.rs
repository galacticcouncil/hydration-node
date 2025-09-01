use crate::tests::mock::DynamicEvmFee;
use crate::tests::mock::*;
use frame_support::traits::OnInitialize;
use hydra_dx_math::types::Ratio;
use pallet_transaction_payment::Multiplier;
use sp_core::U256;

#[test]
fn should_return_default_base_fee_when_min_multiplier() {
	ExtBuilder::default().build().execute_with(|| {
		DynamicEvmFee::on_initialize(1);

		let new_base_fee = DynamicEvmFee::base_evm_fee();
		assert_eq!(new_base_fee, U256::from(15045000));
	});
}

#[test]
fn should_return_lower_base_fee_when_min_multiplier_on_testnet() {
	ExtBuilder::default().build().execute_with(|| {
		Parameters::set_testnet_flag(true);
		DynamicEvmFee::on_initialize(1);

		let new_base_fee = DynamicEvmFee::base_evm_fee();
		assert_eq!(new_base_fee, U256::from(1504500));
	});
}

#[test]
fn should_increase_evm_fee_with_max_multiplier() {
	ExtBuilder::default().build().execute_with(|| {
		set_multiplier(Multiplier::from_rational(320, 1));

		DynamicEvmFee::on_initialize(1);

		let new_base_fee = DynamicEvmFee::base_evm_fee();
		assert_eq!(new_base_fee, U256::from(14415000000u128));
	});
}

#[test]
fn should_increase_evm_fee_when_hdx_pumping_10percent_against_eth() {
	ExtBuilder::default().build().execute_with(|| {
		set_oracle_price(Ratio::new(
			DEFAULT_ETH_HDX_ORACLE_PRICE.n * 110 / 100,
			DEFAULT_ETH_HDX_ORACLE_PRICE.d,
		));

		DynamicEvmFee::on_initialize(1);

		let new_base_fee = DynamicEvmFee::base_evm_fee();
		assert_eq!(new_base_fee, U256::from(16549499));
	});
}

#[test]
fn should_not_change_when_price_pumps_then_remains_same_in_consquent_block() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		set_oracle_price(Ratio::new(
			DEFAULT_ETH_HDX_ORACLE_PRICE.n * 110 / 100,
			DEFAULT_ETH_HDX_ORACLE_PRICE.d,
		));

		DynamicEvmFee::on_initialize(1);

		let new_base_fee = DynamicEvmFee::base_evm_fee();
		assert_eq!(new_base_fee, U256::from(16549499));

		//Act
		DynamicEvmFee::on_initialize(2);

		//Assert
		let new_base_fee = DynamicEvmFee::base_evm_fee();
		assert_eq!(new_base_fee, U256::from(16549499));
	});
}

#[test]
fn should_increase_evm_fee_when_hdx_pumping_1percent_against_eth() {
	ExtBuilder::default().build().execute_with(|| {
		set_oracle_price(Ratio::new(
			DEFAULT_ETH_HDX_ORACLE_PRICE.n * 101 / 100,
			DEFAULT_ETH_HDX_ORACLE_PRICE.d,
		));
		DynamicEvmFee::on_initialize(1);

		let new_base_fee = DynamicEvmFee::base_evm_fee();
		assert_eq!(new_base_fee, U256::from(15195449));
	});
}

#[test]
fn should_decrease_evm_fee_when_hdx_dumping_10percent_against_eth() {
	ExtBuilder::default().build().execute_with(|| {
		set_oracle_price(Ratio::new(
			DEFAULT_ETH_HDX_ORACLE_PRICE.n * 90 / 100,
			DEFAULT_ETH_HDX_ORACLE_PRICE.d,
		));
		DynamicEvmFee::on_initialize(1);

		let new_base_fee = DynamicEvmFee::base_evm_fee();
		assert_eq!(new_base_fee, U256::from(13540499));
	});
}

#[test]
fn evm_fee_should_go_to_minimum_when_hdx_dumps_a_alot() {
	ExtBuilder::default().build().execute_with(|| {
		set_oracle_price(Ratio::new(
			DEFAULT_ETH_HDX_ORACLE_PRICE.n / 100,
			DEFAULT_ETH_HDX_ORACLE_PRICE.d,
		));
		DynamicEvmFee::on_initialize(1);

		let new_base_fee = DynamicEvmFee::base_evm_fee();
		assert_eq!(new_base_fee, U256::from(1500000));
	});
}

#[test]
fn evm_fee_should_go_to_maximum_when_hdx_pumps_a_lot() {
	ExtBuilder::default().build().execute_with(|| {
		set_oracle_price(Ratio::new(
			DEFAULT_ETH_HDX_ORACLE_PRICE.n * 1000,
			DEFAULT_ETH_HDX_ORACLE_PRICE.d,
		));
		DynamicEvmFee::on_initialize(1);

		let new_base_fee = DynamicEvmFee::base_evm_fee();
		assert_eq!(new_base_fee, U256::from(14415000000u128));
	});
}
