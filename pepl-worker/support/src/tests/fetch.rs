use crate::fetch_addresses_provider;
use crate::math::OCTILL;
use crate::traits::{RuntimeApiErr, RuntimeApiProvider};
use crate::types::{Error, MoneyMarket, Reserve, ReserveData, Timestamp};
use crate::{Function, Hydration};
use fp_evm::{ExitReason, ExitSucceed};
use primitives::{AssetId, Balance, EvmAddress};
use sp_core::U256;
use std::cell::RefCell;
use std::collections::HashMap;

type TestBlock = sp_runtime::generic::Block<
	sp_runtime::generic::Header<u32, sp_runtime::traits::BlakeTwo256>,
	sp_runtime::OpaqueExtrinsic,
>;

struct MockApi {
	ret: Vec<u8>,
}

impl RuntimeApiProvider<TestBlock> for MockApi {
	fn call(
		&self,
		_block: sp_core::H256,
		_from: EvmAddress,
		_to: EvmAddress,
		_data: Vec<u8>,
		_gas_limit: U256,
	) -> Result<fp_evm::ExecutionInfoV2<Vec<u8>>, RuntimeApiErr> {
		Ok(fp_evm::ExecutionInfoV2 {
			exit_reason: ExitReason::Succeed(ExitSucceed::Returned),
			value: self.ret.clone(),
			used_gas: fp_evm::UsedGas {
				standard: U256::zero(),
				effective: U256::zero(),
			},
			weight_info: None,
			logs: Vec::new(),
		})
	}

	fn address_to_asset(&self, _block: sp_core::H256, _address: EvmAddress) -> Result<Option<AssetId>, RuntimeApiErr> {
		Ok(None)
	}

	fn minimum_balance(&self, _block: sp_core::H256, _asset_id: AssetId) -> Result<Balance, RuntimeApiErr> {
		Ok(0)
	}

	fn timestamp(&self, _block: sp_core::H256) -> Option<Timestamp> {
		None
	}
}

#[test]
fn fetch_addresses_provider_should_return_provider_when_pool_answers() {
	let pap = EvmAddress::from_slice(&[0xAA; 20]);
	let mut word = vec![0u8; 32];
	word[12..].copy_from_slice(pap.as_bytes());

	let api = MockApi { ret: word };
	let got = fetch_addresses_provider::<TestBlock, _>(
		&api,
		Default::default(),
		EvmAddress::zero(),
		EvmAddress::from_slice(&[0xBB; 20]),
	)
	.expect("resolution should succeed");

	assert_eq!(got, pap);
}

#[test]
fn fetch_addresses_provider_should_fail_when_return_data_is_short() {
	let api = MockApi { ret: vec![0u8; 8] };
	let res = fetch_addresses_provider::<TestBlock, _>(
		&api,
		Default::default(),
		EvmAddress::zero(),
		EvmAddress::from_slice(&[0xBB; 20]),
	);

	assert!(matches!(res, Err(Error::DecodeInvalidLength)));
}

const POOL: EvmAddress = EvmAddress::repeat_byte(0xF0);
const RESERVE: EvmAddress = EvmAddress::repeat_byte(0x11);
const A_TOKEN: EvmAddress = EvmAddress::repeat_byte(0x22);
const VARIABLE_DEBT_TOKEN: EvmAddress = EvmAddress::repeat_byte(0x33);
const STABLE_DEBT_TOKEN: EvmAddress = EvmAddress::repeat_byte(0x44);
const BORROWER: EvmAddress = EvmAddress::repeat_byte(0x55);

/// Answers by `(callee, selector)` and records every call, so a test can assert both the value
/// returned and that a call was (or was not) made at all.
struct RoutedApi {
	answers: HashMap<(EvmAddress, u32), U256>,
	calls: RefCell<Vec<(EvmAddress, u32)>>,
}

impl RoutedApi {
	fn new(answers: Vec<((EvmAddress, Function), u128)>) -> Self {
		Self {
			answers: answers
				.into_iter()
				.map(|((to, f), v)| ((to, Into::<u32>::into(f)), U256::from(v)))
				.collect(),
			calls: RefCell::new(Vec::new()),
		}
	}

	fn called(&self, to: EvmAddress, f: Function) -> bool {
		let selector = Into::<u32>::into(f);
		self.calls.borrow().contains(&(to, selector))
	}
}

