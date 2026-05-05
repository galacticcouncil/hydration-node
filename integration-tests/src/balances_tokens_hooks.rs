#![cfg(test)]

use crate::polkadot_test_net::*;
use ethereum_types::{H160, U256};
use frame_support::assert_ok;
use hydradx_runtime::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use hydradx_runtime::{Balances, Currencies, Ethereum, Runtime, RuntimeOrigin, SyntheticLogs, Tokens};
use hydradx_traits::evm::Erc20Mapping;
use orml_traits::MultiCurrency;
use pallet_synthetic_logs::{h160_to_h256, reserved_address_of, Pending as SyntheticLogsPending, TRANSFER_TOPIC};
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

// dispatcher precompile → substrate transfer/swap → must surface via eth_getLogs.

#[test]
fn dispatcher_precompile_currencies_transfer_token_emits_transfer_log() {
	use codec::Encode;
	use hydradx_runtime::evm::precompiles::DISPATCH_ADDR;
	use hydradx_runtime::evm::Executor;
	use hydradx_runtime::RuntimeCall;
	use hydradx_traits::evm::{CallContext, EVM as EVMTrait};

	TestNet::reset();
	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::EVMAccounts::bind_evm_address(RuntimeOrigin::signed(
			ALICE.into()
		)));
		let alice_evm = hydradx_runtime::EVMAccounts::evm_address(&AccountId::from(ALICE));

		let bob_acc: AccountId = BOB.into();
		let bob_dai_before = <Tokens as MultiCurrency<AccountId>>::free_balance(DAI, &bob_acc);

		let amount: u128 = UNITS;
		let inner = RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: BOB.into(),
			currency_id: DAI,
			amount,
		});

		SyntheticLogsPending::<Runtime>::kill();
		frame_system::Pallet::<Runtime>::reset_events();

		let context = CallContext {
			contract: DISPATCH_ADDR,
			sender: alice_evm,
			origin: alice_evm,
		};
		let result = Executor::<Runtime>::call(context, inner.encode(), U256::zero(), 1_000_000);
		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Succeed(_)),
			"dispatcher precompile call must succeed, got {:?}",
			result.exit_reason
		);

		let bob_dai_after = <Tokens as MultiCurrency<AccountId>>::free_balance(DAI, &bob_acc);
		assert_eq!(
			bob_dai_after,
			bob_dai_before + amount,
			"the substrate Currencies::transfer must have happened"
		);

		let dai_addr = HydraErc20Mapping::asset_address(DAI);
		let from_h256 = h160_to_h256(alice_h160());
		let to_h256 = h160_to_h256(bob_h160());

		let found = buffered_logs().into_iter().any(|(_, emitter, log)| {
			emitter == dai_addr
				&& log.address == dai_addr
				&& log.topics.first() == Some(&TRANSFER_TOPIC)
				&& log.topics.get(1) == Some(&from_h256)
				&& log.topics.get(2) == Some(&to_h256)
		});

		assert!(
			found,
			"BLINDSPOT: dispatcher precompile dispatched Currencies::transfer(DAI) inside an \
			 EVM frame; the substrate transfer happened, but no Transfer log made it into \
			 pallet_synthetic_logs::Pending (and would be invisible to eth_getLogs). The \
			 substrate hooks skip on is_in_evm(); we need them to either push to the current \
			 EVM frame's logs (so they end up in info.logs / capture_logs) or to synthetic-logs."
		);
	});
}

