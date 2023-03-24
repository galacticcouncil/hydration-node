use crate::chain_spec;
use clap::{builder::PossibleValue, Parser};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RuntimeInstanceError(String);

impl fmt::Display for RuntimeInstanceError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let RuntimeInstanceError(message) = self;
		write!(f, "RuntimeInstanceError: {message}")
	}
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Parser)]
pub enum RuntimeInstance {
	HydraDX,
	Testing,
}

impl RuntimeInstance {
	pub fn is_testing_runtime(&self) -> bool {
		match self {
			Self::HydraDX => false,
			Self::Testing => true,
		}
	}
}

impl clap::ValueEnum for RuntimeInstance {
	fn value_variants<'a>() -> &'a [Self] {
		&[Self::HydraDX, Self::Testing]
	}

	fn to_possible_value(&self) -> Option<PossibleValue> {
		match self {
			Self::HydraDX => Some(PossibleValue::new("hydradx")),
			Self::Testing => Some(PossibleValue::new("testing")),
		}
	}
}

impl fmt::Display for RuntimeInstance {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			Self::HydraDX => write!(f, "hydradx"),
			Self::Testing => write!(f, "testing"),
		}
	}
}

impl Default for RuntimeInstance {
	fn default() -> Self {
		RuntimeInstance::HydraDX
	}
}

#[derive(Debug, Parser)]
#[clap(
	propagate_version = true,
	args_conflicts_with_subcommands = true,
	subcommand_negates_reqs = true
)]
pub struct RunCmd {
	#[clap(flatten)]
	pub base: cumulus_client_cli::RunCmd,

	/// Specify the runtime used by the node.
	#[clap(default_value_t, long, value_parser = clap::builder::EnumValueParser::<RuntimeInstance>::new(), ignore_case = true)]
	pub runtime: RuntimeInstance,
}

#[derive(Debug, Parser)]
pub struct Cli {
	#[clap(subcommand)]
	pub subcommand: Option<Subcommand>,

	#[clap(flatten)]
	pub run: RunCmd,

	/// Relaychain arguments
	#[clap(raw = true)]
	pub relaychain_args: Vec<String>,
}

#[derive(Debug)]
pub struct RelayChainCli {
	/// The actual relay chain cli object.
	pub base: polkadot_cli::RunCmd,

	/// Optional chain id that should be passed to the relay chain.
	pub chain_id: Option<String>,

	/// The base path that should be used by the relay chain.
	pub base_path: Option<PathBuf>,
}

impl RelayChainCli {
	/// Parse the relay chain CLI parameters using the para chain `Configuration`.
	pub fn new<'a>(
		para_config: &sc_service::Configuration,
		relay_chain_args: impl Iterator<Item = &'a String>,
	) -> Self {
		let extension = chain_spec::Extensions::try_get(&para_config.chain_spec);
		let chain_id = extension.map(|e| e.relay_chain.clone());
		let base_path = para_config.base_path.as_ref().map(|x| x.path().join("polkadot"));
		Self {
			base_path,
			chain_id,
			base: polkadot_cli::RunCmd::parse_from(relay_chain_args),
		}
	}
}

#[derive(Debug, Parser)]
pub enum Subcommand {
	/// Build a chain specification.
	BuildSpec(sc_cli::BuildSpecCmd),

	/// Validate blocks.
	CheckBlock(sc_cli::CheckBlockCmd),

	/// Export blocks.
	ExportBlocks(sc_cli::ExportBlocksCmd),

	/// Export the state of a given block into a chain spec.
	ExportState(sc_cli::ExportStateCmd),

	/// Import blocks.
	ImportBlocks(sc_cli::ImportBlocksCmd),

	/// Remove the whole chain.
	PurgeChain(cumulus_client_cli::PurgeChainCmd),

	/// Revert the chain to a previous state.
	Revert(sc_cli::RevertCmd),

	/// The custom benchmark subcommmand benchmarking runtime pallets.
	#[clap(subcommand)]
	Benchmark(frame_benchmarking_cli::BenchmarkCmd),

	/// Export the genesis state of the parachain.
	#[clap(name = "export-genesis-state")]
	ExportGenesisState(ExportGenesisStateCommand),

	/// Export the genesis wasm of the parachain.
	#[clap(name = "export-genesis-wasm")]
	ExportGenesisWasm(ExportGenesisWasmCommand),

	/// Try some command against runtime state.
	#[cfg(feature = "try-runtime")]
	TryRuntime(try_runtime_cli::TryRuntimeCmd),

	/// Try some command against runtime state. Note: `try-runtime` feature must be enabled.
	#[cfg(not(feature = "try-runtime"))]
	TryRuntime,
}

/// Command for exporting the genesis state of the parachain
#[derive(Debug, Parser)]
pub struct ExportGenesisStateCommand {
	/// Output file name or stdout if unspecified.
	#[clap(value_parser)]
	pub output: Option<PathBuf>,

	/// Write output in binary. Default is to write in hex.
	#[clap(short, long)]
	pub raw: bool,

	/// The name of the chain for that the genesis state should be exported.
	#[clap(long)]
	pub chain: Option<String>,

	/// Specify the runtime used by the node.
	#[clap(default_value_t, long, value_parser = clap::builder::EnumValueParser::<RuntimeInstance>::new(), ignore_case = true)]
	pub runtime: RuntimeInstance,
}

/// Command for exporting the genesis wasm file.
#[derive(Debug, Parser)]
pub struct ExportGenesisWasmCommand {
	/// Output file name or stdout if unspecified.
	#[clap(value_parser)]
	pub output: Option<PathBuf>,

	/// Write output in binary. Default is to write in hex.
	#[clap(short, long)]
	pub raw: bool,

	/// The name of the chain for that the genesis wasm file should be exported.
	#[clap(long)]
	pub chain: Option<String>,

	/// Specify the runtime used by the node.
	#[clap(default_value_t, long, value_parser = clap::builder::EnumValueParser::<RuntimeInstance>::new(), ignore_case = true)]
	pub runtime: RuntimeInstance,
}
