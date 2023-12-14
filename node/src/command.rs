// This file is part of Substrate.

// Copyright (C) 2017-2020 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::chain_spec;
use crate::cli::{Cli, RelayChainCli, Subcommand};
use crate::service::{new_partial, HydraDXNativeExecutor};

use codec::Encode;
use cumulus_client_cli::generate_genesis_block;
use cumulus_primitives_core::ParaId;
use frame_benchmarking_cli::{BenchmarkCmd, SUBSTRATE_REFERENCE_HARDWARE};
use hydradx_runtime::Block;
use log::info;
use sc_cli::{
	ChainSpec, CliConfiguration, DefaultConfigurationValues, ImportParams, KeystoreParams, NetworkParams, Result,
	RuntimeVersion, SharedParams, SubstrateCli,
};
use sc_executor::{sp_wasm_interface::ExtendedHostFunctions, NativeExecutionDispatch};
use sc_service::config::{BasePath, PrometheusConfig};
use sp_core::hexdisplay::HexDisplay;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::traits::Block as BlockT;
use std::io::Write;

fn load_spec(id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
	Ok(match id {
		"" => Box::new(chain_spec::hydradx::parachain_config()?),
		"local" | "dev" => Box::new(chain_spec::local::parachain_config()?),
		"staging" => Box::new(chain_spec::staging::parachain_config()?),
		"rococo" => Box::new(chain_spec::rococo::parachain_config()?),
		"moonbase" => Box::new(chain_spec::moonbase::parachain_config()?),
		path => Box::new(chain_spec::ChainSpec::from_json_file(std::path::PathBuf::from(path))?),
	})
}

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"HydraDX".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		env!("CARGO_PKG_DESCRIPTION").into()
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"https://github.com/galacticcouncil/HydraDX-node/issues".into()
	}

	fn copyright_start_year() -> i32 {
		2020
	}

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
		let id = if id.is_empty() { "hydradx" } else { id };

		Ok(match id {
			"hydradx" => Box::new(chain_spec::hydradx::parachain_config()?),
			"local" | "dev" => Box::new(chain_spec::local::parachain_config()?),
			"staging" => Box::new(chain_spec::staging::parachain_config()?),
			"rococo" => Box::new(chain_spec::rococo::parachain_config()?),
			"moonbase" => Box::new(chain_spec::moonbase::parachain_config()?),
			path => Box::new(chain_spec::ChainSpec::from_json_file(std::path::PathBuf::from(path))?),
		})
	}
}

impl SubstrateCli for RelayChainCli {
	fn impl_name() -> String {
		"HydraDX".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		env!("CARGO_PKG_DESCRIPTION").into()
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"https://github.com/galacticcouncil/HydraDX-node/issues".into()
	}

	fn copyright_start_year() -> i32 {
		2020
	}

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
		polkadot_cli::Cli::from_iter([RelayChainCli::executable_name()].iter()).load_spec(id)
	}
}

#[allow(clippy::borrowed_box)]
fn extract_genesis_wasm(chain_spec: &Box<dyn sc_service::ChainSpec>) -> Result<Vec<u8>> {
	let mut storage = chain_spec.build_storage()?;

	storage
		.top
		.remove(sp_core::storage::well_known_keys::CODE)
		.ok_or_else(|| "Could not find wasm file in genesis state!".into())
}

