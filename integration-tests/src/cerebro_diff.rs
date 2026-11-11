#![cfg(test)]

// The cerebro guard on storage diffs, against the real runtime. `guarded` runs a call the way the
// block builder runs an extrinsic: in its own overlay transaction, judged from that transaction's
// writes and the state on either side of it, then rolled back.

use crate::polkadot_test_net::*;
use cerebro_core::model::StateDelta;
use cerebro_core::value::Usd6;
use cerebro_core::BlockHash;
use cerebro_hydration::balance::call_output;
use cerebro_hydration::decode::map_records;
use cerebro_hydration::guard::{
	self, candidate_of, is_candidate, read_oracle, read_point, with_state, Accumulators, GuardCtx, InconclusiveReason,
	Judgement, Thresholds, Verdict, POST, PRE,
};
use cerebro_hydration::pricing::Prices;
use cerebro_hydration::state::{self, Change};
use cerebro_hydration::{accounts, Transport};
use fp_rpc::runtime_decl_for_ethereum_runtime_rpc_api::EthereumRuntimeRPCApi;
use frame_support::assert_ok;
use frame_support::storage::with_transaction;
use hydradx_runtime::{Runtime, RuntimeEvent, RuntimeOrigin, System};
use orml_traits::MultiCurrency;
use pretty_assertions::assert_eq;
use sp_core::{H160, H256, U256};
use sp_runtime::{DispatchError, TransactionOutcome};
use sp_state_machine::{Backend, PriorValue};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use xcm_emulator::TestExt;

const SIGNER: [u8; 32] = CHARLIE;

/// The node's defaults: pool leaks hold, signer gain never does.
fn thresholds() -> Thresholds {
	Thresholds {
		min_usd: Usd6(i128::MAX),
		min_pool_leak: Usd6::ZERO,
		min_block_usd: Usd6(i128::MAX),
		min_block_pool_leak: Usd6::ZERO,
		pool_leak_epsilon_bps: 0,
	}
}

/// cerebro's chain reads over the test externalities' current state.
struct ExtTransport<'a>(RefCell<&'a mut sp_io::TestExternalities>);

impl Transport for ExtTransport<'_> {
	fn storage_many(&self, keys: &[Vec<u8>], _at: BlockHash) -> Result<Vec<Option<Vec<u8>>>, String> {
		Ok(self.0.borrow_mut().execute_with(|| {
			keys.iter()
				.map(|k| sp_io::storage::get(k).map(|v| v.to_vec()))
				.collect()
		}))
	}

	fn evm_view(&self, to: [u8; 20], data: &[u8], _at: BlockHash) -> Result<Vec<u8>, String> {
		let info = self.0.borrow_mut().execute_with(|| {
			with_transaction(|| {
				TransactionOutcome::Rollback(Ok::<_, DispatchError>(Runtime::call(
					H160::zero(),
					H160(to),
					data.to_vec(),
					U256::zero(),
					U256::from(10_000_000u64),
					None,
					None,
					None,
					true,
					None,
					None,
				)))
			})
		});
		call_output(info.and_then(|r| r).map_err(|e| format!("{e:?}"))?)
	}

	fn storage_keys(&self, prefix: &[u8], _at: BlockHash) -> Result<Vec<Vec<u8>>, String> {
		Ok(self.0.borrow_mut().execute_with(|| {
			let mut keys = vec![];
			let mut at = prefix.to_vec();
			while let Some(next) = sp_io::storage::next_key(&at).filter(|k| k.starts_with(prefix)) {
				keys.push(next.clone());
				at = next;
			}
			keys
		}))
	}

	fn block_number(&self, _at: BlockHash) -> Result<u32, String> {
		Ok(self.0.borrow_mut().execute_with(System::block_number))
	}
}

fn dump_state() -> BTreeMap<Vec<u8>, Vec<u8>> {
	let mut out = BTreeMap::new();
	let mut at = vec![];
	while let Some(next) = sp_io::storage::next_key(&at) {
		out.insert(
			next.clone(),
			sp_io::storage::get(&next).expect("next_key returned it; qed").to_vec(),
		);
		at = next;
	}
	out
}

type StateDiff = BTreeMap<Vec<u8>, (Option<Vec<u8>>, Option<Vec<u8>>)>;

