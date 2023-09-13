use crate::evm::precompile::EvmAddress;
use hex_literal::hex;
use primitive_types::H160;
use primitives::AssetId;

/// A mapping between AssetId and Erc20 EVM address.
pub trait Erc20Mapping {
	fn encode_evm_address(asset_id: AssetId) -> Option<EvmAddress>;

	fn decode_evm_address(evm_address: EvmAddress) -> Option<AssetId>;
}

pub struct HydraErc20Mapping;

/// Erc20Mapping logic for HydraDX
/// The asset id (with type u32) is encoded in the last 4 bytes of EVM address
impl Erc20Mapping for HydraErc20Mapping {
	fn encode_evm_address(asset_id: AssetId) -> Option<EvmAddress> {
		let asset_id_bytes: [u8; 4] = asset_id.to_le_bytes();

		let mut evm_address_bytes = [0u8; 20];

		evm_address_bytes[15] = 1;

		for i in 0..4 {
			evm_address_bytes[16 + i] = asset_id_bytes[3 - i];
		}

		Some(EvmAddress::from(evm_address_bytes))
	}

	fn decode_evm_address(evm_address: EvmAddress) -> Option<AssetId> {
		if !is_asset_address(evm_address) {
			return None;
		}

		let mut asset_id: u32 = 0;
		for byte in evm_address.as_bytes() {
			asset_id = (asset_id << 8) | (*byte as u32);
		}

		Some(asset_id)
	}
}

pub fn is_asset_address(address: H160) -> bool {
	let asset_address_prefix = &(H160::from(hex!("0000000000000000000000000000000100000000"))[0..16]);

	&address.to_fixed_bytes()[0..16] == asset_address_prefix
}