#[test]
fn dispatcher_precompile_omnipool_sell_emits_swap_log() {
	use codec::Encode;
	use hydradx_runtime::evm::precompiles::DISPATCH_ADDR;
	use hydradx_runtime::evm::Executor;
	use hydradx_runtime::RuntimeCall;
	use hydradx_traits::evm::{CallContext, EVM as EVMTrait};
	use pallet_synthetic_logs::SWAP_TOPIC;

	TestNet::reset();
	Hydra::execute_with(|| {
		crate::evm::init_omnipool_with_oracle_for_block_10();

		assert_ok!(hydradx_runtime::EVMAccounts::bind_evm_address(RuntimeOrigin::signed(
			ALICE.into()
		)));
		let alice_evm = hydradx_runtime::EVMAccounts::evm_address(&AccountId::from(ALICE));
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			DOT,
			(10u128.pow(10) * 100) as i128,
		));

		let amount: u128 = 10_000_000_000;
		let inner = RuntimeCall::Omnipool(pallet_omnipool::Call::sell {
			asset_in: DOT,
			asset_out: HDX,
			amount,
			min_buy_amount: 0,
		});

		SyntheticLogsPending::<Runtime>::kill();
		frame_system::Pallet::<Runtime>::reset_events();

		let context = CallContext {
			contract: DISPATCH_ADDR,
			sender: alice_evm,
			origin: alice_evm,
		};
		let result = Executor::<Runtime>::call(context, inner.encode(), U256::zero(), 2_000_000);
		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Succeed(_)),
			"dispatcher precompile call must succeed, got {:?}",
			result.exit_reason
		);

		let saw_swapped3 = frame_system::Pallet::<Runtime>::events().into_iter().any(|r| {
			matches!(
				&r.event,
				hydradx_runtime::RuntimeEvent::Broadcast(pallet_broadcast::Event::Swapped3 { .. })
			)
		});
		assert!(
			saw_swapped3,
			"Omnipool::sell via dispatcher must produce a Swapped3 substrate event"
		);

		let found = buffered_logs()
			.into_iter()
			.any(|(_, _, log)| log.topics.first() == Some(&SWAP_TOPIC));

		assert!(
			found,
			"BLINDSPOT: dispatcher precompile dispatched Omnipool::sell inside an EVM frame; \
			 Swapped3 fired on the substrate side but OnTrade::on_trade silently skips on \
			 is_in_evm() and the dispatcher does not inline-emit. No uniswap-v2 Swap log is \
			 visible to eth_getLogs. OnTrade should either push into the current EVM frame's \
			 logs or to synthetic-logs when triggered from inside an EVM frame."
		);
	});
}

/// intra-batch order: a dispatcher batch of N transfers produces logs in
/// dispatch order (drained inline at the precompile's call site).
#[test]
fn dispatcher_precompile_batch_preserves_substrate_transfer_order() {
	use codec::Encode;
	use hydradx_runtime::evm::precompiles::DISPATCH_ADDR;
	use hydradx_runtime::evm::Executor;
	use hydradx_runtime::RuntimeCall;
	use hydradx_traits::evm::{CallContext, EVM as EVMTrait};

	TestNet::reset();
	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::EVMAccounts::bind_evm_address(RuntimeOrigin::signed(
			ALICE.into()
		)));
		let alice_evm = hydradx_runtime::EVMAccounts::evm_address(&AccountId::from(ALICE));

		// dispatch a batch of three transfers in a deterministic asset order
		// covers both hook paths: HDX = pallet-balances RuntimeHooks; DAI/DOT = orml-tokens MutationHooks.
		let leg_assets: [AssetId; 3] = [HDX, DAI, DOT];
		let amount: u128 = UNITS;
		let calls: Vec<RuntimeCall> = leg_assets
			.iter()
			.map(|asset| {
				RuntimeCall::Currencies(pallet_currencies::Call::transfer {
					dest: BOB.into(),
					currency_id: *asset,
					amount,
				})
			})
			.collect();
		let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all { calls });

		SyntheticLogsPending::<Runtime>::kill();
		frame_system::Pallet::<Runtime>::reset_events();

		let context = CallContext {
			contract: DISPATCH_ADDR,
			sender: alice_evm,
			origin: alice_evm,
		};
		let result = Executor::<Runtime>::call(context, batch.encode(), U256::zero(), 5_000_000);
		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Succeed(_)),
			"dispatcher batch must succeed, got {:?}",
			result.exit_reason
		);

		let alice_h = alice_h160();
		let bob_h = bob_h160();

		// extract just the Transfer logs that came from one of our three asset addresses,
		// in buffer (== insertion / dispatch) order
		let observed: Vec<AssetId> = buffered_logs()
			.into_iter()
			.filter_map(|(_, _, log)| {
				if log.topics.first() != Some(&TRANSFER_TOPIC) {
					return None;
				}
				if log.topics.get(1) != Some(&h160_to_h256(alice_h)) {
					return None;
				}
				if log.topics.get(2) != Some(&h160_to_h256(bob_h)) {
					return None;
				}
				leg_assets
					.iter()
					.copied()
					.find(|a| HydraErc20Mapping::asset_address(*a) == log.address)
			})
			.collect();

		assert_eq!(
			observed,
			leg_assets.to_vec(),
			"Transfer logs from a batched dispatcher call must appear in dispatch order, \
			 not bunched at the end. Expected {leg_assets:?}, got {observed:?}",
		);
	});
}