struct Guarded {
	changes: Vec<Change>,
	appended: Vec<Vec<u8>>,
	/// Every key whose value differs before and after, from two full state scans.
	full_diff: StateDiff,
	delta: StateDelta,
	/// Whether the event-only guard would have judged it.
	event_candidate: bool,
	diff_candidate: bool,
	judgement: Judgement,
}

/// Run `call` as extrinsic 0 of `signer` in its own overlay transaction, judge it, roll it back.
fn guarded(signer: Option<[u8; 32]>, call: impl FnOnce()) -> Guarded {
	LOCAL_EXT_HYDRAPARACHAIN.with(|cell| {
		let mut ext = cell.borrow_mut();
		ext.execute_with(|| {
			System::reset_events();
			System::note_finished_initialize();
		});
		let before = ext.execute_with(dump_state);

		ext.overlay.start_transaction();
		ext.execute_with(|| {
			// The fee payer is the signer the guard judges; these calls are dispatched without fees.
			if let Some(who) = signer {
				System::deposit_event(RuntimeEvent::TransactionPayment(
					pallet_transaction_payment::Event::TransactionFeePaid {
						who: who.into(),
						actual_fee: 0,
						tip: 0,
					},
				));
			}
			call();
		});

		let mut changes = vec![];
		let mut appended = vec![];
		for (key, prior, post) in ext.overlay.current_transaction_top_changes(|_| true) {
			let pre = match prior {
				PriorValue::Overlay(v) => v,
				PriorValue::Backend => ext.backend.storage(&key).expect("in-memory backend; qed"),
				PriorValue::Appended => {
					appended.push(key);
					continue;
				}
			};
			if pre != post {
				changes.push(Change { key, pre, post });
			}
		}
		let after = ext.execute_with(dump_state);
		let full_diff = before
			.keys()
			.chain(after.keys())
			.filter(|k| before.get(*k) != after.get(*k))
			.map(|k| (k.clone(), (before.get(k).cloned(), after.get(k).cloned())))
			.collect();
		let records = ext.execute_with(System::events);

		let t = ExtTransport(RefCell::new(&mut ext));
		let protocol = accounts::fetch(&t, H256(POST)).expect("protocol accounts read");
		let (contracts, atoken_of) = (HashMap::new(), BTreeMap::new());
		let events = candidate_of(map_records(&records), 0, &protocol, &contracts, None, false);
		let event_candidate = is_candidate(&events);
		let decoded: Vec<_> = changes.iter().filter(|c| state::decoded(&c.key)).cloned().collect();
		let delta = state::decode(&decoded);
		let cand = with_state(events, delta.clone(), &protocol, &contracts);
		let cx = GuardCtx {
			protocol: &protocol,
			contracts: &contracts,
			atoken_of: &atoken_of,
			thresholds: thresholds(),
			committed_trades: &[],
		};
		let plan = guard::plan(&t, POST, &cand, &cx);
		let post = read_point(&t, POST, &plan);

		ext.overlay.rollback_transaction().expect("opened above; qed");
		let t = ExtTransport(RefCell::new(&mut ext));
		let pre = read_point(&t, PRE, &plan);
		let oracle = read_oracle(&t, PRE, &plan, &Prices::empty());
		let judgement = guard::judge(&cand, &plan, &pre, &post, &oracle, &cx, &Accumulators::default());

		Guarded {
			changes,
			appended,
			full_diff,
			delta,
			event_candidate,
			diff_candidate: is_candidate(&cand),
			judgement,
		}
	})
}

impl Guarded {
	/// The SDK's per-transaction changes must be exactly the full-scan diff, appended keys aside.
	fn assert_diff_is_complete(&self) {
		let from_sdk: BTreeMap<_, _> = self
			.changes
			.iter()
			.map(|c| (c.key.clone(), (c.pre.clone(), c.post.clone())))
			.collect();
		let mut full = self.full_diff.clone();
		for key in &self.appended {
			full.remove(key);
		}
		assert_eq!(names(&from_sdk), names(&full));
		assert_eq!(from_sdk, full);
	}
}

fn names(diff: &StateDiff) -> Vec<String> {
	diff.keys().map(|k| state::item_name(k)).collect()
}

const USDT: u32 = 10;

