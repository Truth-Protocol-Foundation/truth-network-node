#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::string::String;

use crate::*;
const SIGNED_SUBMIT_EXTERNAL_PROPOSAL_CONTEXT: &'static [u8] = b"wt_submit_external_proposal";
const SIGNED_SUBMIT_VOTE_CONTEXT: &'static [u8] = b"wt_submit_vote";

impl<T: Config> Pallet<T> {
    // external_ref will ensure signature re-use is not possible but we also add a lifetime (block
    // number) to be on the safe side.
    pub fn encode_signed_submit_external_proposal_params(
        relayer: &T::AccountId,
        proposal: &ProposalRequest,
        block_number: &BlockNumberFor<T>,
    ) -> Vec<u8> {
        (SIGNED_SUBMIT_EXTERNAL_PROPOSAL_CONTEXT, relayer.clone(), proposal, block_number).encode()
    }

    // Voters and only vote once per proposal so no nonce needed here.
    pub fn encode_signed_submit_vote_params(
        relayer: &T::AccountId,
        proposal_id: &ProposalId,
        aye: &bool,
        block_number: &BlockNumberFor<T>,
    ) -> Vec<u8> {
        (SIGNED_SUBMIT_VOTE_CONTEXT, relayer.clone(), proposal_id, aye, block_number).encode()
    }

    pub fn offchain_signature_is_valid<D: Encode>(
        data: &D,
        signer: &T::SignerId,
        signature: &<T::SignerId as RuntimeAppPublic>::Signature,
    ) -> bool {
        let signature_valid =
            data.using_encoded(|encoded_data| signer.verify(&encoded_data, &signature));

        log::trace!(
            "🪲 Validating ocw signature: [ data {:?} - account {:?} - signature {:?} ] Result: {}",
            data.encode(),
            signer.encode(),
            signature,
            signature_valid
        );
        return signature_valid;
    }

    pub fn invoke_finalised_internal_vote(proposal_id: ProposalId, proposal: &Proposal<T>) -> DispatchResult {

         let call = Call::internal_vote {
            proposal_id,
            aye,
            voter: Default::default(),
            signature: Default::default()
        };

        match SubmitTransaction::<T, pallet_watchtower::Call<T>>::submit_unsigned_transaction(call.into()) {
            Ok(()) => (),
            Err(_e) => {
                log::debug!("Error submitting vote from Summary Watchtower OCW for block {:?}", block_number);
            }
        };

        Ok(())
    }
}