/// inline contract logs interleaved with a precompile-triggered substrate transfer
/// must produce `[Marker(0), Transfer, Marker(1)]` in log_index order.
#[test]
fn evm_inline_logs_around_precompile_call_preserve_log_index_order() {
	use ethereum_types::H256;
	use hex_literal::hex;
	use hydradx_runtime::evm::Executor;
	use hydradx_traits::evm::{CallContext, EVM as EVMTrait};

	// keccak256("Marker(uint256)") — see node_modules ethers computation.
	const MARKER_TOPIC: H256 = H256(hex!("83264c98256454386201e4c55918ea57058c5c0052e60bd0b0f9a8fd2f3c1b24"));
	// first 4 bytes of keccak256("exercise(address,address,uint256)")
	const EXERCISE_SELECTOR: [u8; 4] = hex!("430c9304");

	TestNet::reset();
	Hydra::execute_with(|| {
		// deploy the probe contract using ALICE as deployer
		let alice_evm = hydradx_runtime::EVMAccounts::evm_address(&AccountId::from(ALICE));
		let probe = crate::utils::contracts::deploy_contract("LogOrderProbe", alice_evm);

		// fund the probe with DAI so it can transfer to BOB
		let probe_acc = hydradx_runtime::EVMAccounts::truncated_account_id(probe);
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			probe_acc,
			DAI,
			(UNITS * 10) as i128,
		));

		// build calldata: exercise(token=DAI_addr, to=BOB_evm, amount=UNITS)
		let dai_addr = HydraErc20Mapping::asset_address(DAI);
		let bob_evm = bob_h160();
		let amount = UNITS;
		let mut calldata = EXERCISE_SELECTOR.to_vec();
		calldata.extend_from_slice(&[0u8; 12]);
		calldata.extend_from_slice(dai_addr.as_bytes());
		calldata.extend_from_slice(&[0u8; 12]);
		calldata.extend_from_slice(bob_evm.as_bytes());
		calldata.extend_from_slice(&U256::from(amount).to_big_endian());

		SyntheticLogsPending::<Runtime>::kill();
		frame_system::Pallet::<Runtime>::reset_events();

		let context = CallContext {
			contract: probe,
			sender: alice_evm,
			origin: alice_evm,
		};
		let result = Executor::<Runtime>::call(context, calldata, U256::zero(), 5_000_000);
		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Succeed(_)),
			"probe.exercise must succeed, got {:?}",
			result.exit_reason
		);

		// extract the 3 relevant logs in buffered order (= info.logs order = log_index order)
		// classify each: 0=Marker(0), 1=Transfer, 2=Marker(1)
		let from_h256 = h160_to_h256(probe);
		let to_h256 = h160_to_h256(bob_evm);

		let kinds: Vec<&'static str> = buffered_logs()
			.into_iter()
			.filter_map(|(_, _, log)| {
				if log.topics.first() == Some(&MARKER_TOPIC) && log.address == probe {
					// extract the uint256 idx from data
					let idx = U256::from_big_endian(&log.data);
					if idx == U256::zero() {
						Some("Marker0")
					} else if idx == U256::one() {
						Some("Marker1")
					} else {
						None
					}
				} else if log.topics.first() == Some(&TRANSFER_TOPIC)
					&& log.address == dai_addr
					&& log.topics.get(1) == Some(&from_h256)
					&& log.topics.get(2) == Some(&to_h256)
				{
					Some("Transfer")
				} else {
					None
				}
			})
			.collect();

		assert_eq!(
			kinds,
			vec!["Marker0", "Transfer", "Marker1"],
			"log_index ordering must match EVM execution order: \
			 inline Marker(0), then precompile-emitted Transfer (drained at multicurrency precompile call site), \
			 then inline Marker(1). Got: {kinds:?}",
		);
	});
}

// reserve/unreserve mirror as `Transfer(owner, reserved_address_of(owner))`.

