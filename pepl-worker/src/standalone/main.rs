//! Standalone PEPL liquidation worker binary.
//!
//! Connects to a Hydration node via RPC, monitors borrower health factors,
//! and reports (or optionally executes) liquidation opportunities.
//!
//! Usage:
//!   hydra-liquidator --rpc-url wss://rpc.hydradx.cloud
//!   hydra-liquidator --rpc-url ws://localhost:8000 --submit

use ethabi::ethereum_types::U256;
use jsonrpsee_core::client::{ClientT, SubscriptionClientT};
use jsonrpsee_ws_client::{WsClient, WsClientBuilder};
use liquidation_worker_support::{Borrower, BorrowersData, MoneyMarketData};
use pepl_worker::config::*;
use pepl_worker::standalone::oracle_injector::{OracleInjector, OracleScenario};
use pepl_worker::standalone::rpc_block_source::*;
use pepl_worker::standalone::rpc_provider::RpcState;
use pepl_worker::standalone::report_submitter::{ReportSubmitter, StandaloneDryRunner};
use pepl_worker::standalone::types::*;
use pepl_worker::traits::*;
use sp_core::H160;
use std::cmp::Ordering;
use std::sync::{mpsc, Arc};

#[derive(clap::Parser, Debug)]
#[clap(name = "hydra-liquidator", about = "PEPL Liquidation Worker — standalone mode")]
struct Cli {
	/// WebSocket RPC URL of the Hydration node.
	#[clap(long, default_value = "wss://rpc.hydradx.cloud")]
	rpc_url: String,

	/// Omniwatch API URL for initial borrower data.
	#[clap(long, default_value = DEFAULT_OMNIWATCH_URL)]
	omniwatch_url: String,

	/// Actually submit liquidation transactions (default: report-only).
	#[clap(long, default_value_t = false)]
	submit: bool,

	/// Maximum number of liquidations per block.
	#[clap(long, default_value_t = 10)]
	max_liquidations: usize,

	/// Target health factor (as decimal, e.g. 1.001).
	#[clap(long, default_value_t = 1.001)]
	target_hf: f64,

	/// Oracle scenario JSON file for price injection testing.
	#[clap(long)]
	oracle_scenario: Option<String>,

	/// PoolAddressesProvider contract address (hex).
	#[clap(long)]
	pap_contract: Option<String>,

	/// HF scan threshold: only fetch on-chain data for borrowers with cached HF below this.
	/// Borrowers well above this are skipped to save RPC calls. Set to 0 to scan all.
	#[clap(long, default_value_t = 1.1)]
	hf_threshold: f64,

	/// Complete the full borrower scan even if a new block arrives mid-scan.
	/// By default the scan is interrupted on new blocks (correct for node mode where
	/// re-init is fast). Use this flag in standalone mode where MM re-init is slow (~8-10s).
	#[clap(long, default_value_t = false)]
	no_interrupt: bool,

	/// Re-apply oracle scenario prices after every MoneyMarketData re-init.
	/// Without this, injected prices from --oracle-scenario are lost when fresh
	/// oracle prices are fetched from chain on each new block.
	#[clap(long, default_value_t = false)]
	oracle_persist: bool,
}

