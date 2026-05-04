#![cfg(test)]

use crate::polkadot_test_net::*;
use ethereum_types::{H160, U256};
use frame_support::assert_ok;
use hydradx_runtime::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use hydradx_runtime::{Balances, Currencies, Runtime, RuntimeOrigin, Tokens};
use hydradx_traits::evm::Erc20Mapping;
use orml_traits::MultiCurrency;
use pallet_synthetic_logs::{h160_to_h256, Pending as SyntheticLogsPending, TRANSFER_TOPIC};
use xcm_emulator::TestExt;

fn buffered_logs() -> Vec<(pallet_synthetic_logs::Bucket, H160, ethereum::Log)> {
	SyntheticLogsPending::<Runtime>::get()
}

fn alice_h160() -> H160 {
	hydradx_runtime::EVMAccounts::evm_address(&AccountId::from(ALICE))
}

fn bob_h160() -> H160 {
	hydradx_runtime::EVMAccounts::evm_address(&AccountId::from(BOB))
}

#[test]
fn currencies_transfer_routes_native_via_balances() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			BOB.into(),
			HDX,
			UNITS,
		));

		expect_hydra_events(vec![pallet_balances::Event::Transfer {
			from: ALICE.into(),
			to: BOB.into(),
			amount: UNITS,
		}
		.into()]);
	});
}

#[test]
fn currencies_transfer_routes_non_native_via_orml_tokens() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			BOB.into(),
			DAI,
			UNITS,
		));

		expect_hydra_events(vec![orml_tokens::Event::Transfer {
			currency_id: DAI,
			from: ALICE.into(),
			to: BOB.into(),
			amount: UNITS,
		}
		.into()]);
	});
}

#[test]
fn orml_tokens_post_transfer_buffers_synth_log() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			BOB.into(),
			DAI,
			UNITS,
		));

		let logs = buffered_logs();
		let asset_addr = HydraErc20Mapping::asset_address(DAI);

		let entry = logs
			.iter()
			.find(|(_, emitter, log)| *emitter == asset_addr && log.address == asset_addr)
			.expect("synth log for DAI transfer");

		let (_, _, log) = entry;
		assert_eq!(log.topics[0], TRANSFER_TOPIC);
		assert_eq!(log.topics[1], h160_to_h256(alice_h160()));
		assert_eq!(log.topics[2], h160_to_h256(bob_h160()));

		let data = U256::from(UNITS).to_big_endian();
		assert_eq!(log.data.as_slice(), &data[..]);
	});
}

#[test]
fn orml_tokens_post_deposit_buffers_mint_log() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			BOB.into(),
			DAI,
			UNITS as i128,
		));

		let logs = buffered_logs();
		let asset_addr = HydraErc20Mapping::asset_address(DAI);
		let entry = logs
			.iter()
			.find(|(_, emitter, log)| {
				*emitter == asset_addr
					&& log.topics.first() == Some(&TRANSFER_TOPIC)
					&& log.topics[1] == h160_to_h256(H160::zero())
			})
			.expect("synth log for DAI mint");

		let (_, _, log) = entry;
		assert_eq!(log.topics[1], h160_to_h256(H160::zero()));
		assert_eq!(log.topics[2], h160_to_h256(bob_h160()));
	});
}

#[test]
fn orml_tokens_on_slash_buffers_burn_log() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let amount = UNITS;
		<Tokens as MultiCurrency<AccountId>>::deposit(DAI, &ALICE.into(), amount).unwrap();
		SyntheticLogsPending::<Runtime>::kill();

		<Tokens as orml_traits::currency::MultiCurrency<AccountId>>::slash(DAI, &ALICE.into(), amount);

		let logs = buffered_logs();
		let asset_addr = HydraErc20Mapping::asset_address(DAI);
		let entry = logs
			.iter()
			.find(|(_, emitter, log)| *emitter == asset_addr && log.topics[2] == h160_to_h256(H160::zero()))
			.expect("synth log for DAI slash");

		let (_, _, log) = entry;
		assert_eq!(log.topics[0], TRANSFER_TOPIC);
		assert_eq!(log.topics[1], h160_to_h256(alice_h160()));
		assert_eq!(log.topics[2], h160_to_h256(H160::zero()));
	});
}