#[test]
fn orml_tokens_reserve_buffers_transfer_log_to_reserved_sentinel() {
	use orml_traits::MultiReservableCurrency;

	TestNet::reset();
	Hydra::execute_with(|| {
		SyntheticLogsPending::<Runtime>::kill();
		let amount = UNITS;
		assert_ok!(<Tokens as MultiReservableCurrency<AccountId>>::reserve(
			DAI,
			&ALICE.into(),
			amount
		));

		let asset_addr = HydraErc20Mapping::asset_address(DAI);
		let alice_h = alice_h160();
		let reserved = reserved_address_of(alice_h);

		let entry = buffered_logs().into_iter().find(|(_, emitter, log)| {
			*emitter == asset_addr
				&& log.topics.first() == Some(&TRANSFER_TOPIC)
				&& log.topics.get(1) == Some(&h160_to_h256(alice_h))
				&& log.topics.get(2) == Some(&h160_to_h256(reserved))
		});
		assert!(
			entry.is_some(),
			"orml_tokens::reserve must buffer Transfer(owner, reserved_sentinel, amount); \
			 reserved_sentinel = {reserved:?} for alice = {alice_h:?}",
		);
	});
}

#[test]
fn orml_tokens_unreserve_buffers_transfer_log_from_reserved_sentinel() {
	use orml_traits::MultiReservableCurrency;

	TestNet::reset();
	Hydra::execute_with(|| {
		let amount = UNITS;
		assert_ok!(<Tokens as MultiReservableCurrency<AccountId>>::reserve(
			DAI,
			&ALICE.into(),
			amount
		));
		SyntheticLogsPending::<Runtime>::kill();

		let unreserved = <Tokens as MultiReservableCurrency<AccountId>>::unreserve(DAI, &ALICE.into(), amount);
		assert_eq!(unreserved, 0, "all of `amount` must be unreserved");

		let asset_addr = HydraErc20Mapping::asset_address(DAI);
		let alice_h = alice_h160();
		let reserved = reserved_address_of(alice_h);

		let entry = buffered_logs().into_iter().find(|(_, emitter, log)| {
			*emitter == asset_addr
				&& log.topics.first() == Some(&TRANSFER_TOPIC)
				&& log.topics.get(1) == Some(&h160_to_h256(reserved))
				&& log.topics.get(2) == Some(&h160_to_h256(alice_h))
		});
		assert!(
			entry.is_some(),
			"orml_tokens::unreserve must buffer Transfer(reserved_sentinel, owner, amount)",
		);
	});
}

#[test]
fn balances_reserve_buffers_transfer_log_to_reserved_sentinel() {
	use frame_support::traits::ReservableCurrency;

	TestNet::reset();
	Hydra::execute_with(|| {
		SyntheticLogsPending::<Runtime>::kill();
		let amount = UNITS;
		assert_ok!(<Balances as ReservableCurrency<AccountId>>::reserve(
			&ALICE.into(),
			amount
		));

		let hdx_addr = HydraErc20Mapping::asset_address(HDX);
		let alice_h = alice_h160();
		let reserved = reserved_address_of(alice_h);

		let entry = buffered_logs().into_iter().find(|(_, emitter, log)| {
			*emitter == hdx_addr
				&& log.topics.first() == Some(&TRANSFER_TOPIC)
				&& log.topics.get(1) == Some(&h160_to_h256(alice_h))
				&& log.topics.get(2) == Some(&h160_to_h256(reserved))
		});
		assert!(
			entry.is_some(),
			"pallet_balances::reserve must buffer Transfer(owner, reserved_sentinel, amount) for HDX",
		);
	});
}

#[test]
fn balances_unreserve_buffers_transfer_log_from_reserved_sentinel() {
	use frame_support::traits::ReservableCurrency;

	TestNet::reset();
	Hydra::execute_with(|| {
		let amount = UNITS;
		assert_ok!(<Balances as ReservableCurrency<AccountId>>::reserve(
			&ALICE.into(),
			amount
		));
		SyntheticLogsPending::<Runtime>::kill();

		let unreserved = <Balances as ReservableCurrency<AccountId>>::unreserve(&ALICE.into(), amount);
		assert_eq!(unreserved, 0, "all of `amount` must be unreserved");

		let hdx_addr = HydraErc20Mapping::asset_address(HDX);
		let alice_h = alice_h160();
		let reserved = reserved_address_of(alice_h);

		let entry = buffered_logs().into_iter().find(|(_, emitter, log)| {
			*emitter == hdx_addr
				&& log.topics.first() == Some(&TRANSFER_TOPIC)
				&& log.topics.get(1) == Some(&h160_to_h256(reserved))
				&& log.topics.get(2) == Some(&h160_to_h256(alice_h))
		});
		assert!(
			entry.is_some(),
			"pallet_balances::unreserve must buffer Transfer(reserved_sentinel, owner, amount) for HDX",
		);
	});
}