/// Parse and run command line arguments
pub fn run() -> sc_cli::Result<()> {
	let cli = Cli::from_args();

	match &cli.subcommand {
		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		}
		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let partials = new_partial(&config)?;
				Ok((cmd.run(partials.client, partials.import_queue), partials.task_manager))
			})
		}
		Some(Subcommand::ExportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let partials = new_partial(&config)?;
				Ok((cmd.run(partials.client, config.database), partials.task_manager))
			})
		}
		Some(Subcommand::ExportState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let partials = new_partial(&config)?;
				Ok((cmd.run(partials.client, config.chain_spec), partials.task_manager))
			})
		}
		Some(Subcommand::ImportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let partials = new_partial(&config)?;
				Ok((cmd.run(partials.client, partials.import_queue), partials.task_manager))
			})
		}
		Some(Subcommand::PurgeChain(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| {
				let polkadot_cli = RelayChainCli::new(
					&config,
					[RelayChainCli::executable_name()]
						.iter()
						.chain(cli.relaychain_args.iter()),
				);

				let polkadot_config =
					SubstrateCli::create_configuration(&polkadot_cli, &polkadot_cli, config.tokio_handle.clone())
						.map_err(|err| format!("Relay chain argument error: {err}"))?;

				cmd.run(config, polkadot_config)
			})
		}
		Some(Subcommand::Revert(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let partials = new_partial(&config)?;
				Ok((cmd.run(partials.client, partials.backend, None), partials.task_manager))
			})
		}
		Some(Subcommand::Benchmark(cmd)) => {
			let runner = cli.create_runner(cmd)?;

			match cmd {
				BenchmarkCmd::Pallet(cmd) => {
					if cfg!(feature = "runtime-benchmarks") {
						runner.sync_run(|config| {
							cmd.run::<Block, ExtendedHostFunctions<
								sp_io::SubstrateHostFunctions,
								<HydraDXNativeExecutor as NativeExecutionDispatch>::ExtendHostFunctions,
							>>(config)
						})
					} else {
						Err("Benchmarking wasn't enabled when building the node. \
			   You can enable it with `--features runtime-benchmarks`."
							.into())
					}
				}
				BenchmarkCmd::Block(cmd) => runner.sync_run(|config| {
					let partials = crate::service::new_partial(&config)?;
					cmd.run(partials.client)
				}),
				#[cfg(not(feature = "runtime-benchmarks"))]
				BenchmarkCmd::Storage(_) => Err("Storage benchmarking can be enabled with `--features runtime-benchmarks`.".into()),
				#[cfg(feature = "runtime-benchmarks")]
				BenchmarkCmd::Storage(cmd) => runner.sync_run(|config| {
					let partials = new_partial(&config)?;
					let db = partials.backend.expose_db();
					let storage = partials.backend.expose_storage();

					cmd.run(config, partials.client, db, storage)
				}),
				BenchmarkCmd::Overhead(_) | BenchmarkCmd::Extrinsic(_) => {
					Err("Unsupported benchmarking command".into())
				}
				BenchmarkCmd::Machine(cmd) => {
					runner.sync_run(|config| cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone()))
				}
			}
		}
		Some(Subcommand::ExportGenesisState(params)) => {
			let mut builder = sc_cli::LoggerBuilder::new("");
			builder.with_profiling(sc_tracing::TracingReceiver::Log, "");
			let _ = builder.init();

			let spec = load_spec(&params.shared_params.chain.clone().unwrap_or_default())?;
			let state_version = Cli::runtime_version().state_version();

			let block: Block = generate_genesis_block(&*spec, state_version)?;
			let raw_header = block.header().encode();
			let output_buf = if params.raw {
				raw_header
			} else {
				format!("0x{:?}", HexDisplay::from(&block.header().encode())).into_bytes()
			};

			if let Some(output) = &params.output {
				std::fs::write(output, output_buf)?;
			} else {
				std::io::stdout().write_all(&output_buf)?;
			}

			Ok(())
		}
		Some(Subcommand::ExportGenesisWasm(params)) => {
			let mut builder = sc_cli::LoggerBuilder::new("");
			builder.with_profiling(sc_tracing::TracingReceiver::Log, "");
			let _ = builder.init();

			let raw_wasm_blob =
				extract_genesis_wasm(&load_spec(&params.shared_params.chain.clone().unwrap_or_default())?)?;
			let output_buf = if params.raw {
				raw_wasm_blob
			} else {
				format!("0x{:?}", HexDisplay::from(&raw_wasm_blob)).into_bytes()
			};

			if let Some(output) = &params.output {
				std::fs::write(output, output_buf)?;
			} else {
				std::io::stdout().write_all(&output_buf)?;
			}

			Ok(())
		}
		Some(Subcommand::TryRuntime) => Err("The `try-runtime` subcommand has been migrated to a standalone CLI (https://github.com/paritytech/try-runtime-cli). It is no longer being maintained here and will be removed entirely some time after January 2024. Please remove this subcommand from your runtime and use the standalone CLI.".into()),
		None => {
			let runner = cli.create_runner(&cli.run.base.normalize())?;

			runner.run_node_until_exit(|config| async move {
				if cfg!(feature = "runtime-benchmarks") && config.role.is_authority() {
					return Err("It is not allowed to run a collator node with the benchmarking runtime.".into());
				};

				let hwbench = (!cli.no_hardware_benchmarks)
					.then_some(config.database.path().map(|database_path| {
						let _ = std::fs::create_dir_all(database_path);
						sc_sysinfo::gather_hwbench(Some(database_path))
					}))
					.flatten();

				let polkadot_cli = RelayChainCli::new(
					&config,
					[RelayChainCli::executable_name()]
						.iter()
						.chain(cli.relaychain_args.iter()),
				);

				let para_id = chain_spec::Extensions::try_get(&config.chain_spec)
					.map(|e| e.para_id)
					.expect("Could not find parachain ID in chain-spec.");

				let id = ParaId::from(para_id);

				let parachain_account =
					AccountIdConversion::<polkadot_primitives::v5::AccountId>::into_account_truncating(&id);

				let state_version = Cli::runtime_version().state_version();

				let block: Block =
					generate_genesis_block(&*config.chain_spec, state_version).map_err(|e| format!("{e:?}"))?;
				let genesis_state = format!("0x{:?}", HexDisplay::from(&block.header().encode()));

				let task_executor = config.tokio_handle.clone();
				let polkadot_config = SubstrateCli::create_configuration(&polkadot_cli, &polkadot_cli, task_executor)
					.map_err(|err| format!("Relay chain argument error: {err}"))?;

				let collator_options = cli.run.base.collator_options();

				info!("Parachain id: {:?}", para_id);
				info!("Parachain Account: {}", parachain_account);
				info!("Parachain genesis state: {}", genesis_state);
				info!(
					"Is collating: {}",
					if config.role.is_authority() { "yes" } else { "no" }
				);

				crate::service::start_node(config, polkadot_config, cli.ethereum_config, collator_options, id, hwbench)
					.await
					.map(|r| r.0)
					.map_err(Into::into)
			})
		}
	}
}

