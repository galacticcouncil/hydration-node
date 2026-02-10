use super::*;

pub type Bytes32 = [u8; 32];

pub const MAX_SERIALIZED_OUTPUT_LENGTH: u32 = 65_536;

pub const ECDSA: &[u8] = b"ecdsa";
pub const BITCOIN: &[u8] = b"bitcoin";

#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq, MaxEncodedLen)]
pub struct PalletConfigData {
	pub paused: bool,
}

impl Default for PalletConfigData {
	fn default() -> Self {
		Self { paused: false }
	}
}

#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq, MaxEncodedLen)]
pub struct PendingDepositData<AccountId> {
	pub requester: AccountId,
	pub amount_sats: u64,
	pub txid: Bytes32,
	pub path: BoundedVec<u8, ConstU32<256>>,
}

#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq, MaxEncodedLen)]
pub struct PendingWithdrawalData<AccountId> {
	pub requester: AccountId,
	pub amount_sats: u64,
}

pub const ERROR_PREFIX: [u8; 4] = [0xde, 0xad, 0xbe, 0xef];
pub const WITHDRAWAL_PATH: &[u8] = b"root";
