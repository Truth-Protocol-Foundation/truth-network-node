use codec::Codec;
use sc_client_api::{client::BlockBackend, UsageProvider};
use sp_runtime::traits::Block as BlockT;
use std::{sync::Arc, time::Instant};

use jsonrpsee::{
    core::{
        RpcResult
    },
    proc_macros::rpc,
};

use tnf_service::{extrinsic_utils::{self}, merkle_tree_utils::*, summary_utils::EncodedLeafData};
use node_primitives::AccountId;

#[rpc(server)]
pub trait SummaryCalculationProviderRpc{
    #[method(name = "summary_calculation", blocking)]
    fn get_summary_calculation(
        &self,
        from_block: u32,
        to_block:u32,
    ) -> RpcResult<String>;
}

pub struct SummaryCalculationProvider<C, Block> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<Block>,
}

impl<C, Block> SummaryCalculationProvider<C, Block> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl <C, Block> SummaryCalculationProviderRpcServer for SummaryCalculationProvider<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static + BlockBackend<Block> + UsageProvider<Block>,
    AccountId: Clone + std::fmt::Display + Codec,
{
    fn get_summary_calculation(&self, from_block: u32, to_block: u32) -> RpcResult<String> {
        let extrinsics_start_time = Instant::now();
        
        let extrinsics = fetch_extrinsics_from_client::<Block, C>(&self.client, from_block, to_block).unwrap();

        let extrinsics_duration = extrinsics_start_time.elapsed();
            log::info!(
                "⏲️  get_extrinsics on block range [{:?}, {:?}] time: {:?}",
                from_block, to_block,
                extrinsics_duration
            );

            if !extrinsics.is_empty() {
                let root_hash_start_time = Instant::now();
                let root_hash = generate_tree_root(extrinsics).unwrap();
                let root_hash_duration = root_hash_start_time.elapsed();
                log::info!(
                    "⏲️  generate_tree_root on block range [{:?}, {:?}] time: {:?}",
                    from_block, to_block,
                    root_hash_duration
                );

                return Ok(hex::encode(root_hash))
            }

        return Ok(hex::encode([0; 32]));
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
            .map_err(|e| format!("Error getting extrinsics data: {:?}", e)).unwrap();
        abi_encoded_leaves.append(&mut extrinsics);
    }

    Ok(abi_encoded_leaves)
}