#[test]
fn orml_tokens_zero_amount_transfer_does_not_buffer() {
	TestNet::reset();
	Hydra::execute_with(|| {
		SyntheticLogsPending::<Runtime>::kill();
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			BOB.into(),
			DAI,
			0,
		));

		let logs = buffered_logs();
		let asset_addr = HydraErc20Mapping::asset_address(DAI);
		assert!(
			logs.iter().all(|(_, emitter, _)| *emitter != asset_addr),
			"zero-amount transfer must not buffer a log"
		);
	});
}

#[test]
fn balances_transfer_buffers_synth_log() {
	TestNet::reset();
	Hydra::execute_with(|| {
		SyntheticLogsPending::<Runtime>::kill();
		assert_ok!(Balances::transfer_keep_alive(
			RuntimeOrigin::signed(ALICE.into()),
			BOB.into(),
			UNITS,
		));

		let logs = buffered_logs();
		let asset_addr = HydraErc20Mapping::asset_address(HDX);
		let entry = logs
			.iter()
			.find(|(_, emitter, _)| *emitter == asset_addr)
			.expect("synth log for HDX balances transfer");

		let (_, _, log) = entry;
		assert_eq!(log.topics[0], TRANSFER_TOPIC);
		assert_eq!(log.topics[1], h160_to_h256(alice_h160()));
		assert_eq!(log.topics[2], h160_to_h256(bob_h160()));
	});
}

#[test]
fn balances_force_transfer_buffers_synth_log() {
	TestNet::reset();
	Hydra::execute_with(|| {
		SyntheticLogsPending::<Runtime>::kill();
		assert_ok!(Balances::force_transfer(
			RuntimeOrigin::root(),
			ALICE.into(),
			BOB.into(),
			UNITS,
		));

		let asset_addr = HydraErc20Mapping::asset_address(HDX);
		let entry = buffered_logs()
			.into_iter()
			.find(|(_, emitter, _)| *emitter == asset_addr)
			.expect("synth log for HDX force_transfer");

		let (_, _, log) = entry;
		assert_eq!(log.topics[1], h160_to_h256(alice_h160()));
		assert_eq!(log.topics[2], h160_to_h256(bob_h160()));
	});
}

#[test]
fn currencies_transfer_native_buffers_synth_log() {
	TestNet::reset();
	Hydra::execute_with(|| {
		SyntheticLogsPending::<Runtime>::kill();
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			BOB.into(),
			HDX,
			UNITS,
		));

		let asset_addr = HydraErc20Mapping::asset_address(HDX);
		assert!(
			buffered_logs().iter().any(|(_, emitter, _)| *emitter == asset_addr),
			"Currencies::transfer of native HDX must buffer a synth log"
		);
	});
}

#[test]
fn balances_dust_loss_buffers_burn_log() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let hdx_ed = <Runtime as pallet_balances::Config>::ExistentialDeposit::get();
		assert!(hdx_ed > 0);

		assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), ALICE.into(), hdx_ed,));
		SyntheticLogsPending::<Runtime>::kill();
		assert_ok!(Balances::transfer_allow_death(
			RuntimeOrigin::signed(ALICE.into()),
			BOB.into(),
			hdx_ed - 1,
		));

		let asset_addr = HydraErc20Mapping::asset_address(HDX);
		let burn_to_zero = buffered_logs()
			.into_iter()
			.find(|(_, emitter, log)| *emitter == asset_addr && log.topics[2] == h160_to_h256(H160::zero()));
		assert!(
			burn_to_zero.is_some(),
			"dust loss must buffer Transfer(from, 0x0, amount)"
		);
	});
}

#[test]
fn currencies_withdraw_orml_buffers_burn_log() {
	TestNet::reset();
	Hydra::execute_with(|| {
		<Tokens as MultiCurrency<AccountId>>::deposit(DAI, &ALICE.into(), UNITS).unwrap();
		SyntheticLogsPending::<Runtime>::kill();
		<Tokens as orml_traits::MultiCurrency<AccountId>>::withdraw(
			DAI,
			&ALICE.into(),
			UNITS,
			frame_support::traits::ExistenceRequirement::AllowDeath,
		)
		.unwrap();

		let asset_addr = HydraErc20Mapping::asset_address(DAI);
		let burn = buffered_logs()
			.into_iter()
			.find(|(_, emitter, log)| *emitter == asset_addr && log.topics[2] == h160_to_h256(H160::zero()));
		assert!(
			burn.is_some(),
			"Currencies::withdraw must buffer Transfer(from, 0x0, amount)"
		);
	});
}
