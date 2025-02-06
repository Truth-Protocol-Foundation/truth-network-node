use clap::Parser;
use sc_cli::{
    ChainSpec, CliConfiguration, ImportParams, KeystoreParams, NetworkParams, OffchainWorkerParams,
    Result as CLIResult, Role, RunCmd, SharedParams,
};

use avn_key_subcommand as key;
use sc_service::{config::PrometheusConfig, BasePath, TransactionPoolOptions};
use sc_telemetry::TelemetryEndpoints;
use std::net::SocketAddr;

#[derive(Debug, Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Option<Subcommand>,

    #[clap(flatten)]
    pub run: TnfRunCmd,
}

#[derive(Debug, Parser)]
pub struct TnfRunCmd {
    #[clap(flatten)]
    pub base: RunCmd,

    /// Tnf server port number
    #[arg(long = "tnf-port", value_name = "Tnf PORT")]
    pub tnf_service_port: Option<String>,

    /// URL for connecting with an ethereum node
    #[arg(long = "ethereum-node-url", value_name = "ETH URL")]
    pub eth_node_url: Option<String>,

    /// Flag to specify the Id of the registered node
    #[arg(long = "registered-node-id", value_name = "Registered Node Id")]
    pub registered_node_id: Option<String>,
}

impl std::ops::Deref for TnfRunCmd {
    type Target = RunCmd;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

use sc_cli::DefaultConfigurationValues;

impl<DCV> CliConfiguration<DCV> for TnfRunCmd
where
    RunCmd: CliConfiguration<DCV>,
    DCV: DefaultConfigurationValues,
{
    fn shared_params(&self) -> &SharedParams {
        self.base.shared_params()
    }

    fn import_params(&self) -> Option<&ImportParams> {
        self.base.import_params()
    }

    fn network_params(&self) -> Option<&NetworkParams> {
        self.base.network_params()
    }

    fn keystore_params(&self) -> Option<&KeystoreParams> {
        self.base.keystore_params()
    }

    fn offchain_worker_params(&self) -> Option<&OffchainWorkerParams> {
        self.base.offchain_worker_params()
    }

    fn node_name(&self) -> CLIResult<String> {
        self.base.node_name()
    }

    fn dev_key_seed(&self, is_dev: bool) -> CLIResult<Option<String>> {
        self.base.dev_key_seed(is_dev)
    }

    fn telemetry_endpoints(
        &self,
        chain_spec: &Box<dyn ChainSpec>,
    ) -> CLIResult<Option<TelemetryEndpoints>> {
        self.base.telemetry_endpoints(chain_spec)
    }

    fn role(&self, is_dev: bool) -> CLIResult<Role> {
        self.base.role(is_dev)
    }

    fn force_authoring(&self) -> CLIResult<bool> {
        self.base.force_authoring()
    }

    fn prometheus_config(
        &self,
        default_listen_port: u16,
        chain_spec: &Box<dyn ChainSpec>,
    ) -> CLIResult<Option<PrometheusConfig>> {
        self.base.prometheus_config(default_listen_port, chain_spec)
    }

    fn disable_grandpa(&self) -> CLIResult<bool> {
        self.base.disable_grandpa()
    }

    fn rpc_max_connections(&self) -> CLIResult<u32> {
        self.base.rpc_max_connections()
    }

    fn rpc_cors(&self, is_dev: bool) -> CLIResult<Option<Vec<String>>> {
        self.base.rpc_cors(is_dev)
    }

    fn rpc_addr(&self, default_listen_port: u16) -> CLIResult<Option<SocketAddr>> {
        self.base.rpc_addr(default_listen_port)
    }

    fn rpc_methods(&self) -> CLIResult<sc_service::config::RpcMethods> {
        self.base.rpc_methods()
    }

    fn rpc_max_request_size(&self) -> CLIResult<u32> {
        self.base.rpc_max_request_size()
    }

    fn rpc_max_response_size(&self) -> CLIResult<u32> {
        self.base.rpc_max_response_size()
    }

    fn rpc_max_subscriptions_per_connection(&self) -> CLIResult<u32> {
        self.base.rpc_max_subscriptions_per_connection()
    }

    fn transaction_pool(&self, is_dev: bool) -> CLIResult<TransactionPoolOptions> {
        self.base.transaction_pool(is_dev)
    }

    fn max_runtime_instances(&self) -> CLIResult<Option<usize>> {
        self.base.max_runtime_instances()
    }

    fn runtime_cache_size(&self) -> CLIResult<u8> {
        self.base.runtime_cache_size()
    }

    fn base_path(&self) -> CLIResult<Option<BasePath>> {
        self.base.base_path()
    }
}

#[derive(Debug, clap::Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Subcommand {
    /// Key management cli utilities
    #[command(subcommand)]
    Key(key::AvnKeySubcommand),

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

    /// Sub-commands concerned with benchmarking.
    #[command(subcommand)]
    Benchmark(frame_benchmarking_cli::BenchmarkCmd),

    TryRuntime,

    /// Db meta columns information.
    ChainInfo(sc_cli::ChainInfoCmd),
}