// repatriate_reserved: A.reserved → B.free or B.reserved.

#[test]
fn orml_tokens_repatriate_to_free_buffers_transfer_log_to_beneficiary() {
	use orml_traits::{BalanceStatus, MultiReservableCurrency};

	TestNet::reset();
	Hydra::execute_with(|| {
		let amount = UNITS;
		assert_ok!(<Tokens as MultiReservableCurrency<AccountId>>::reserve(
			DAI,
			&ALICE.into(),
			amount
		));
		SyntheticLogsPending::<Runtime>::kill();

		assert_ok!(<Tokens as MultiReservableCurrency<AccountId>>::repatriate_reserved(
			DAI,
			&ALICE.into(),
			&BOB.into(),
			amount,
			BalanceStatus::Free,
		));

		let dai_addr = HydraErc20Mapping::asset_address(DAI);
		let alice_reserved = reserved_address_of(alice_h160());
		let bob_h = bob_h160();

		let entry = buffered_logs().into_iter().find(|(_, emitter, log)| {
			*emitter == dai_addr
				&& log.topics.first() == Some(&TRANSFER_TOPIC)
				&& log.topics.get(1) == Some(&h160_to_h256(alice_reserved))
				&& log.topics.get(2) == Some(&h160_to_h256(bob_h))
		});
		assert!(
			entry.is_some(),
			"orml_tokens::repatriate_reserved (status=Free) must buffer \
			 Transfer(reserved_address_of(slashed), beneficiary, amount)",
		);
	});
}

#[test]
fn orml_tokens_repatriate_to_reserved_buffers_transfer_log_between_reserved_buckets() {
	use orml_traits::{BalanceStatus, MultiReservableCurrency};

	TestNet::reset();
	Hydra::execute_with(|| {
		let amount = UNITS;
		assert_ok!(<Tokens as MultiReservableCurrency<AccountId>>::reserve(
			DAI,
			&ALICE.into(),
			amount
		));
		SyntheticLogsPending::<Runtime>::kill();

		assert_ok!(<Tokens as MultiReservableCurrency<AccountId>>::repatriate_reserved(
			DAI,
			&ALICE.into(),
			&BOB.into(),
			amount,
			BalanceStatus::Reserved,
		));

		let dai_addr = HydraErc20Mapping::asset_address(DAI);
		let alice_reserved = reserved_address_of(alice_h160());
		let bob_reserved = reserved_address_of(bob_h160());

		let entry = buffered_logs().into_iter().find(|(_, emitter, log)| {
			*emitter == dai_addr
				&& log.topics.first() == Some(&TRANSFER_TOPIC)
				&& log.topics.get(1) == Some(&h160_to_h256(alice_reserved))
				&& log.topics.get(2) == Some(&h160_to_h256(bob_reserved))
		});
		assert!(
			entry.is_some(),
			"orml_tokens::repatriate_reserved (status=Reserved) must buffer \
			 Transfer(reserved_address_of(slashed), reserved_address_of(beneficiary), amount)",
		);
	});
}

#[test]
fn orml_tokens_slash_reserved_buffers_burn_log_from_reserved_sentinel() {
	use orml_traits::MultiReservableCurrency;

	TestNet::reset();
	Hydra::execute_with(|| {
		let amount = UNITS;
		assert_ok!(<Tokens as MultiReservableCurrency<AccountId>>::reserve(DAI, &ALICE.into(), amount));
		SyntheticLogsPending::<Runtime>::kill();

		let remaining = <Tokens as MultiReservableCurrency<AccountId>>::slash_reserved(DAI, &ALICE.into(), amount);
		assert_eq!(remaining, 0);

		let dai_addr = HydraErc20Mapping::asset_address(DAI);
		let reserved = reserved_address_of(alice_h160());
		let entry = buffered_logs().into_iter().find(|(_, emitter, log)| {
			*emitter == dai_addr
				&& log.topics.first() == Some(&TRANSFER_TOPIC)
				&& log.topics.get(1) == Some(&h160_to_h256(reserved))
				&& log.topics.get(2) == Some(&h160_to_h256(H160::zero()))
		});
		assert!(
			entry.is_some(),
			"orml_tokens::slash_reserved must buffer Transfer(reserved_sentinel, 0x0, amount)",
		);
	});
}

