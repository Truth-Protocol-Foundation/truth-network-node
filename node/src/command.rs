use crate::{
    benchmarking::{inherent_benchmark_data, RemarkBuilder, TransferKeepAliveBuilder},
    chain_spec,
    cli::{Cli, Subcommand},
    service,
    tnf_config::TnfCliConfiguration,
};
use common_primitives::constants::TNF_CHAIN_PREFIX;
use frame_benchmarking_cli::{BenchmarkCmd, ExtrinsicFactory, SUBSTRATE_REFERENCE_HARDWARE};
use sc_cli::SubstrateCli;
use sc_service::PartialComponents;
use sp_core::crypto::{self};
use sp_keyring::Sr25519Keyring;
use tnf_node_runtime::{Block, NATIVE_EXISTENTIAL_DEPOSIT};

impl SubstrateCli for Cli {
    fn impl_name() -> String {
        "Tnf Node".into()
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
        "support.anonymous.an".into()
    }

    fn copyright_start_year() -> i32 {
        2024
    }

    fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
        Ok(match id {
            "dev" => Box::new(chain_spec::development_config()?),
            "dev-testnet" => Box::new(chain_spec::dev_testnet_config()?),
            "public-testnet" => Box::new(chain_spec::public_testnet_config()?),
            "mainnet" => Box::new(chain_spec::mainnet_config()?),
            "" | "local" => Box::new(chain_spec::local_testnet_config()?),
            path =>
                Box::new(chain_spec::ChainSpec::from_json_file(std::path::PathBuf::from(path))?),
        })
    }
}

/// Parse and run command line arguments
pub fn run() -> sc_cli::Result<()> {
    let cli = Cli::from_args();

    match &cli.subcommand {
        Some(Subcommand::Key(cmd)) => cmd.run(&cli),
        Some(Subcommand::BuildSpec(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            crypto::set_default_ss58_version(TNF_CHAIN_PREFIX.into());
            runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
        },
        Some(Subcommand::CheckBlock(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            crypto::set_default_ss58_version(TNF_CHAIN_PREFIX.into());

            runner.async_run(|config| {
                let PartialComponents { client, task_manager, import_queue, .. } =
                    service::new_partial(&config)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        },
        Some(Subcommand::ExportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            crypto::set_default_ss58_version(TNF_CHAIN_PREFIX.into());

            runner.async_run(|config| {
                let PartialComponents { client, task_manager, .. } = service::new_partial(&config)?;
                Ok((cmd.run(client, config.database), task_manager))
            })
        },
        Some(Subcommand::ExportState(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            crypto::set_default_ss58_version(TNF_CHAIN_PREFIX.into());

            runner.async_run(|config| {
                let PartialComponents { client, task_manager, .. } = service::new_partial(&config)?;
                Ok((cmd.run(client, config.chain_spec), task_manager))
            })
        },
        Some(Subcommand::ImportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            crypto::set_default_ss58_version(TNF_CHAIN_PREFIX.into());

            runner.async_run(|config| {
                let PartialComponents { client, task_manager, import_queue, .. } =
                    service::new_partial(&config)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        },
        Some(Subcommand::PurgeChain(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            crypto::set_default_ss58_version(TNF_CHAIN_PREFIX.into());

            runner.sync_run(|config| cmd.run(config.database))
        },
        Some(Subcommand::Revert(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            crypto::set_default_ss58_version(TNF_CHAIN_PREFIX.into());

            runner.async_run(|config| {
                let PartialComponents { client, task_manager, backend, .. } =
                    service::new_partial(&config)?;
                let aux_revert = Box::new(|client, _, blocks| {
                    sc_consensus_grandpa::revert(client, blocks)?;
                    Ok(())
                });
                Ok((cmd.run(client, backend, Some(aux_revert)), task_manager))
            })
        },
        Some(Subcommand::Benchmark(cmd)) => {
            let runner = cli.create_runner(cmd)?;

            runner.sync_run(|config| {
                // This switch needs to be in the client, since the client decides
                // which sub-commands it wants to support.
                match cmd {
                    BenchmarkCmd::Pallet(cmd) => {
                        crypto::set_default_ss58_version(TNF_CHAIN_PREFIX.into());

                        if !cfg!(feature = "runtime-benchmarks") {
                            return Err(
                                "Runtime benchmarking wasn't enabled when building the node. \
							You can enable it with `--features runtime-benchmarks`."
                                    .into(),
                            );
                        }

                        cmd.run::<Block, ()>(config)
                    },
                    BenchmarkCmd::Block(cmd) => {
                        let PartialComponents { client, .. } = service::new_partial(&config)?;
                        cmd.run(client)
                    },
                    #[cfg(not(feature = "runtime-benchmarks"))]
                    BenchmarkCmd::Storage(_) => Err(
                        "Storage benchmarking can be enabled with `--features runtime-benchmarks`."
                            .into(),
                    ),
                    #[cfg(feature = "runtime-benchmarks")]
                    BenchmarkCmd::Storage(cmd) => {
                        let PartialComponents { client, backend, .. } =
                            service::new_partial(&config)?;
                        let db = backend.expose_db();
                        let storage = backend.expose_storage();

                        cmd.run(config, client, db, storage)
                    },
                    BenchmarkCmd::Overhead(cmd) => {
                        let PartialComponents { client, .. } = service::new_partial(&config)?;
                        let ext_builder = RemarkBuilder::new(client.clone());

                        cmd.run(
                            config,
                            client,
                            inherent_benchmark_data()?,
                            Vec::new(),
                            &ext_builder,
                        )
                    },
                    BenchmarkCmd::Extrinsic(cmd) => {
                        let PartialComponents { client, .. } = service::new_partial(&config)?;
                        // Register the *Remark* and *TKA* builders.
                        let ext_factory = ExtrinsicFactory(vec![
                            Box::new(RemarkBuilder::new(client.clone())),
                            Box::new(TransferKeepAliveBuilder::new(
                                client.clone(),
                                Sr25519Keyring::Alice.to_account_id(),
                                NATIVE_EXISTENTIAL_DEPOSIT,
                            )),
                        ]);

                        cmd.run(client, inherent_benchmark_data()?, Vec::new(), &ext_factory)
                    },
                    BenchmarkCmd::Machine(cmd) =>
                        cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone()),
                }
            })
        },
        Some(Subcommand::TryRuntime) => Err("The `try-runtime` subcommand has been migrated to a standalone CLI (https://github.com/paritytech/try-runtime-cli). It is no longer being maintained here and will be removed entirely some time after January 2024. Please remove this subcommand from your runtime and use the standalone CLI.".into()),
        Some(Subcommand::ChainInfo(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.sync_run(|config| cmd.run::<Block>(&config))
        },
        None => {
            let runner = cli.create_runner(&cli.run)?;
            let tnf_config = TnfCliConfiguration {
                tnf_service_port: cli.run.tnf_service_port,
                ethereum_node_url: cli.run.eth_node_url,
                registered_node_id: cli.run.registered_node_id,
            };
            runner.run_node_until_exit(|config| async move {
                crypto::set_default_ss58_version(TNF_CHAIN_PREFIX.into());
                service::new_full(config, tnf_config).map_err(sc_cli::Error::Service)
            })
        },
    }
}
