#![allow(clippy::upper_case_acronyms)]

use structopt::StructOpt;
use std::{fmt, str::FromStr};

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
	fn default() -> Self { RuntimeInstance::HydraDX }
}

impl FromStr for RuntimeInstance {
	type Err = RuntimeInstanceError;

	fn from_str(input: &str) -> Result<Self, Self::Err> {
		let input_lower = input.to_lowercase();
		match input_lower.as_str() {
			"testing" => Ok( RuntimeInstance::Testing ),
			"hydradx" | "" => Ok( RuntimeInstance::HydraDX ),
			other => Err(RuntimeInstanceError( format!("Invalid variant: `{}`", other)))
		}
	}
}

#[derive(Debug, StructOpt)]
pub struct Cli {
	#[structopt(subcommand)]
	pub subcommand: Option<Subcommand>,

	#[structopt(flatten)]
	pub run: RunCmd,
}

#[derive(Debug, StructOpt)]
pub struct RunCmd {
	#[structopt(flatten)]
	pub base: sc_cli::RunCmd,

	/// Specify the runtime used by the node.
	#[structopt(default_value, long, possible_values = &RuntimeInstance::variants(), case_insensitive = true)]
	pub runtime: RuntimeInstance,
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
	PurgeChain(sc_cli::PurgeChainCmd),

	/// Revert the chain to a previous state.
	Revert(sc_cli::RevertCmd),

	/// The custom benchmark subcommmand benchmarking runtime pallets.
	#[structopt(name = "benchmark", about = "Benchmark runtime pallets.")]
	Benchmark(frame_benchmarking_cli::BenchmarkCmd),
}