#[test]
fn balances_slash_reserved_buffers_burn_log_from_reserved_sentinel() {
	use frame_support::traits::ReservableCurrency;

	TestNet::reset();
	Hydra::execute_with(|| {
		let amount = UNITS;
		assert_ok!(<Balances as ReservableCurrency<AccountId>>::reserve(&ALICE.into(), amount));
		SyntheticLogsPending::<Runtime>::kill();

		let (_imbalance, remaining) = <Balances as ReservableCurrency<AccountId>>::slash_reserved(&ALICE.into(), amount);
		assert_eq!(remaining, 0);

		let hdx_addr = HydraErc20Mapping::asset_address(HDX);
		let reserved = reserved_address_of(alice_h160());
		let entry = buffered_logs().into_iter().find(|(_, emitter, log)| {
			*emitter == hdx_addr
				&& log.topics.first() == Some(&TRANSFER_TOPIC)
				&& log.topics.get(1) == Some(&h160_to_h256(reserved))
				&& log.topics.get(2) == Some(&h160_to_h256(H160::zero()))
		});
		assert!(
			entry.is_some(),
			"pallet_balances::slash_reserved must buffer Transfer(reserved_sentinel, 0x0, amount) for HDX",
		);
	});
}

#[test]
fn balances_currency_withdraw_buffers_burn_log() {
	// covers the native HDX fee path which routes through
	// `BasicCurrencyAdapter::withdraw` → `pallet_balances::Currency::withdraw`
	use frame_support::traits::{Currency, ExistenceRequirement, WithdrawReasons};

	TestNet::reset();
	Hydra::execute_with(|| {
		SyntheticLogsPending::<Runtime>::kill();
		let _imbalance = <Balances as Currency<AccountId>>::withdraw(
			&ALICE.into(),
			UNITS,
			WithdrawReasons::TRANSACTION_PAYMENT,
			ExistenceRequirement::AllowDeath,
		)
		.expect("alice has enough HDX");

		let hdx_addr = HydraErc20Mapping::asset_address(HDX);
		let entry = buffered_logs().into_iter().find(|(_, emitter, log)| {
			*emitter == hdx_addr
				&& log.topics.first() == Some(&TRANSFER_TOPIC)
				&& log.topics.get(1) == Some(&h160_to_h256(alice_h160()))
				&& log.topics.get(2) == Some(&h160_to_h256(H160::zero()))
		});
		assert!(
			entry.is_some(),
			"pallet_balances::Currency::withdraw must buffer Transfer(payer, 0x0, amount); \
			 this is the native HDX fee burn path",
		);
	});
}

#[test]
fn balances_currency_deposit_creating_buffers_mint_log() {
	// covers the deposit half of the fee path
	// (treasury credit, etc.): `Currency::deposit_creating` was silent before.
	use frame_support::traits::Currency;

	TestNet::reset();
	Hydra::execute_with(|| {
		SyntheticLogsPending::<Runtime>::kill();
		let _imbalance = <Balances as Currency<AccountId>>::deposit_creating(&BOB.into(), UNITS);

		let hdx_addr = HydraErc20Mapping::asset_address(HDX);
		let entry = buffered_logs().into_iter().find(|(_, emitter, log)| {
			*emitter == hdx_addr
				&& log.topics.first() == Some(&TRANSFER_TOPIC)
				&& log.topics.get(1) == Some(&h160_to_h256(H160::zero()))
				&& log.topics.get(2) == Some(&h160_to_h256(bob_h160()))
		});
		assert!(
			entry.is_some(),
			"pallet_balances::Currency::deposit_creating must buffer Transfer(0x0, recipient, amount)",
		);
	});
}