impl DefaultConfigurationValues for RelayChainCli {
	fn p2p_listen_port() -> u16 {
		30334
	}

	fn rpc_listen_port() -> u16 {
		9945
	}

	fn prometheus_listen_port() -> u16 {
		9616
	}
}

impl CliConfiguration<Self> for RelayChainCli {
	fn shared_params(&self) -> &SharedParams {
		self.base.base.shared_params()
	}

	fn import_params(&self) -> Option<&ImportParams> {
		self.base.base.import_params()
	}

	fn network_params(&self) -> Option<&NetworkParams> {
		self.base.base.network_params()
	}

	fn keystore_params(&self) -> Option<&KeystoreParams> {
		self.base.base.keystore_params()
	}

	fn base_path(&self) -> Result<Option<BasePath>> {
		Ok(self
			.shared_params()
			.base_path()?
			.or_else(|| self.base_path.clone().map(Into::into)))
	}

	fn prometheus_config(
		&self,
		default_listen_port: u16,
		chain_spec: &Box<dyn ChainSpec>,
	) -> Result<Option<PrometheusConfig>> {
		self.base.base.prometheus_config(default_listen_port, chain_spec)
	}

	fn init<F>(
		&self,
		_support_url: &String,
		_impl_version: &String,
		_logger_hook: F,
		_config: &sc_service::Configuration,
	) -> Result<()>
	where
		F: FnOnce(&mut sc_cli::LoggerBuilder, &sc_service::Configuration),
	{
		unreachable!("PolkadotCli is never initialized; qed");
	}

	fn chain_id(&self, is_dev: bool) -> Result<String> {
		let chain_id = self.base.base.chain_id(is_dev)?;

		Ok(if chain_id.is_empty() {
			self.chain_id.clone().unwrap_or_default()
		} else {
			chain_id
		})
	}

	fn role(&self, is_dev: bool) -> Result<sc_service::Role> {
		self.base.base.role(is_dev)
	}

	fn transaction_pool(&self, is_dev: bool) -> Result<sc_service::config::TransactionPoolOptions> {
		self.base.base.transaction_pool(is_dev)
	}

	fn rpc_methods(&self) -> Result<sc_service::config::RpcMethods> {
		self.base.base.rpc_methods()
	}

	fn rpc_cors(&self, is_dev: bool) -> Result<Option<Vec<String>>> {
		self.base.base.rpc_cors(is_dev)
	}

	fn default_heap_pages(&self) -> Result<Option<u64>> {
		self.base.base.default_heap_pages()
	}

	fn force_authoring(&self) -> Result<bool> {
		self.base.base.force_authoring()
	}

	fn disable_grandpa(&self) -> Result<bool> {
		self.base.base.disable_grandpa()
	}

	fn max_runtime_instances(&self) -> Result<Option<usize>> {
		self.base.base.max_runtime_instances()
	}

	fn announce_block(&self) -> Result<bool> {
		self.base.base.announce_block()
	}

	fn telemetry_endpoints(&self, chain_spec: &Box<dyn ChainSpec>) -> Result<Option<sc_telemetry::TelemetryEndpoints>> {
		self.base.base.telemetry_endpoints(chain_spec)
	}
}

impl Cli {
	fn runtime_version() -> &'static RuntimeVersion {
		&hydradx_runtime::VERSION
	}
}
