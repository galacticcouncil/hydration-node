use crate::chain_spec;
use std::path::PathBuf;
use std::{fmt, str::FromStr};
use structopt::StructOpt;

#[derive(Debug, Clone)]
pub struct RuntimeInstanceError(String);

impl fmt::Display for RuntimeInstanceError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let RuntimeInstanceError(message) = self;
		write!(f, "RuntimeInstanceError: {}", message)
	}
}

#[derive(Debug, StructOpt)]
pub enum RuntimeInstance {
	HydraDX,
	Testing,
}

impl RuntimeInstance {
	fn variants() -> [&'static str; 2] {
		["hydradx", "testing"]
	}

	pub fn is_testing_runtime(&self) -> bool {
		match self {
			Self::HydraDX => false,
			Self::Testing => true,
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

impl FromStr for RuntimeInstance {
	type Err = RuntimeInstanceError;

	fn from_str(input: &str) -> Result<Self, Self::Err> {
		let input_lower = input.to_lowercase();
		match input_lower.as_str() {
			"testing" => Ok(RuntimeInstance::Testing),
			"hydradx" | "" => Ok(RuntimeInstance::HydraDX),
			other => Err(RuntimeInstanceError(format!("Invalid variant: `{}`", other))),
		}
	}
}

#[derive(Debug, StructOpt)]
pub struct RunCmd {
	#[structopt(flatten)]
	pub base: cumulus_client_cli::RunCmd,

	/// Specify the runtime used by the node.
	#[structopt(default_value, long, possible_values = &RuntimeInstance::variants(), case_insensitive = true)]
	pub runtime: RuntimeInstance,
}

#[derive(Debug, StructOpt)]
#[structopt(settings = &[
	structopt::clap::AppSettings::GlobalVersion,
	structopt::clap::AppSettings::ArgsNegateSubcommands,
	structopt::clap::AppSettings::SubcommandsNegateReqs,
])]
pub struct Cli {
	#[structopt(subcommand)]
	pub subcommand: Option<Subcommand>,

	#[structopt(flatten)]
	pub run: RunCmd,

	/// Relaychain arguments
	#[structopt(raw = true)]
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
			base: polkadot_cli::RunCmd::from_iter(relay_chain_args),
		}
	}
}

#[derive(Debug, StructOpt)]
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
	#[structopt(name = "benchmark", about = "Benchmark runtime pallets.")]
	Benchmark(frame_benchmarking_cli::BenchmarkCmd),

	/// Export the genesis state of the parachain.
	#[structopt(name = "export-genesis-state")]
	ExportGenesisState(ExportGenesisStateCommand),

	/// Export the genesis wasm of the parachain.
	#[structopt(name = "export-genesis-wasm")]
	ExportGenesisWasm(ExportGenesisWasmCommand),

	/// Try some command against runtime state.
	#[cfg(feature = "try-runtime")]
	TryRuntime(try_runtime_cli::TryRuntimeCmd),

	/// Try some command against runtime state. Note: `try-runtime` feature must be enabled.
	#[cfg(not(feature = "try-runtime"))]
	TryRuntime,
}

/// Command for exporting the genesis state of the parachain
#[derive(Debug, StructOpt)]
pub struct ExportGenesisStateCommand {
	/// Output file name or stdout if unspecified.
	#[structopt(parse(from_os_str))]
	pub output: Option<PathBuf>,

	/// Id of the parachain this state is for.
	#[structopt(long, default_value = "200")]
	pub parachain_id: u32,

	/// Write output in binary. Default is to write in hex.
	#[structopt(short, long)]
	pub raw: bool,

	/// The name of the chain for that the genesis state should be exported.
	#[structopt(long)]
	pub chain: Option<String>,

	/// Specify the runtime used by the node.
	#[structopt(default_value, long, possible_values = &RuntimeInstance::variants(), case_insensitive = true)]
	pub runtime: RuntimeInstance,
}

/// Command for exporting the genesis wasm file.
#[derive(Debug, StructOpt)]
pub struct ExportGenesisWasmCommand {
	/// Output file name or stdout if unspecified.
	#[structopt(parse(from_os_str))]
	pub output: Option<PathBuf>,

	/// Write output in binary. Default is to write in hex.
	#[structopt(short, long)]
	pub raw: bool,

	/// The name of the chain for that the genesis wasm file should be exported.
	#[structopt(long)]
	pub chain: Option<String>,

	/// Specify the runtime used by the node.
	#[structopt(default_value, long, possible_values = &RuntimeInstance::variants(), case_insensitive = true)]
	pub runtime: RuntimeInstance,
}