#[tokio::main]
async fn main() {
	use clap::Parser;
	env_logger::init();

	let cli = Cli::parse();

	log::info!(target: "pepl-worker", "Starting hydra-liquidator");
	log::info!(target: "pepl-worker", "RPC: {}", cli.rpc_url);
	log::info!(target: "pepl-worker", "Mode: {}", if cli.submit { "SUBMIT" } else { "REPORT ONLY" });

	// 1. Connect to RPC
	log::info!(target: "pepl-worker", "Connecting to {}...", cli.rpc_url);
	let client = match WsClientBuilder::default()
		.max_buffer_capacity_per_subscription(1024)
		.build(&cli.rpc_url)
		.await
	{
		Ok(c) => Arc::new(c),
		Err(e) => {
			log::error!(target: "pepl-worker", "Failed to connect to RPC: {}", e);
			std::process::exit(1);
		}
	};
	log::info!(target: "pepl-worker", "Connected to RPC");

	// 2. Get current timestamp (async — we're in tokio context)
	let timestamp = {
		let block: serde_json::Value = client
			.request(
				"eth_getBlockByNumber",
				vec![serde_json::json!("latest"), serde_json::json!(false)],
			)
			.await
			.unwrap_or_else(|e| {
				log::error!(target: "pepl-worker", "Failed to get latest block: {}", e);
				std::process::exit(1);
			});
		let ts_hex = block
			.get("timestamp")
			.and_then(|v| v.as_str())
			.unwrap_or_else(|| {
				log::error!(target: "pepl-worker", "Block has no timestamp field");
				std::process::exit(1);
			});
		u64::from_str_radix(ts_hex.trim_start_matches("0x"), 16).unwrap_or_else(|_| {
			log::error!(target: "pepl-worker", "Failed to parse timestamp");
			std::process::exit(1);
		})
	};
	log::info!(target: "pepl-worker", "Current EVM timestamp: {}", timestamp);

	// 3. Fetch initial borrowers from Omniwatch (async)
	log::info!(target: "pepl-worker", "Fetching borrowers from {}...", cli.omniwatch_url);
	let borrowers = match fetch_borrowers(&cli.omniwatch_url).await {
		Some(b) => b,
		None => {
			log::error!(target: "pepl-worker", "Failed to fetch borrowers from Omniwatch");
			std::process::exit(1);
		}
	};
	log::info!(target: "pepl-worker", "Fetched {} borrowers", borrowers.len());

	// 4. Parse PAP contract address
	let pap_contract = cli
		.pap_contract
		.as_ref()
		.and_then(|s| {
			let bytes = hex::decode(s.trim_start_matches("0x")).ok()?;
			if bytes.len() == 20 {
				Some(H160::from_slice(&bytes))
			} else {
				None
			}
		})
		.unwrap_or(DEFAULT_PAP_CONTRACT);

	// 5. Set up block subscription
	let (block_tx, block_rx) = mpsc::channel::<BlockEvent>();
	let event_config = EventParserConfig::default();

	let sub_client = client.clone();
	tokio::spawn(async move {
		if let Err(e) = run_block_subscription(sub_client, block_tx, event_config).await {
			log::error!(target: "pepl-worker", "Block subscription failed: {}", e);
		}
	});

	// 6. Set up oracle source (before spawning worker)
	let mut oracle_source = if let Some(scenario_path) = &cli.oracle_scenario {
		let mut injector = OracleInjector::new();
		match std::fs::read_to_string(scenario_path) {
			Ok(json) => match serde_json::from_str::<OracleScenario>(&json) {
				Ok(scenario) => {
					log::info!(target: "pepl-worker", "Loaded oracle scenario: {} updates", scenario.oracle_updates.len());
					injector.load_scenario(&scenario);
				}
				Err(e) => {
					log::error!(target: "pepl-worker", "Failed to parse oracle scenario: {}", e);
					std::process::exit(1);
				}
			},
			Err(e) => {
				log::error!(target: "pepl-worker", "Failed to read oracle scenario file: {}", e);
				std::process::exit(1);
			}
		}
		injector
	} else {
		OracleInjector::new() // empty injector acts as no-op
	};

	// 7. Configure and run worker
	let target_hf_u128 = (cli.target_hf * 1e18) as u128;
	let hf_threshold = if cli.hf_threshold == 0.0 {
		None // scan all borrowers
	} else {
		Some((cli.hf_threshold * 1e18) as u128)
	};
	let config = WorkerConfig {
		pap_contract,
		runtime_api_caller: DEFAULT_RUNTIME_API_CALLER,
		target_hf: target_hf_u128,
		max_liquidations_per_block: cli.max_liquidations,
		dry_run: !cli.submit,
		hf_scan_threshold: hf_threshold,
		no_interrupt: cli.no_interrupt,
		oracle_persist: cli.oracle_persist,
	};

	let submitter = ReportSubmitter;
	let dry_runner = StandaloneDryRunner;
	let mut block_source = RpcBlockSource::new(block_rx);
	let mut borrowers = borrowers;

	log::info!(target: "pepl-worker", "Starting worker (MoneyMarketData init + block loop)...");

	// Run in blocking context — MoneyMarketData::new() calls RuntimeApiProvider
	// which uses Handle::block_on() internally, so it can't run in async context.
	let rpc_state = RpcState::new(client.clone(), tokio::runtime::Handle::current());
	let worker_handle = tokio::task::spawn_blocking(move || {
		// Initialize MoneyMarketData (makes eth_call via Handle::block_on)
		log::info!(target: "pepl-worker", "Initializing money market (PAP: {:?})...", pap_contract);
		let mut money_market = match MoneyMarketData::<
			StandaloneBlock,
			StandaloneOriginCaller,
			StandaloneRuntimeCall,
			StandaloneRuntimeEvent,
		>::new(&rpc_state, Default::default(), pap_contract, DEFAULT_RUNTIME_API_CALLER)
		{
			Ok(mm) => mm,
			Err(e) => {
				log::error!(target: "pepl-worker", "Failed to initialize money market: {:?}", e);
				return;
			}
		};
		log::info!(target: "pepl-worker", "Money market initialized: {} reserves", money_market.reserves().len());

		log::info!(target: "pepl-worker", "Listening for new blocks...");

		let api: &RpcState = &rpc_state;
		pepl_worker::run_worker::<
			StandaloneBlock,
			StandaloneOriginCaller,
			StandaloneRuntimeCall,
			StandaloneRuntimeEvent,
			_,
			_,
			_,
			_,
			&RpcState,
		>(
			&mut block_source,
			&submitter,
			&mut oracle_source,
			&dry_runner,
			&api,
			&config,
			&mut money_market,
			&mut borrowers,
			timestamp,
		);
	});

	if let Err(e) = worker_handle.await {
		log::error!(target: "pepl-worker", "Worker task failed: {:?}", e);
	}
}

