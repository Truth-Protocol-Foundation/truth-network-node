use super::{
    AccountId, Box, Decode, Encode, InnerCallValidator, Proof, ProvableProxy, Runtime, RuntimeCall,
    RuntimeDebug, Signature, TypeInfo,
};

// Avn proxy configuration logic
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct AvnProxyConfig {}
impl Default for AvnProxyConfig {
    fn default() -> Self {
        AvnProxyConfig {}
    }
}

impl ProvableProxy<RuntimeCall, Signature, AccountId> for AvnProxyConfig {
    fn get_proof(call: &RuntimeCall) -> Option<Proof<Signature, AccountId>> {
        match call {
            RuntimeCall::EthereumEvents(
                pallet_ethereum_events::Call::signed_add_ethereum_log {
                    proof,
                    event_type: _,
                    tx_hash: _,
                },
            ) => return Some(proof.clone()),
            RuntimeCall::TokenManager(pallet_token_manager::pallet::Call::signed_transfer {
                proof,
                from: _,
                to: _,
                token_id: _,
                amount: _,
            }) => return Some(proof.clone()),
            RuntimeCall::TokenManager(
                pallet_token_manager::pallet::Call::schedule_signed_lower {
                    proof,
                    from: _,
                    token_id: _,
                    amount: _,
                    t1_recipient: _,
                },
            ) => return Some(proof.clone()),
            RuntimeCall::NftManager(pallet_nft_manager::Call::signed_mint_single_nft {
                proof,
                unique_external_ref: _,
                royalties: _,
                t1_authority: _,
            }) => return Some(proof.clone()),
            RuntimeCall::NftManager(pallet_nft_manager::Call::signed_list_nft_open_for_sale {
                proof,
                nft_id: _,
                market: _,
            }) => return Some(proof.clone()),
            RuntimeCall::NftManager(pallet_nft_manager::Call::signed_transfer_fiat_nft {
                proof,
                nft_id: _,
                t2_transfer_to_public_key: _,
            }) => return Some(proof.clone()),
            RuntimeCall::NftManager(pallet_nft_manager::Call::signed_cancel_list_fiat_nft {
                proof,
                nft_id: _,
            }) => return Some(proof.clone()),
            RuntimeCall::NftManager(pallet_nft_manager::Call::signed_create_batch {
                proof,
                total_supply: _,
                royalties: _,
                t1_authority: _,
            }) => return Some(proof.clone()),
            RuntimeCall::NftManager(pallet_nft_manager::Call::signed_mint_batch_nft {
                proof,
                batch_id: _,
                index: _,
                owner: _,
                unique_external_ref: _,
            }) => return Some(proof.clone()),
            RuntimeCall::NftManager(pallet_nft_manager::Call::signed_list_batch_for_sale {
                proof,
                batch_id: _,
                market: _,
            }) => return Some(proof.clone()),
            RuntimeCall::NftManager(pallet_nft_manager::Call::signed_end_batch_sale {
                proof,
                batch_id: _,
            }) => return Some(proof.clone()),
            RuntimeCall::PredictionMarkets(
                pallet_prediction_markets::Call::signed_create_market_and_deploy_pool {
                    proof,
                    base_asset: _,
                    creator_fee: _,
                    oracle: _,
                    period: _,
                    deadlines: _,
                    metadata: _,
                    market_type: _,
                    dispute_mechanism: _,
                    amount: _,
                    spot_prices: _,
                    swap_fee: _,
                },
            ) => return Some(proof.clone()),
            RuntimeCall::PredictionMarkets(pallet_prediction_markets::Call::signed_report {
                proof,
                market_id: _,
                outcome: _,
            }) => return Some(proof.clone()),
            RuntimeCall::PredictionMarkets(
                pallet_prediction_markets::Call::signed_transfer_asset {
                    proof,
                    token: _,
                    to: _,
                    amount: _,
                },
            ) => return Some(proof.clone()),
            RuntimeCall::PredictionMarkets(
                pallet_prediction_markets::Call::signed_redeem_shares { proof, market_id: _ },
            ) => return Some(proof.clone()),
            RuntimeCall::PredictionMarkets(
                pallet_prediction_markets::Call::signed_withdraw_tokens {
                    proof,
                    token: _,
                    amount: _,
                },
            ) => return Some(proof.clone()),
            RuntimeCall::PredictionMarkets(
                pallet_prediction_markets::Call::signed_buy_complete_set {
                    proof,
                    market_id: _,
                    amount: _,
                },
            ) => return Some(proof.clone()),
            RuntimeCall::HybridRouter(pallet_pm_hybrid_router::Call::signed_buy {
                proof,
                market_id: _,
                asset_count: _,
                asset: _,
                amount_in: _,
                max_price: _,
                orders: _,
                strategy: _,
            }) => return Some(proof.clone()),
            RuntimeCall::HybridRouter(pallet_pm_hybrid_router::Call::signed_sell {
                proof,
                market_id: _,
                asset_count: _,
                asset: _,
                amount_in: _,
                min_price: _,
                orders: _,
                strategy: _,
            }) => return Some(proof.clone()),
            RuntimeCall::NodeManager(pallet_node_manager::Call::signed_register_node {
                proof,
                node: _,
                owner: _,
                signing_key: _,
                block_number: _,
            }) => return Some(proof.clone()),
            RuntimeCall::NeoSwaps(pallet_pm_neo_swaps::Call::signed_join {
                proof,
                market_id: _,
                pool_shares_amount: _,
                max_amounts_in: _,
                block_number: _,
            }) => return Some(proof.clone()),
            RuntimeCall::NeoSwaps(pallet_pm_neo_swaps::Call::signed_exit {
                proof,
                market_id: _,
                pool_shares_amount_out: _,
                min_amounts_out: _,
                block_number: _,
            }) => return Some(proof.clone()),
            RuntimeCall::NeoSwaps(pallet_pm_neo_swaps::Call::signed_withdraw_fees {
                proof,
                market_id: _,
                block_number: _,
            }) => return Some(proof.clone()),
            RuntimeCall::NodeManager(pallet_node_manager::Call::signed_deregister_nodes {
                proof,
                owner: _,
                nodes_to_deregister: _,
                block_number: _,
            }) => return Some(proof.clone()),
            _ => None,
        }
    }
}

impl InnerCallValidator for AvnProxyConfig {
    type Call = RuntimeCall;

    fn signature_is_valid(call: &Box<Self::Call>) -> bool {
        match **call {
            RuntimeCall::EthereumEvents(..) =>
                return pallet_ethereum_events::Pallet::<Runtime>::signature_is_valid(call),
            RuntimeCall::TokenManager(..) =>
                return pallet_token_manager::Pallet::<Runtime>::signature_is_valid(call),
            RuntimeCall::NftManager(..) =>
                return pallet_nft_manager::Pallet::<Runtime>::signature_is_valid(call),
            RuntimeCall::PredictionMarkets(..) =>
                return pallet_prediction_markets::Pallet::<Runtime>::signature_is_valid(call),
            RuntimeCall::HybridRouter(..) =>
                return pallet_pm_hybrid_router::Pallet::<Runtime>::signature_is_valid(call),
            RuntimeCall::NodeManager(..) =>
                return pallet_node_manager::Pallet::<Runtime>::signature_is_valid(call),
            RuntimeCall::NeoSwaps(..) =>
                return pallet_pm_neo_swaps::Pallet::<Runtime>::signature_is_valid(call),
            _ => false,
        }
    }
}