/// A stableswap pool of USDT (the asset cerebro values everything in) and four 18-decimal stables,
/// 100k units each. Returns the pool and one non-USDT member.
fn usd_stable_pool() -> (u32, u32) {
	use frame_support::BoundedVec;
	use hydradx_traits::registry::{AssetKind, Create};
	use hydradx_traits::stableswap::AssetAmount;

	assert_ok!(hydradx_runtime::AssetRegistry::register(
		RuntimeOrigin::root(),
		Some(USDT),
		Some(b"USDT".to_vec().try_into().unwrap()),
		pallet_asset_registry::AssetType::Token,
		Some(1u128),
		Some(b"USDT".to_vec().try_into().unwrap()),
		Some(6u8),
		None,
		None,
		true,
	));
	let mut assets = vec![(USDT, 100_000_000_000u128)];
	for idx in 0u32..4 {
		let id = hydradx_runtime::AssetRegistry::register_sufficient_asset(
			None,
			Some(idx.to_ne_bytes().to_vec().try_into().unwrap()),
			AssetKind::Token,
			1u128,
			Some(b"xDUM".to_vec().try_into().unwrap()),
			Some(18u8),
			None,
			None,
		)
		.unwrap();
		assets.push((id, 100_000_000_000_000_000_000_000));
	}
	for (id, amount) in &assets {
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			RuntimeOrigin::root(),
			BOB.into(),
			*id,
			*amount as i128,
		));
	}
	let pool_id = hydradx_runtime::AssetRegistry::register_sufficient_asset(
		None,
		Some(b"pool".to_vec().try_into().unwrap()),
		AssetKind::Token,
		1u128,
		None,
		None,
		None,
		None,
	)
	.unwrap();
	assert_ok!(hydradx_runtime::Stableswap::create_pool(
		RuntimeOrigin::root(),
		pool_id,
		BoundedVec::truncate_from(assets.iter().map(|(id, _)| *id).collect()),
		100,
		sp_runtime::Permill::from_percent(1),
	));
	assert_ok!(hydradx_runtime::Stableswap::add_assets_liquidity(
		RuntimeOrigin::signed(BOB.into()),
		pool_id,
		BoundedVec::truncate_from(assets.iter().map(|(id, a)| AssetAmount::new(*id, *a)).collect()),
		0,
	));
	(pool_id, assets[1].0)
}

fn setup(f: impl FnOnce()) {
	TestNet::reset();
	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			f();
			TransactionOutcome::Commit(Ok::<_, DispatchError>(()))
		});
	});
}

#[test]
fn guard_should_allow_when_stableswap_trade_is_fair() {
	let mut pool = None;
	setup(|| {
		let (pool_id, asset_a) = usd_stable_pool();
		let asset_b = USDT;
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			RuntimeOrigin::root(),
			SIGNER.into(),
			asset_a,
			1_000_000_000_000_000_000i128,
		));
		pool = Some((pool_id, asset_a, asset_b));
	});
	let (pool_id, asset_a, asset_b) = pool.unwrap();

	let g = guarded(Some(SIGNER), || {
		assert_ok!(hydradx_runtime::Stableswap::sell(
			RuntimeOrigin::signed(SIGNER.into()),
			pool_id,
			asset_a,
			asset_b,
			1_000_000_000_000_000_000,
			0,
		));
	});

	g.assert_diff_is_complete();
	assert!(g.diff_candidate);
	assert_eq!(g.delta.unbalanced_assets(), Default::default());
	assert_eq!(g.delta.stable_pools, Default::default());
	assert_eq!(g.judgement.verdict, Verdict::Allow(guard::AllowReason::Clean));
}

#[test]
fn guard_should_allow_when_liquidity_is_added_and_removed() {
	let mut pool = None;
	setup(|| {
		let (pool_id, asset_a) = usd_stable_pool();
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			RuntimeOrigin::root(),
			SIGNER.into(),
			asset_a,
			1_000_000_000_000_000_000i128,
		));
		pool = Some((pool_id, asset_a));
	});
	let (pool_id, asset_a) = pool.unwrap();

	let g = guarded(Some(SIGNER), || {
		assert_ok!(hydradx_runtime::Stableswap::add_assets_liquidity(
			RuntimeOrigin::signed(SIGNER.into()),
			pool_id,
			vec![hydradx_traits::stableswap::AssetAmount::new(
				asset_a,
				1_000_000_000_000_000_000
			)]
			.try_into()
			.unwrap(),
			0,
		));
		let shares = hydradx_runtime::Currencies::free_balance(pool_id, &SIGNER.into());
		assert_ok!(hydradx_runtime::Stableswap::remove_liquidity_one_asset(
			RuntimeOrigin::signed(SIGNER.into()),
			pool_id,
			asset_a,
			shares,
			0,
		));
	});

	g.assert_diff_is_complete();
	assert_eq!(g.delta.unbalanced_assets(), Default::default());
	assert_eq!(g.judgement.verdict, Verdict::Allow(guard::AllowReason::Clean));
}