/// Subscribe to new block headers and send parsed events to the channel.
async fn run_block_subscription(
	client: Arc<WsClient>,
	tx: mpsc::Sender<BlockEvent>,
	event_config: EventParserConfig,
) -> Result<(), String> {
	let mut sub = client
		.subscribe::<serde_json::Value, Vec<serde_json::Value>>(
			"chain_subscribeNewHeads",
			vec![],
			"chain_unsubscribeNewHeads",
		)
		.await
		.map_err(|e| format!("Failed to subscribe to new heads: {}", e))?;

	while let Some(Ok(header_value)) = sub.next().await {
		let header: SubstrateHeader = match serde_json::from_value(header_value) {
			Ok(h) => h,
			Err(e) => {
				log::warn!(target: "pepl-worker", "Failed to parse header: {}", e);
				continue;
			}
		};

		let block_number =
			u32::from_str_radix(header.number.trim_start_matches("0x"), 16).unwrap_or(0);

		// Get the block hash for this block number
		let block_hash_hex: String = match client
			.request(
				"chain_getBlockHash",
				vec![serde_json::json!(block_number)],
			)
			.await
		{
			Ok(h) => h,
			Err(e) => {
				log::warn!(target: "pepl-worker", "Failed to get block hash for #{}: {}", block_number, e);
				continue;
			}
		};

		let block_hash = {
			let bytes = hex::decode(block_hash_hex.trim_start_matches("0x")).unwrap_or_default();
			let mut hash = [0u8; 32];
			if bytes.len() == 32 {
				hash.copy_from_slice(&bytes);
			}
			hash
		};

		// Fetch EVM logs for this block
		let hex_block = format!("0x{:x}", block_number);
		let (new_borrowers, liquidated_users, new_assets) = match client
			.request::<serde_json::Value, _>(
				"eth_getLogs",
				vec![serde_json::json!({
					"fromBlock": hex_block,
					"toBlock": hex_block,
				})],
			)
			.await
		{
			Ok(value) => {
				let logs: Vec<EvmLog> = serde_json::from_value(value).unwrap_or_default();
				parse_evm_logs(&logs, &event_config)
			}
			Err(e) => {
				log::debug!(target: "pepl-worker", "eth_getLogs failed for block #{}: {}", block_number, e);
				(vec![], vec![], vec![])
			}
		};

		let event = BlockEvent {
			block_number,
			block_hash,
			new_borrowers,
			liquidated_users,
			new_assets,
		};

		if tx.send(event).is_err() {
			log::info!(target: "pepl-worker", "Block channel closed, stopping subscription");
			break;
		}
	}

	Ok(())
}

/// Fetch initial borrowers from the Omniwatch API.
async fn fetch_borrowers(url: &str) -> Option<Vec<Borrower>> {
	let resp = reqwest::get(url).await.ok()?;
	if !resp.status().is_success() {
		log::error!(target: "pepl-worker", "Omniwatch returned status {}", resp.status());
		return None;
	}

	let data: BorrowersData<String> = resp.json().await.ok()?;
	let one = U256::from(10u128.pow(18));
	let fractional_multiplier = U256::from(10u128.pow(12));

	let mut borrowers: Vec<Borrower> = data
		.borrowers
		.iter()
		.map(|(user_address, details)| {
			let integer_part =
				U256::from(details.health_factor.trunc() as u128).checked_mul(one);
			let fractional_part =
				U256::from((details.health_factor.fract() * 1_000_000f32) as u128)
					.checked_mul(fractional_multiplier);

			let health_factor = integer_part
				.zip(fractional_part)
				.and_then(|(i, f)| i.checked_add(f))
				.unwrap_or_default();

			Borrower {
				user_address: *user_address,
				health_factor,
			}
		})
		.collect();

	borrowers.sort_by(|a, b| {
		a.health_factor
			.partial_cmp(&b.health_factor)
			.unwrap_or(Ordering::Equal)
	});

	Some(borrowers)
}
