use crate::*;
use scale_info::prelude::boxed::Box;

pub const CREATE_MARKET_AND_DEPLOY_POOL_CONTEXT: &[u8] = b"create_market_and_deploy_pool";
pub const REPORT_OUTCOME_CONTEXT: &[u8] = b"report_market_outcome_context";
pub const WITHDRAW_TOKENS_CONTEXT: &[u8] = b"withdraw_tokens_context";
pub const TRANSFER_TOKENS_CONTEXT: &[u8] = b"transfer_tokens_context";
pub const REDEEM_SHARES: &[u8] = b"redeem_shares_context";
pub const BUY_COMPLETE_SET_CONTEXT: &[u8] = b"buy_complete_set_context";

pub fn encode_signed_redeem_shares_params<T: Config>(
    relayer: &T::AccountId,
    nonce: &u64,
    market_id: &MarketIdOf<T>,
) -> Vec<u8> {
    (REDEEM_SHARES, relayer.clone(), nonce, market_id).encode()
}

pub fn encode_signed_buy_complete_set_params<T: Config>(
    relayer: &T::AccountId,
    nonce: &u64,
    market_id: &MarketIdOf<T>,
    amount: &BalanceOf<T>,
) -> Vec<u8> {
    (BUY_COMPLETE_SET_CONTEXT, relayer.clone(), nonce, market_id, amount).encode()
}

pub fn encode_signed_create_market_and_deploy_pool_params<T: Config>(
    relayer: &T::AccountId,
    nonce: u64,
    base_asset: &AssetOf<T>,
    creator_fee: &Perbill,
    oracle: &T::AccountId,
    period: &MarketPeriodOf<T>,
    deadlines: &DeadlinesOf<T>,
    metadata: &MultiHash,
    market_type: &MarketType,
    dispute_mechanism: &Option<MarketDisputeMechanism>,
    amount: &BalanceOf<T>,
    spot_prices: &Vec<BalanceOf<T>>,
    swap_fee: &BalanceOf<T>,
) -> Vec<u8> {
    (
        CREATE_MARKET_AND_DEPLOY_POOL_CONTEXT,
        relayer.clone(),
        nonce,
        base_asset,
        creator_fee,
        oracle,
        period,
        deadlines,
        metadata,
        market_type,
        dispute_mechanism,
        amount,
        spot_prices,
        swap_fee,
    )
        .encode()
}

pub fn encode_signed_report_params<T: Config>(
    relayer: &T::AccountId,
    nonce: &u64,
    market_id: &MarketIdOf<T>,
    outcome: &OutcomeReport,
) -> Vec<u8> {
    (REPORT_OUTCOME_CONTEXT, relayer.clone(), nonce, market_id, outcome).encode()
}

pub fn encode_signed_withdraw_params<T: Config>(
    relayer: &T::AccountId,
    nonce: &u64,
    token: &EthAddress,
    owner: &T::AccountId,
    amount: &BalanceOf<T>,
) -> Vec<u8> {
    (WITHDRAW_TOKENS_CONTEXT, relayer.clone(), nonce, token, owner, amount).encode()
}

pub fn encode_signed_transfer_params<T: Config>(
    relayer: &T::AccountId,
    nonce: &u64,
    token: &EthAddress,
    from: &T::AccountId,
    to: &T::AccountId,
    amount: &BalanceOf<T>,
) -> Vec<u8> {
    (TRANSFER_TOKENS_CONTEXT, relayer.clone(), nonce, token, from, to, amount).encode()
}

pub fn get_encoded_call_param<T: Config>(
    call: &<T as Config>::RuntimeCall,
) -> Option<(&Proof<T::Signature, T::AccountId>, Vec<u8>)> {
    let call = match call.is_sub_type() {
        Some(call) => call,
        None => return None,
    };

    match call {
        Call::signed_create_market_and_deploy_pool {
            ref proof,
            ref base_asset,
            ref creator_fee,
            ref oracle,
            ref period,
            ref deadlines,
            ref metadata,
            ref market_type,
            ref dispute_mechanism,
            ref amount,
            ref spot_prices,
            ref swap_fee,
        } => {
            let market_nonce = <UserNonces<T>>::get(&proof.signer);
            let encoded_data = encode_signed_create_market_and_deploy_pool_params::<T>(
                &proof.relayer,
                market_nonce,
                &base_asset,
                &creator_fee,
                &oracle,
                &period,
                &deadlines,
                &metadata,
                &market_type,
                &dispute_mechanism,
                &amount,
                &spot_prices,
                &swap_fee,
            );

            Some((proof, encoded_data))
        },
        Call::signed_report { ref proof, ref market_id, ref outcome } => {
            let nonce = <MarketNonces<T>>::get(&proof.signer, &market_id);
            let encoded_data =
                encode_signed_report_params::<T>(&proof.relayer, &nonce, &market_id, &outcome);
            Some((proof, encoded_data))
        },
        Call::signed_withdraw_tokens { ref proof, ref token, ref amount } => {
            let nonce = <UserNonces<T>>::get(&proof.signer);
            let encoded_data = encode_signed_withdraw_params::<T>(
                &proof.relayer,
                &nonce,
                token,
                &proof.signer,
                &amount,
            );
            Some((proof, encoded_data))
        },
        Call::signed_transfer_asset { ref proof, ref token, ref to, ref amount } => {
            let nonce = <UserNonces<T>>::get(&proof.signer);
            let encoded_data = encode_signed_transfer_params::<T>(
                &proof.relayer,
                &nonce,
                &token,
                &proof.signer,
                &to,
                &amount,
            );
            Some((proof, encoded_data))
        },
        Call::signed_redeem_shares { ref proof, ref market_id } => {
            let nonce = MarketNonces::<T>::get(&proof.signer, &market_id);
            let encoded_data =
                encode_signed_redeem_shares_params::<T>(&proof.relayer, &nonce, &market_id);
            Some((proof, encoded_data))
        },
        Call::signed_buy_complete_set { ref proof, ref market_id, ref amount } => {
            let nonce = MarketNonces::<T>::get(&proof.signer, &market_id);
            let encoded_data = encode_signed_buy_complete_set_params::<T>(
                &proof.relayer,
                &nonce,
                &market_id,
                &amount,
            );
            Some((proof, encoded_data))
        },
        _ => None,
    }
}

impl<T: Config> InnerCallValidator for Pallet<T> {
    type Call = <T as Config>::RuntimeCall;

    fn signature_is_valid(call: &Box<Self::Call>) -> bool {
        if let Some((proof, signed_payload)) = get_encoded_call_param::<T>(call) {
            return verify_signature::<T::Signature, T::AccountId>(
                &proof,
                &signed_payload.as_slice(),
            )
            .is_ok();
        }

        return false;
    }
}