#[test]
fn guard_should_hold_share_mint_when_no_event_is_emitted() {
	let mut pool = None;
	setup(|| pool = Some(usd_stable_pool().0));
	let pool_id = pool.unwrap();

	let g = guarded(Some(SIGNER), || {
		let minted = 10_000_000_000_000_000_000_000u128;
		orml_tokens::Accounts::<Runtime>::mutate(sp_runtime::AccountId32::from(SIGNER), pool_id, |a| a.free += minted);
		orml_tokens::TotalIssuance::<Runtime>::mutate(pool_id, |i| *i += minted);
	});

	assert!(!g.event_candidate);
	assert!(g.diff_candidate);
	assert_eq!(g.judgement.verdict, Verdict::Hold);
	assert_eq!(
		g.judgement.pools,
		vec![guard::PoolOutcome {
			pool: guard::PoolRef::Stableswap(pool_id),
			vps_bps: -196,
			loss: Usd6(9_803_921_569),
			block_loss: Usd6(9_803_921_569),
		}]
	);
}

#[test]
fn guard_should_report_decode_gap_when_balance_is_written_without_issuance() {
	let mut asset = None;
	setup(|| asset = Some(usd_stable_pool().1));
	let asset = asset.unwrap();

	let g = guarded(Some(SIGNER), || {
		orml_tokens::Accounts::<Runtime>::mutate(sp_runtime::AccountId32::from(SIGNER), asset, |a| a.free += 1);
	});

	assert!(!g.event_candidate);
	assert_eq!(g.delta.unbalanced_assets(), [asset].into());
	assert_eq!(
		g.judgement.verdict,
		Verdict::Inconclusive(InconclusiveReason::DecodeGap)
	);
	assert!(!g.judgement.verdict.blocks());
}

// Every everyday path must conserve, or the guard can't tell what it moved.

fn assert_conserved(g: &Guarded) {
	g.assert_diff_is_complete();
	assert_eq!(g.delta.unbalanced_assets(), Default::default());
	assert_eq!(g.delta.undecodable, Default::default());
	assert!(!g.judgement.verdict.blocks(), "{:?}", g.judgement);
}

fn keyring_signer() -> (sp_core::sr25519::Pair, AccountId) {
	use sp_core::Pair;
	use sp_runtime::traits::IdentifyAccount;
	let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
	let who = sp_runtime::MultiSigner::from(pair.public()).into_account();
	(pair, who)
}

fn fund(who: &AccountId, asset: u32, amount: u128) {
	assert_ok!(hydradx_runtime::Currencies::update_balance(
		RuntimeOrigin::root(),
		who.clone(),
		asset,
		amount as i128,
	));
}

#[test]
fn guard_should_conserve_balances_when_signed_transfer_pays_fees() {
	let (pair, who) = keyring_signer();
	setup(|| fund(&who, HDX, 1_000 * UNITS));

	let g = guarded(None, || {
		crate::utils::executive::assert_executive_apply_signed_extrinsic(
			hydradx_runtime::RuntimeCall::Balances(pallet_balances::Call::transfer_keep_alive {
				dest: BOB.into(),
				value: 10 * UNITS,
			}),
			pair,
		);
	});

	assert_conserved(&g);
	assert_eq!(g.judgement.signer, Some(who.into()));
}

#[test]
fn guard_should_conserve_balances_when_signed_omnipool_sell_pays_fees() {
	let (pair, who) = keyring_signer();
	setup(|| {
		init_omnipool();
		fund(&who, HDX, 1_000 * UNITS);
		fund(&who, DAI, 1_000 * UNITS);
	});

	let g = guarded(None, || {
		crate::utils::executive::assert_executive_apply_signed_extrinsic(
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::sell {
				asset_in: DAI,
				asset_out: HDX,
				amount: 10 * UNITS,
				min_buy_amount: 0,
			}),
			pair,
		);
	});

	assert_conserved(&g);
	let judged: Vec<_> = g.judgement.pools.iter().map(|p| (p.pool.clone(), p.loss)).collect();
	assert_eq!(
		judged,
		vec![
			(guard::PoolRef::Omnipool(HDX), Usd6::ZERO),
			(guard::PoolRef::Omnipool(DAI), Usd6::ZERO)
		]
	);
}