/// SyntheticLogs::on_finalize must run before Ethereum::on_finalize so the
/// synth tx lands in pallet_ethereum::Pending → CurrentBlock/Statuses/Receipts.
#[test]
fn on_finalize_drains_buffer_into_pallet_ethereum_pending_before_ethereum_seal() {
	use frame_support::traits::OnFinalize;

	TestNet::reset();
	Hydra::execute_with(|| {
		let n = frame_system::Pallet::<Runtime>::block_number();
		SyntheticLogsPending::<Runtime>::kill();

		// Push a Transfer log directly into the buffer (skip the extrinsic to
		// avoid xcm-emulator's auto-finalize coupling).
		let dai_addr = HydraErc20Mapping::asset_address(DAI);
		let mut data = Vec::with_capacity(32);
		data.extend_from_slice(&U256::from(UNITS).to_big_endian());
		let log = ethereum::Log {
			address: dai_addr,
			topics: vec![TRANSFER_TOPIC, h160_to_h256(alice_h160()), h160_to_h256(bob_h160())],
			data,
		};
		pallet_synthetic_logs::Pallet::<Runtime>::push(dai_addr, log);
		assert!(!SyntheticLogsPending::<Runtime>::get().is_empty());

		// Verify the runtime ordering directly: SyntheticLogs::on_finalize must
		// drain the buffer into pallet_ethereum::Pending BEFORE
		// Ethereum::on_finalize rolls Pending into CurrentBlock + statuses.
		// (construct_runtime! order: SyntheticLogs=86 runs before Ethereum=92.)
		SyntheticLogs::on_finalize(n);

		// After SyntheticLogs's flush but BEFORE Ethereum::on_finalize, the
		// synth tx must already be in pallet_ethereum::Pending.
		let pending: Vec<_> = pallet_ethereum::Pending::<Runtime>::iter().collect();
		assert!(
			!pending.is_empty(),
			"flush must write into pallet_ethereum::Pending",
		);

		// Now seal the ethereum block.
		Ethereum::on_finalize(n);

		// Buffer drained.
		assert!(
			SyntheticLogsPending::<Runtime>::get().is_empty(),
			"buffer must be empty after on_finalize flush",
		);

		// pallet_ethereum::on_finalize moves Pending → CurrentBlock + per-tx
		// CurrentTransactionStatuses + CurrentReceipts. After the seal, the
		// synth tx should appear in the block's tx list, statuses, and receipts.
		let block = pallet_ethereum::CurrentBlock::<Runtime>::get().expect("ethereum block sealed");
		let statuses = pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
			.expect("statuses stored after seal");
		let receipts = pallet_ethereum::CurrentReceipts::<Runtime>::get().expect("receipts stored after seal");

		assert_eq!(block.transactions.len(), statuses.len(), "tx count == status count");
		assert_eq!(block.transactions.len(), receipts.len(), "tx count == receipt count");

		let dai_addr = HydraErc20Mapping::asset_address(DAI);
		let synth_status = statuses.iter().find(|s| {
			s.from == pallet_synthetic_logs::SENTINEL_ADDRESS
				&& s.logs.iter().any(|log| {
					log.address == dai_addr
						&& log.topics.first() == Some(&TRANSFER_TOPIC)
						&& log.topics.get(1) == Some(&h160_to_h256(alice_h160()))
						&& log.topics.get(2) == Some(&h160_to_h256(bob_h160()))
				})
		});
		assert!(
			synth_status.is_some(),
			"synthetic tx with the DAI Transfer log must be in CurrentTransactionStatuses after seal; \
			 got {} statuses",
			statuses.len(),
		);
		let synth_status = synth_status.unwrap();

		// Receipt at the same index must have matching logs (logs_bloom too).
		let receipt_logs = match &receipts[synth_status.transaction_index as usize] {
			pallet_ethereum::Receipt::EIP1559(d)
			| pallet_ethereum::Receipt::EIP2930(d)
			| pallet_ethereum::Receipt::Legacy(d) => &d.logs,
			_ => panic!("unexpected receipt variant"),
		};
		assert_eq!(
			receipt_logs.len(),
			synth_status.logs.len(),
			"receipt logs match status logs",
		);
	});
}

