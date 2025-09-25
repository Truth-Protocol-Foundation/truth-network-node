use crate::*;
use frame_system::offchain::SubmitTransaction;
use sp_runtime::offchain::storage::{MutateStorageError, StorageRetrievalError, StorageValueRef};

const BLOCK_INCLUSION_PERIOD: u32 = 5;
const SIGNED_SUBMIT_EXTERNAL_PROPOSAL_CONTEXT: &'static [u8] = b"wt_submit_external_proposal";
const SIGNED_SUBMIT_VOTE_CONTEXT: &'static [u8] = b"wt_submit_vote";
const OC_DB_PREFIX: &[u8] = b"wt_ocw_db";

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

    pub fn invoke_finalise_internal_vote(
        proposal_id: ProposalId,
        watchtower: T::AccountId,
        signing_key: T::SignerId,
        block_number: BlockNumberFor<T>,
    ) {
        if let Some(signature) = signing_key
            .sign(&(WATCHTOWER_FINALISE_PROPOSAL_CONTEXT, proposal_id, &watchtower).encode())
        {
            let call = Call::unsigned_finalise_proposal {
                proposal_id,
                watchtower: watchtower.clone(),
                signature,
            };

            match SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()) {
                Ok(()) => (),
                Err(_e) => {
                    log::error!(
                        "Error submitting finalise_internal_vote from Watchtower OCW for block {:?}",
                        block_number
                    );
                },
            };
        } else {
            log::error!(
                "Error signing finalise_internal_vote from Watchtower OCW for block {:?}",
                block_number
            );
        }

        // Add a lock to the ocw so it does not try to do this again for a few blocks
        if let Err(_) =
            Self::record_finalise_internal_vote_submission(block_number, proposal_id, watchtower)
        {
            log::error!("Error getting a lock on OCW DB for block {:?}", block_number);
        }
    }

    pub fn record_finalise_internal_vote_submission(
        block_number: BlockNumberFor<T>,
        proposal_id: ProposalId,
        watchtower: T::AccountId,
    ) -> Result<(), Error<T>> {
        let mut key = OC_DB_PREFIX.to_vec();
        key.extend((proposal_id, watchtower).encode());

        let storage = StorageValueRef::persistent(&key);
        let result = storage
            .mutate(|_: Result<Option<BlockNumberFor<T>>, StorageRetrievalError>| Ok(block_number));
        match result {
            Err(MutateStorageError::ValueFunctionFailed(e)) => Err(e),
            Err(MutateStorageError::ConcurrentModification(_)) =>
                Err(Error::<T>::FailedToAcquireOcwDbLock),
            Ok(_) => return Ok(()),
        }
    }

    pub fn finalise_internal_vote_submission_in_progress(
        proposal_id: ProposalId,
        watchtower: T::AccountId,
        block_number: BlockNumberFor<T>,
    ) -> bool {
        let mut key = OC_DB_PREFIX.to_vec();
        key.extend((proposal_id, watchtower).encode());

        match StorageValueRef::persistent(&key).get::<BlockNumberFor<T>>().ok().flatten() {
            Some(last_submission) => {
                // Allow BLOCK_INCLUSION_PERIOD blocks for the transaction to be included
                return block_number <=
                    last_submission
                        .saturating_add(BlockNumberFor::<T>::from(BLOCK_INCLUSION_PERIOD));
            },
            _ => false,
        }
    }
}
