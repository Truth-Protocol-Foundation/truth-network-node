//! A collection of node-specific RPC methods.
//! Substrate provides the `sc-rpc` crate, which defines the core RPC layer
//! used by Substrate nodes. This file extends those RPC definitions with
//! capabilities that are specific to this project's runtime configuration.

#![warn(missing_docs)]

use std::sync::Arc;

use jsonrpsee::RpcModule;
use sc_client_api::{BlockBackend, UsageProvider};
use sc_transaction_pool_api::TransactionPool;
use sp_api::{offchain::OffchainStorage, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use tnf_node_runtime::{opaque::Block, AccountId, Balance, Nonce};

pub use sc_rpc_api::DenyUnsafe;

/// Full client dependencies.
pub struct FullDeps<C, P, O> {
    /// The client instance to use.
    pub client: Arc<C>,
    /// Transaction pool instance.
    pub pool: Arc<P>,
    /// Whether to deny unsafe calls
    pub deny_unsafe: DenyUnsafe,
    /// Optional off-chain storage for caching
    pub offchain_storage: Option<O>,
}

/// Instantiate all full RPC extensions.
pub fn create_full<C, P, O>(
    deps: FullDeps<C, P, O>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    C: ProvideRuntimeApi<Block>,
    C: BlockBackend<Block>,
    C: UsageProvider<Block>,
    C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError> + 'static,
    C: Send + Sync + 'static,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
    C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
    C::Api: BlockBuilder<Block>,
    P: TransactionPool + 'static,
    O: OffchainStorage + Clone + Send + Sync + 'static,
{
    use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
    use substrate_frame_rpc_system::{System, SystemApiServer};
    use summary_calculation_rpc::{
        SummaryCalculationProvider, SummaryCalculationProviderRpcServer,
    };

    let mut module = RpcModule::new(());
    let FullDeps { client, pool, deny_unsafe, offchain_storage } = deps;

    module.merge(System::new(client.clone(), pool, deny_unsafe).into_rpc())?;
    module.merge(TransactionPayment::new(client.clone()).into_rpc())?;

    module.merge(SummaryCalculationProvider::new(client.clone(), offchain_storage).into_rpc())?;

    Ok(module)
}