#[test]
fn guard_should_conserve_balances_when_omnipool_liquidity_is_added_and_removed() {
	setup(|| {
		init_omnipool();
		fund(&SIGNER.into(), DAI, 1_000 * UNITS);
	});

	let g = guarded(Some(SIGNER), || {
		let position = hydradx_runtime::Omnipool::next_position_id();
		assert_ok!(hydradx_runtime::Omnipool::add_liquidity(
			RuntimeOrigin::signed(SIGNER.into()),
			DAI,
			100 * UNITS,
		));
		let shares = hydradx_runtime::Omnipool::load_position(position, SIGNER.into())
			.unwrap()
			.shares;
		assert_ok!(hydradx_runtime::Omnipool::remove_liquidity(
			RuntimeOrigin::signed(SIGNER.into()),
			position,
			shares,
		));
	});

	assert_conserved(&g);
}

#[test]
fn guard_should_conserve_balances_when_xyk_pool_is_created_and_traded() {
	setup(|| {
		fund(&SIGNER.into(), DOT, 1_000 * UNITS);
		fund(&SIGNER.into(), BTC, 1_000 * UNITS);
	});

	let g = guarded(Some(SIGNER), || {
		assert_ok!(hydradx_runtime::XYK::create_pool(
			RuntimeOrigin::signed(SIGNER.into()),
			DOT,
			100 * UNITS,
			BTC,
			200 * UNITS,
		));
		assert_ok!(hydradx_runtime::XYK::sell(
			RuntimeOrigin::signed(SIGNER.into()),
			DOT,
			BTC,
			10 * UNITS,
			0,
			false,
		));
	});

	assert_conserved(&g);
	assert_eq!(g.delta.xyk_pools.len(), 1);
}

#[test]
fn guard_should_conserve_balances_when_evm_transaction_pays_gas() {
	use crate::utils::accounts::{alith_evm_account, alith_secret_key};
	use ethereum::{
		eip2930::TransactionSignature, EIP1559Transaction, EIP1559TransactionMessage, TransactionAction, TransactionV2,
	};
	use libsecp256k1::{sign, Message, SecretKey};
	use pallet_evm::FeeCalculator;
	use sp_core::Get;

	TestNet::reset();
	Hydra::execute_with(|| {
		fund(&alith_evm_account(), WETH, 1_000_000_000_000_000_000);
		crate::evm::init_omnipool_with_oracle_for_block_10();
	});

	let g = guarded(None, || {
		let chain_id = <Runtime as pallet_evm::Config>::ChainId::get();
		let (base_gas_price, _) = hydradx_runtime::DynamicEvmFee::min_gas_price();
		let msg = EIP1559TransactionMessage {
			chain_id,
			nonce: U256::zero(),
			max_priority_fee_per_gas: base_gas_price,
			max_fee_per_gas: base_gas_price * 10,
			gas_limit: 1_000_000u64.into(),
			action: TransactionAction::Call(hydradx_runtime::evm::precompiles::DISPATCH_ADDR),
			value: U256::zero(),
			input: hex_literal::hex!("0107081337").to_vec(),
			access_list: vec![],
		};
		let mut hash = [0u8; 32];
		hash.copy_from_slice(&msg.hash().0);
		let (rs, v) = sign(&Message::parse(&hash), &SecretKey::parse(&alith_secret_key()).unwrap());
		let signature =
			TransactionSignature::new(v.serialize() != 0, H256::from(rs.r.b32()), H256::from(rs.s.b32())).unwrap();
		let tx = TransactionV2::EIP1559(EIP1559Transaction {
			chain_id: msg.chain_id,
			nonce: msg.nonce,
			max_priority_fee_per_gas: msg.max_priority_fee_per_gas,
			max_fee_per_gas: msg.max_fee_per_gas,
			gas_limit: msg.gas_limit,
			action: msg.action,
			value: msg.value,
			input: msg.input.clone(),
			access_list: vec![],
			signature,
		});
		crate::utils::executive::assert_executive_apply_unsigned_extrinsic(hydradx_runtime::RuntimeCall::Ethereum(
			pallet_ethereum::Call::transact { transaction: tx.into() },
		));
	});

	assert_conserved(&g);
	assert!(!g.delta.evm_slots.is_empty() || g.delta.balances.keys().any(|(_, a)| *a == WETH));
}
