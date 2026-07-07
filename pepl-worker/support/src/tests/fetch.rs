use crate::fetch_addresses_provider;
use crate::traits::{RuntimeApiErr, RuntimeApiProvider};
use crate::types::{Error, Timestamp};
use fp_evm::{ExitReason, ExitSucceed};
use primitives::{AssetId, Balance, EvmAddress};
use sp_core::U256;

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
