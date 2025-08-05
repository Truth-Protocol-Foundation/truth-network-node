use codec::{Codec, Encode};
use sc_client_api::{client::BlockBackend, HeaderBackend, UsageProvider};
use sp_api::offchain::OffchainStorage;
use sp_runtime::{
    traits::{Block as BlockT, SaturatedConversion},
};
use std::sync::{Arc, Mutex};

use jsonrpsee::{core::RpcResult, proc_macros::rpc};

use node_primitives::AccountId;
use tnf_service::{
    extrinsic_utils::{self},
    merkle_tree_utils::*,
    summary_utils::EncodedLeafData,
};

#[rpc(server)]
pub trait SummaryCalculationProviderRpc {
    #[method(name = "summary_calculation", blocking)]
    fn get_summary_calculation(&self, from_block: u32, to_block: u32) -> RpcResult<String>;
}

const CACHE_PREFIX: &[u8] = b"tnf_summary_cache::v1::";

pub struct SummaryCalculationProvider<C, Block, O = ()> {
    client: Arc<C>,
    offchain_storage: Option<Arc<Mutex<O>>>,
    _marker: std::marker::PhantomData<Block>,
}

impl<C, Block, O> SummaryCalculationProvider<C, Block, O>
where
    O: OffchainStorage,
{
    pub fn new(client: Arc<C>, offchain_storage: Option<O>) -> Self {
        let wrapped_storage = offchain_storage.map(|storage| Arc::new(Mutex::new(storage)));
        Self { client, offchain_storage: wrapped_storage, _marker: Default::default() }
    }

    fn get_cached_summary(&self, from_block: u32, to_block: u32) -> Option<[u8; 32]> {
        if let Some(ref storage_mutex) = self.offchain_storage {
            if let Ok(storage) = storage_mutex.lock() {
                let key = (from_block, to_block).encode();
                storage.get(CACHE_PREFIX, &key).and_then(|data| {
                    if data.len() == 32 {
                        let mut array = [0u8; 32];
                        array.copy_from_slice(&data);
                        Some(array)
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    
    fn set_cached_summary(&self, from_block: u32, to_block: u32, root: [u8; 32]) {
        if let Some(ref storage_mutex) = self.offchain_storage {
            if let Ok(mut storage) = storage_mutex.lock() {
                let key = (from_block, to_block).encode();
                storage.set(CACHE_PREFIX, &key, &root.to_vec());
            }
        }
    }


}

impl<C, Block> SummaryCalculationProvider<C, Block, ()> {
    pub fn new_without_storage(client: Arc<C>) -> Self {
        Self { client, offchain_storage: None, _marker: Default::default() }
    }
}

impl<C, Block, O> SummaryCalculationProviderRpcServer for SummaryCalculationProvider<C, Block, O>
where
    Block: BlockT,
    C: Send + Sync + 'static + BlockBackend<Block> + UsageProvider<Block> + HeaderBackend<Block>,
    O: OffchainStorage + 'static,
    AccountId: Clone + std::fmt::Display + Codec,
{
    fn get_summary_calculation(&self, from_block: u32, to_block: u32) -> RpcResult<String> {
        let finalized_block_number: u32 = self.client.info().finalized_number.saturated_into();

        if to_block <= finalized_block_number {
            if let Some(cached_root) = self.get_cached_summary(from_block, to_block) {
                return Ok(hex::encode(cached_root));
            }
        }

        let extrinsics =
            fetch_extrinsics_from_client::<Block, C>(&self.client, from_block, to_block).map_err(
                |e| jsonrpsee::core::Error::Custom(format!("Error fetching extrinsics: {:?}", e)),
            )?;

        let (result, root_bytes) = if !extrinsics.is_empty() {
            let root_hash = generate_tree_root(extrinsics).map_err(|e| {
                jsonrpsee::core::Error::Custom(format!("Error generating tree root: {:?}", e))
            })?;

            let root_bytes = root_hash.0; // Extract [u8; 32] from H256
            (hex::encode(root_bytes), root_bytes)
        } else {
            let empty_root = [0; 32];
            (hex::encode(empty_root), empty_root)
        };

        if to_block <= finalized_block_number {
            self.set_cached_summary(from_block, to_block, root_bytes);
        }

        Ok(result)
    }
}

pub fn fetch_extrinsics_from_client<Block, C>(
    client: &Arc<C>,
    from_block_number: u32,
    to_block_number: u32,
) -> RpcResult<Vec<EncodedLeafData>>
where
    Block: BlockT,
    C: Send + Sync + 'static + BlockBackend<Block> + UsageProvider<Block>,
{
    let mut abi_encoded_leaves: Vec<Vec<u8>> = vec![];

    for block_number in from_block_number..=to_block_number {
        let (_, mut extrinsics) =
            extrinsic_utils::process_extrinsics_in_block_and_check_if_filter_target_exists(
                &client,
                block_number,
                None,
            )
            .map_err(|e| {
                jsonrpsee::core::Error::Custom(format!("Error getting extrinsics data: {:?}", e))
            })?;
        abi_encoded_leaves.append(&mut extrinsics);
    }

    Ok(abi_encoded_leaves)
}
