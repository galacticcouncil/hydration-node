use ethabi::ethereum_types::U256;
use primitives::EvmAddress;
use std::fmt;

/// Address types used throughout the worker.
pub type UserAddress = EvmAddress;
pub type AssetAddress = EvmAddress;
pub type Price = U256;
pub type AssetSymbol = Vec<u8>;
pub type BlockNumber = u32;

/// Event data received when a new block is imported.
#[derive(Clone, Debug)]
pub struct BlockEvent {
	pub block_number: BlockNumber,
	pub block_hash: [u8; 32],
	/// New borrowers discovered in on-chain events.
	pub new_borrowers: Vec<UserAddress>,
	/// Users successfully liquidated in the previous block.
	pub liquidated_users: Vec<UserAddress>,
	/// New asset addresses from CollateralConfigurationChanged events.
	pub new_assets: Vec<AssetAddress>,
}

/// A liquidation transaction to be submitted (or reported).
#[derive(Clone, Debug)]
pub struct LiquidationTx {
	pub user: UserAddress,
	pub collateral_asset: u32,
	pub debt_asset: u32,
	pub debt_to_cover: u128,
	/// For dry-run reporting.
	pub health_factor: U256,
}

impl fmt::Display for LiquidationTx {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"LiquidationTx {{ user: {:?}, collateral: {}, debt: {}, amount: {}, hf: {} }}",
			self.user, self.collateral_asset, self.debt_asset, self.debt_to_cover, self.health_factor
		)
	}
}

/// Result of transaction submission.
#[derive(Clone, Debug)]
pub enum SubmitResult {
	/// Transaction submitted to the pool successfully.
	Submitted,
	/// Transaction was dry-run only (report mode).
	DryRun,
	/// Submission failed with the given reason.
	Failed(String),
}

/// Oracle update data parsed from DIA oracle transactions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OracleUpdate {
	pub asset_address: AssetAddress,
	pub price: Option<Price>,
}

/// Source of new blocks and chain events.
pub trait BlockSource {
	/// Wait (blocking) for the next block event. Returns `None` if the source is exhausted.
	fn next_block(&mut self) -> Option<BlockEvent>;

	/// Non-blocking check for a new block. Returns `None` immediately if no block is available.
	/// Used inside scan loops to check for interrupts without stalling.
	/// Default implementation delegates to `next_block()` (suitable for test/in-memory sources).
	fn try_next_block(&mut self) -> Option<BlockEvent> {
		self.next_block()
	}
}

/// Submits liquidation transactions.
pub trait TxSubmitter {
	/// Submit a liquidation tx. In report mode, just logs.
	fn submit(&self, tx: &LiquidationTx, block_hash: [u8; 32]) -> SubmitResult;
}

/// Oracle update source (mempool interception or manual injection).
pub trait OracleSource {
	/// Poll for oracle updates. Returns empty vec if no update available.
	fn poll_oracle_updates(&mut self) -> Vec<OracleUpdate>;
}

/// Dry-run support for validating liquidation transactions before submission.
pub trait DryRunner {
	/// Returns true if the liquidation tx would succeed on-chain.
	fn dry_run(&self, tx: &LiquidationTx, block_hash: [u8; 32]) -> bool;
}