/// outer EVM frame reverts → substrate state rolls back; runner end-drain
/// must NOT append buffered hook logs (would be ghost logs for non-events).
#[test]
fn evm_frame_revert_drops_buffered_substrate_hook_logs() {
	use codec::Encode;
	use hex_literal::hex;
	use hydradx_runtime::evm::precompiles::DISPATCH_ADDR;
	use hydradx_runtime::evm::Executor;
	use hydradx_runtime::RuntimeCall;
	use hydradx_traits::evm::{CallContext, EVM as EVMTrait};

	// keccak256("try_dispatch_then_revert(address,bytes)")[..4]
	const PROBE_SELECTOR: [u8; 4] = hex!("4351d800");

	TestNet::reset();
	Hydra::execute_with(|| {
		let alice_evm = hydradx_runtime::EVMAccounts::evm_address(&AccountId::from(ALICE));
		let probe = crate::utils::contracts::deploy_contract("RevertingDispatcher", alice_evm);

		// Fund probe with DAI so the inner dispatch would succeed if not reverted.
		let probe_acc = hydradx_runtime::EVMAccounts::truncated_account_id(probe);
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			probe_acc,
			DAI,
			(UNITS * 10) as i128,
		));

		let bob_acc: AccountId = BOB.into();
		let bob_dai_before = <Tokens as MultiCurrency<AccountId>>::free_balance(DAI, &bob_acc);

		let amount: u128 = UNITS;
		let inner = RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: BOB.into(),
			currency_id: DAI,
			amount,
		});
		let inner_encoded = inner.encode();

		// abi-encode try_dispatch_then_revert(address dispatcher, bytes sub_call_data):
		// selector || dispatcher (32 bytes) || offset_to_bytes (=0x40, 32 bytes)
		// || bytes_len (32) || bytes_payload (padded)
		let mut calldata = PROBE_SELECTOR.to_vec();
		calldata.extend_from_slice(&[0u8; 12]);
		calldata.extend_from_slice(DISPATCH_ADDR.as_bytes());
		calldata.extend_from_slice(&U256::from(0x40u64).to_big_endian());
		calldata.extend_from_slice(&U256::from(inner_encoded.len() as u64).to_big_endian());
		calldata.extend_from_slice(&inner_encoded);
		// pad to 32-byte boundary
		let pad = (32 - inner_encoded.len() % 32) % 32;
		calldata.extend(std::iter::repeat_n(0u8, pad));

		SyntheticLogsPending::<Runtime>::kill();

		let context = CallContext {
			contract: probe,
			sender: alice_evm,
			origin: alice_evm,
		};
		let result = Executor::<Runtime>::call(context, calldata, U256::zero(), 5_000_000);
		// The contract reverts intentionally — the outer call result is Revert.
		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)),
			"probe must revert, got {:?}",
			result.exit_reason
		);

		let bob_dai_after = <Tokens as MultiCurrency<AccountId>>::free_balance(DAI, &bob_acc);
		assert_eq!(
			bob_dai_after, bob_dai_before,
			"substrate state must be rolled back: bob's DAI balance must NOT change",
		);

		let dai_addr = HydraErc20Mapping::asset_address(DAI);
		let leaked = buffered_logs()
			.into_iter()
			.any(|(_, _, log)| log.address == dai_addr && log.topics.first() == Some(&TRANSFER_TOPIC));
		assert!(
			!leaked,
			"BLINDSPOT: a DAI Transfer log leaked into the synth tx even though the outer EVM frame reverted; \
			 the WrapRunner end-drain is appending buffered hook logs unconditionally — must gate on Succeed.",
		);
	});
}

#[test]
fn balances_repatriate_to_free_buffers_transfer_log_to_beneficiary() {
	use frame_support::traits::{tokens::BalanceStatus, ReservableCurrency};

	TestNet::reset();
	Hydra::execute_with(|| {
		let amount = UNITS;
		assert_ok!(<Balances as ReservableCurrency<AccountId>>::reserve(
			&ALICE.into(),
			amount
		));
		SyntheticLogsPending::<Runtime>::kill();

		assert_ok!(<Balances as ReservableCurrency<AccountId>>::repatriate_reserved(
			&ALICE.into(),
			&BOB.into(),
			amount,
			BalanceStatus::Free,
		));

		let hdx_addr = HydraErc20Mapping::asset_address(HDX);
		let alice_reserved = reserved_address_of(alice_h160());
		let bob_h = bob_h160();

		let entry = buffered_logs().into_iter().find(|(_, emitter, log)| {
			*emitter == hdx_addr
				&& log.topics.first() == Some(&TRANSFER_TOPIC)
				&& log.topics.get(1) == Some(&h160_to_h256(alice_reserved))
				&& log.topics.get(2) == Some(&h160_to_h256(bob_h))
		});
		assert!(
			entry.is_some(),
			"pallet_balances::repatriate_reserved (status=Free) must buffer \
			 Transfer(reserved_address_of(slashed), beneficiary, amount) for HDX",
		);
	});
}
