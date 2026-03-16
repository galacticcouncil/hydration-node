use primitives::EvmAddress;
use sp_core::H160;

/// Default constants matching the node configuration.
pub const DEFAULT_PAP_CONTRACT: EvmAddress = H160(hex_literal::hex!(
	"f3ba4d1b50f78301bdd7eaea9b67822a15fca691"
));
pub const DEFAULT_RUNTIME_API_CALLER: EvmAddress = H160(hex_literal::hex!(
	"33a5e905fB83FcFB62B0Dd1595DfBc06792E054e"
));
pub const DEFAULT_BORROW_CALL_ADDRESS: EvmAddress = H160(hex_literal::hex!(
	"1b02E051683b5cfaC5929C25E84adb26ECf87B38"
));
pub const DEFAULT_POOL_CONFIGURATOR_ADDRESS: EvmAddress = H160(hex_literal::hex!(
	"e64c38e2fa00dfe4f1d0b92f75b8e44ebdf292e4"
));

pub const DEFAULT_ORACLE_UPDATE_SIGNERS: &[EvmAddress] = &[
	H160(hex_literal::hex!("33a5e905fB83FcFB62B0Dd1595DfBc06792E054e")),
	H160(hex_literal::hex!("ff0c624016c873d359dde711b42a2f475a5a07d3")),
];

pub const DEFAULT_ORACLE_UPDATE_CALL_ADDRESSES: &[EvmAddress] = &[
	H160(hex_literal::hex!("dee629af973ebf5bf261ace12ffd1900ac715f5e")),
	H160(hex_literal::hex!("48ae7803cd09c48434e3fc5629f15fb76f0b5ce5")),
];

pub const DEFAULT_TARGET_HF: u128 = 1_001_000_000_000_000_000u128; // 1.001
pub const DEFAULT_WEIGHT_RESERVE: u8 = 10u8;
pub const DEFAULT_OMNIWATCH_URL: &str = "https://omniwatch.play.hydration.cloud/api/borrowers/by-health";

/// Number of blocks after which a waitlisted user without a Liquidated event is re-evaluated.
pub const WAITLIST_TTL_BLOCKS: u32 = 2;

/// Default HF scan threshold: only fetch on-chain data for borrowers with cached HF below this.
/// 1.1 in 18-decimal format. Borrowers well above 1.0 are skipped to save RPC calls.
pub const DEFAULT_HF_SCAN_THRESHOLD: u128 = 1_100_000_000_000_000_000;

/// Configuration for the liquidation worker, independent of node types.
#[derive(Clone, Debug)]
pub struct WorkerConfig {
	/// Address of the Pool Address Provider contract.
	pub pap_contract: EvmAddress,
	/// EVM address of the account that calls Runtime API.
	pub runtime_api_caller: EvmAddress,
	/// Target health factor.
	pub target_hf: u128,
	/// Maximum number of liquidation transactions per block.
	pub max_liquidations_per_block: usize,
	/// Whether to run in dry-run (report-only) mode.
	pub dry_run: bool,
	/// HF scan threshold: only fetch on-chain user data for borrowers with cached HF below this.
	/// Borrowers with HF above this are considered safe and skipped until an oracle update changes
	/// reserve prices. Set to `None` to scan all borrowers every block (node mode).
	pub hf_scan_threshold: Option<u128>,
	/// When true, complete the full borrower scan even if a new block arrives mid-scan.
	/// Default is `false` (interrupt on new block) which is correct for node mode where
	/// MoneyMarketData re-init is fast (~200ms) and stale-state scanning is wasteful.
	/// Standalone mode should set this to `true` because MM re-init takes ~8-10s over RPC
	/// and interrupting means borrowers are never evaluated.
	pub no_interrupt: bool,
	/// When true, oracle scenario overrides are re-applied after every MoneyMarketData re-init.
	/// Without this, injected prices are lost when the MM fetches fresh oracle prices from chain.
	/// Only useful in standalone mode with `--oracle-scenario`.
	pub oracle_persist: bool,
}

impl Default for WorkerConfig {
	fn default() -> Self {
		Self {
			pap_contract: DEFAULT_PAP_CONTRACT,
			runtime_api_caller: DEFAULT_RUNTIME_API_CALLER,
			target_hf: DEFAULT_TARGET_HF,
			max_liquidations_per_block: 10,
			dry_run: false,
			hf_scan_threshold: None, // node mode: scan all
			no_interrupt: false,    // node mode: interrupt on new block (MM re-init is fast)
			oracle_persist: false,  // no persistent oracle overrides by default
		}
	}
}