impl RuntimeApiProvider<TestBlock> for RoutedApi {
	fn call(
		&self,
		_block: sp_core::H256,
		_from: EvmAddress,
		to: EvmAddress,
		data: Vec<u8>,
		_gas_limit: U256,
	) -> Result<fp_evm::ExecutionInfoV2<Vec<u8>>, RuntimeApiErr> {
		let selector = u32::from_be_bytes(data[0..4].try_into().expect("selector is 4 bytes"));
		self.calls.borrow_mut().push((to, selector));

		let word = self
			.answers
			.get(&(to, selector))
			.copied()
			.unwrap_or_default()
			.to_big_endian();

		Ok(fp_evm::ExecutionInfoV2 {
			exit_reason: ExitReason::Succeed(ExitSucceed::Returned),
			value: word.to_vec(),
			used_gas: fp_evm::UsedGas {
				standard: U256::zero(),
				effective: U256::zero(),
			},
			weight_info: None,
			logs: Vec::new(),
		})
	}

	fn address_to_asset(&self, _block: sp_core::H256, _address: EvmAddress) -> Result<Option<AssetId>, RuntimeApiErr> {
		Ok(None)
	}

	fn minimum_balance(&self, _block: sp_core::H256, _asset_id: AssetId) -> Result<Balance, RuntimeApiErr> {
		Ok(0)
	}

	fn timestamp(&self, _block: sp_core::H256) -> Option<Timestamp> {
		None
	}
}

/// A single reserve the borrower has debt in. 8 decimals at a price of 1.0 so token amounts and
/// base-currency amounts coincide, and `variable_borrow_index == RAY` so the variable leg's
/// `ray_mul` is the identity.
fn debt_market(has_stable_debt: bool) -> MoneyMarket {
	let reserve = Reserve {
		idx: 0,
		data: ReserveData {
			configuration: U256::from(8u128) << 48, // decimals
			liquidity_index: OCTILL,
			current_liquidity_rate: 0,
			variable_borrow_index: OCTILL,
			current_variable_borrow_rate: 0,
			last_update_timestamp: 0,
			a_token_address: A_TOKEN,
			stable_debt_token_address: STABLE_DEBT_TOKEN,
			variable_debt_token_address: VARIABLE_DEBT_TOKEN,
		},
		address: RESERVE,
		asset_id: 0,
		symbol: "TST".to_string(),
		price: U256::from(100_000_000u128),
		existential_deposit: 0,
		emode: None,
		has_stable_debt,
	};

	let mut reserves = HashMap::new();
	reserves.insert(reserve.address, reserve);

	MoneyMarket {
		pool: POOL,
		oracle: EvmAddress::zero(),
		reserves,
		poisoned: Vec::new(),
	}
}

// `UserConfiguration` packs two bits per reserve: debt at `2*idx`, collateral at `2*idx + 1`.
const USER_CONFIG_DEBT_IN_RESERVE_0: u128 = 1;

fn fetch_debt(has_stable_debt: bool, variable_scaled: u128, stable: u128) -> (u128, RoutedApi) {
	let api = RoutedApi::new(vec![
		((POOL, Function::GetUserConfiguration), USER_CONFIG_DEBT_IN_RESERVE_0),
		((POOL, Function::GetUserEMode), 0),
		((VARIABLE_DEBT_TOKEN, Function::ScaledBalanceOf), variable_scaled),
		((STABLE_DEBT_TOKEN, Function::BalanceOf), stable),
	]);

	let mm = debt_market(has_stable_debt);
	let borrower = Hydration::new(EvmAddress::zero(), POOL, "test")
		.fetch_borrower::<TestBlock, _>(&api, Default::default(), 1, &mm, BORROWER, 0)
		.expect("borrower should be fetched");

	(borrower.total_debt.as_u128(), api)
}

// Aave's `StableDebtToken.balanceOf` is already compounded, so the stable leg enters `total_debt`
// unscaled while the variable leg is `ray_mul`-ed by the borrow index.
#[test]
fn fetch_borrower_should_sum_variable_and_stable_debt_when_reserve_carries_stable_debt() {
	let (total_debt, api) = fetch_debt(true, 500, 250);

	assert_eq!(total_debt, 750);
	assert!(api.called(STABLE_DEBT_TOKEN, Function::BalanceOf));
}

// The regression this guards: a stable-only borrower read as zero debt is pruned from the working
// set after `ZERO_DEBT_READS_BEFORE_PRUNE` scans and never liquidated.
#[test]
fn fetch_borrower_should_report_stable_only_debt_when_variable_debt_is_zero() {
	let (total_debt, _) = fetch_debt(true, 0, 1_000);

	assert_eq!(total_debt, 1_000);
}

// With zero stable supply on the reserve no borrower can hold stable debt, so the scan must not
// spend an EVM call per borrower proving it.
#[test]
fn fetch_borrower_should_not_read_stable_debt_when_reserve_has_none() {
	let (total_debt, api) = fetch_debt(false, 500, 250);

	assert_eq!(total_debt, 500);
	assert!(!api.called(STABLE_DEBT_TOKEN, Function::BalanceOf));
}
