#![cfg_attr(not(feature = "std"), no_std)]
use frame_support::{
    dispatch::DispatchResult,
    pallet_prelude::*,
    traits::{IsSubType, IsType},
};
use frame_system::{
    ensure_none,
    offchain::{SendTransactionTypes, SubmitTransaction},
    pallet_prelude::*,
};

use sp_runtime::RuntimeAppPublic;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
};

use hex;
use log;

use pallet_avn::{self as avn};
use sp_avn_common::{
    RootId, SummarySourceInstance as SummarySource, VoteStatusNotifier, VotingStatus,
};

use sp_core::H256;
use sp_runtime::{
    traits::{Dispatchable, ValidateUnsigned},
    transaction_validity::{
        InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity,
        ValidTransaction,
    },
};
use sp_std::prelude::*;

pub mod default_weights;

#[cfg(test)]
pub mod mock;

#[cfg(test)]
mod test;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub use crate::default_weights::WeightInfo;

pub const OCW_LOCK_PREFIX: &[u8] = b"pallet-watchtower::lock::";
pub const OCW_LOCK_TIMEOUT_MS: u64 = 10000;
pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);
pub const WATCHTOWER_OCW_CONTEXT: &[u8] = b"watchtower_ocw_vote";
pub const WATCHTOWER_VOTE_PROVIDE_TAG: &[u8] = b"WatchtowerVoteProvideTag";
pub const DEFAULT_VOTING_PERIOD_BLOCKS: u32 = 100;

pub type AVN<T> = avn::Pallet<T>;

pub type WatchtowerOnChainHash = H256;

pub trait NodeManagerInterface<AccountId, SignerId> {
    fn is_authorized_watchtower(who: &AccountId) -> bool;

    fn get_node_signing_key(node: &AccountId) -> Option<SignerId>;

    fn get_node_from_local_signing_keys() -> Option<(AccountId, SignerId)>;

    /// Get the count of authorized watchtowers without fetching the full list
    fn get_authorized_watchtowers_count() -> u32;
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        SendTransactionTypes<Call<Self>> + frame_system::Config + pallet_avn::Config
    {
        type RuntimeEvent: From<Event<Self>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>
            + Clone
            + Eq
            + PartialEq
            + core::fmt::Debug;

        type RuntimeCall: Parameter
            + Dispatchable<RuntimeOrigin = <Self as frame_system::Config>::RuntimeOrigin>
            + IsSubType<Call<Self>>
            + From<Call<Self>>;

        type WeightInfo: WeightInfo;

        type SignerId: Member + Parameter + sp_runtime::RuntimeAppPublic + Ord + MaxEncodedLen;

        type VoteStatusNotifier: VoteStatusNotifier<BlockNumberFor<Self>>;
        type NodeManager: NodeManagerInterface<Self::AccountId, Self::SignerId>;

        /// Minimum allowed voting period in blocks
        #[pallet::constant]
        type MinVotingPeriod: Get<BlockNumberFor<Self>>;
    }

    #[pallet::storage]
    #[pallet::getter(fn vote_counters)]
    pub type VoteCounters<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        SummarySource,
        Blake2_128Concat,
        RootId<BlockNumberFor<T>>,
        (u32, u32), // (yes_votes, no_votes)
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn voter_history)]
    pub type VoterHistory<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        (SummarySource, RootId<BlockNumberFor<T>>),
        Blake2_128Concat,
        T::AccountId,
        (),
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn consensus_reached_flag)]
    pub type VoteConsensusReached<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        (SummarySource, RootId<BlockNumberFor<T>>),
        bool,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn voting_start_block)]
    pub type VotingStartBlock<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        (SummarySource, RootId<BlockNumberFor<T>>),
        BlockNumberFor<T>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn voting_period)]
    pub type VotingPeriod<T: Config> =
        StorageValue<_, BlockNumberFor<T>, ValueQuery, DefaultVotingPeriod<T>>;

    #[pallet::storage]
    #[pallet::getter(fn pending_validation_root_hash)]
    pub type PendingValidationRootHash<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        (SummarySource, RootId<BlockNumberFor<T>>),
        WatchtowerOnChainHash,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn consensus_threshold)]
    pub type ConsensusThreshold<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        (SummarySource, RootId<BlockNumberFor<T>>),
        u32,
        OptionQuery,
    >;

    #[pallet::type_value]
    pub fn DefaultVotingPeriod<T: Config>() -> BlockNumberFor<T> {
        DEFAULT_VOTING_PERIOD_BLOCKS.into()
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        WatchtowerVoteSubmitted {
            voter: T::AccountId,
            summary_instance: SummarySource,
            root_id: RootId<BlockNumberFor<T>>,
            vote_is_valid: bool,
        },
        WatchtowerConsensusReached {
            summary_instance: SummarySource,
            root_id: RootId<BlockNumberFor<T>>,
            consensus_result: VotingStatus,
        },
        VotingPeriodUpdated {
            old_period: BlockNumberFor<T>,
            new_period: BlockNumberFor<T>,
        },
        ExpiredVotingSessionCleaned {
            summary_instance: SummarySource,
            root_id: RootId<BlockNumberFor<T>>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Summary update operation failed to complete successfully.
        SummaryUpdateFailed,
        /// The verification submission provided is invalid or malformed.
        InvalidVerificationSubmission,
        /// The caller is not an authorized watchtower and cannot perform this operation.
        NotAuthorizedWatchtower,
        /// The watchtower has already voted and cannot vote again.
        AlreadyVoted,
        /// Consensus has already been reached for this verification, no more votes needed.
        ConsensusAlreadyReached,
        /// The voting period has expired and no more votes can be submitted.
        VotingPeriodExpired,
        /// Voting has not started yet for this verification.
        VotingNotStarted,
        /// The specified voting period configuration is invalid.
        InvalidVotingPeriod,
        /// The cleanup operation failed or was not needed.
        CleanupFailed,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(block_number: BlockNumberFor<T>) {
            log::debug!(target: "runtime::watchtower::ocw", "Watchtower OCW running for block {:?}", block_number);

            let maybe_node_info = Self::get_node_from_signing_key();
            let (node, signing_key) = match maybe_node_info {
                Some(node_info) => node_info,
                None => {
                    return;
                },
            };

            Self::process_pending_validations(node, signing_key);
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::vote())]
        pub fn vote(
            origin: OriginFor<T>,
            summary_instance: SummarySource,
            root_id: RootId<BlockNumberFor<T>>,
            vote_is_valid: bool,
        ) -> DispatchResult {
            let voter = ensure_signed(origin)?;

            ensure!(
                T::NodeManager::is_authorized_watchtower(&voter),
                Error::<T>::NotAuthorizedWatchtower
            );

            Self::internal_submit_vote(voter, summary_instance, root_id, vote_is_valid)
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::ocw_vote())]
        pub fn ocw_vote(
            origin: OriginFor<T>,
            node: T::AccountId,
            _signing_key: T::SignerId,
            summary_instance: SummarySource,
            root_id: RootId<BlockNumberFor<T>>,
            vote_is_valid: bool,
            _signature: <T::SignerId as sp_runtime::RuntimeAppPublic>::Signature,
        ) -> DispatchResult {
            ensure_none(origin)?;

            ensure!(T::NodeManager::is_authorized_watchtower(&node), {
                Error::<T>::NotAuthorizedWatchtower
            });

            Self::internal_submit_vote(
                node.clone(),
                summary_instance,
                root_id.clone(),
                vote_is_valid,
            )
        }

        #[pallet::call_index(2)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::set_voting_period())]
        pub fn set_voting_period(
            origin: OriginFor<T>,
            new_period: BlockNumberFor<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let min_period = T::MinVotingPeriod::get();
            ensure!(new_period >= min_period, Error::<T>::InvalidVotingPeriod);

            let old_period = VotingPeriod::<T>::get();
            VotingPeriod::<T>::put(new_period);

            Self::deposit_event(Event::VotingPeriodUpdated { old_period, new_period });

            Ok(())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            if let Call::ocw_vote {
                node,
                signing_key,
                summary_instance,
                root_id,
                vote_is_valid,
                signature,
            } = call
            {
                if !T::NodeManager::is_authorized_watchtower(node) {
                    return InvalidTransaction::Call.into();
                }

                if Self::offchain_signature_is_valid(
                    &(WATCHTOWER_OCW_CONTEXT, summary_instance, root_id, vote_is_valid),
                    signing_key,
                    signature,
                ) {
                    let current_block = frame_system::Pallet::<T>::block_number();
                    let unique_payload_for_provides = (
                        WATCHTOWER_VOTE_PROVIDE_TAG,
                        node.clone(),
                        *summary_instance,
                        root_id.clone(),
                        *vote_is_valid,
                        current_block,
                        source,
                        signature.encode()[0..8].to_vec(),
                    );

                    let provides_tag = unique_payload_for_provides.encode();

                    ValidTransaction::with_tag_prefix("WatchtowerOCW")
                        .priority(TransactionPriority::MAX)
                        .and_provides(vec![provides_tag])
                        .longevity(64_u64)
                        .propagate(true)
                        .build()
                } else {
                    InvalidTransaction::BadSigner.into()
                }
            } else {
                InvalidTransaction::Call.into()
            }
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn notify_summary_ready_for_validation(
            instance: SummarySource,
            root_id: RootId<BlockNumberFor<T>>,
            root_hash: WatchtowerOnChainHash,
        ) -> DispatchResult {
            let consensus_key = (instance, root_id.clone());

            if VoteConsensusReached::<T>::get(&consensus_key) {
                return Ok(());
            }

            if VotingStartBlock::<T>::get(&consensus_key).is_some() {
                return Ok(());
            }

            // Reject zero/empty root hashes as they provide no meaningful validation target
            if root_hash == sp_core::H256::zero() {
                return Err(Error::<T>::InvalidVerificationSubmission.into());
            }

            // Calculate and store consensus threshold once when voting starts
            let total_authorized_watchtowers = T::NodeManager::get_authorized_watchtowers_count();
            // Fixed threshold calculation: (n * 2 + 2) / 3 for proper 2/3 majority
            let required_for_consensus = (total_authorized_watchtowers * 2) / 3;

            VotingStartBlock::<T>::insert(
                &consensus_key,
                frame_system::Pallet::<T>::block_number(),
            );
            PendingValidationRootHash::<T>::insert(&consensus_key, root_hash);
            ConsensusThreshold::<T>::insert(&consensus_key, required_for_consensus);

            Ok(())
        }

        fn process_pending_validations(node: T::AccountId, signing_key: T::SignerId) {
            let current_block = frame_system::Pallet::<T>::block_number();
            let voting_period = VotingPeriod::<T>::get();

            for (consensus_key, start_block) in VotingStartBlock::<T>::iter() {
                let (instance, root_id) = consensus_key.clone();

                // Skip expired sessions - they'll be cleaned up by on_idle or lazy cleanup
                if current_block > start_block + voting_period {
                    continue;
                }

                // Only process active voting sessions for validation
                if let Some(root_hash) = PendingValidationRootHash::<T>::get(&consensus_key) {
                    Self::perform_ocw_recalculation(
                        node.clone(),
                        signing_key.clone(),
                        instance,
                        root_id,
                        root_hash,
                    );
                }
            }
        }

        fn internal_cleanup_expired_votes(
            instance: SummarySource,
            root_id: RootId<BlockNumberFor<T>>,
        ) -> DispatchResult {
            let consensus_key = (instance, root_id.clone());
            let current_block = frame_system::Pallet::<T>::block_number();

            if let Some(start_block) = VotingStartBlock::<T>::get(&consensus_key) {
                let voting_deadline = start_block + VotingPeriod::<T>::get();
                if current_block > voting_deadline {
                    // Clean up expired voting session
                    VoteCounters::<T>::remove(instance, &root_id);
                    let _ = VoterHistory::<T>::clear_prefix(&consensus_key, u32::MAX, None);
                    VotingStartBlock::<T>::remove(&consensus_key);
                    PendingValidationRootHash::<T>::remove(&consensus_key);
                    ConsensusThreshold::<T>::remove(&consensus_key);

                    Self::deposit_event(Event::ExpiredVotingSessionCleaned {
                        summary_instance: instance,
                        root_id,
                    });

                    return Ok(());
                }
            }

            Err(Error::<T>::CleanupFailed.into())
        }

        fn perform_ocw_recalculation(
            node: T::AccountId,
            signing_key: T::SignerId,
            instance: SummarySource,
            root_id: RootId<BlockNumberFor<T>>,
            onchain_root_hash: WatchtowerOnChainHash,
        ) {
            match Self::try_ocw_process_recalculation(instance, root_id.clone(), onchain_root_hash)
            {
                Ok(recalculated_hash_matches) => {
                    if let Err(e) = Self::submit_ocw_vote(
                        node,
                        signing_key,
                        instance,
                        root_id,
                        recalculated_hash_matches,
                    ) {
                        log::error!(
                            target: "runtime::watchtower::ocw",
                            "Failed to submit OCW vote for {:?} from instance {:?}: {:?}",
                            root_id, instance, e
                        );
                    }
                },
                Err(_e) => {},
            }
        }

        fn try_ocw_process_recalculation(
            instance: SummarySource,
            root_id: RootId<BlockNumberFor<T>>,
            on_chain_hash: WatchtowerOnChainHash,
        ) -> Result<bool, String> {
            let mut lock_identifier_vec = OCW_LOCK_PREFIX.to_vec();
            lock_identifier_vec.extend_from_slice(&instance.encode());
            lock_identifier_vec.extend_from_slice(&root_id.encode());

            let mut lock = AVN::<T>::get_ocw_locker(&lock_identifier_vec);

            let result = match lock.try_lock() {
                Ok(guard) => {
                    let recalculated_hash = Self::calculate_root_hash(
                        root_id.range.from_block,
                        root_id.range.to_block,
                    )?;
                    guard.forget();
                    Ok(recalculated_hash == on_chain_hash)
                },
                Err(_lock_error) =>
                    Err("Failed to acquire OCW lock for verification processing".to_string()),
            };
            result
        }

        fn calculate_root_hash(
            from_block: BlockNumberFor<T>,
            to_block: BlockNumberFor<T>,
        ) -> Result<WatchtowerOnChainHash, String> {
            let from_block_u32: u32 = from_block.try_into().map_err(|_| {
                let err_msg = format!(
                    "From_block number {:?} too large for u32 for URL construction",
                    from_block
                );
                err_msg
            })?;
            let to_block_u32: u32 = to_block.try_into().map_err(|_| {
                let err_msg = format!(
                    "To_block number {:?} too large for u32 for URL construction",
                    to_block
                );
                err_msg
            })?;

            let mut url_path = "roothash/".to_string();
            url_path.push_str(&from_block_u32.to_string());
            url_path.push_str("/");
            url_path.push_str(&to_block_u32.to_string());

            log::debug!(target: "runtime::watchtower::ocw", "Fetching recalculated root hash using AVN service, path: {}", url_path);

            let response = AVN::<T>::get_data_from_service(url_path).map_err(|dispatch_err| {
                let err_msg = format!("AVN service call failed: {:?}", dispatch_err);
                err_msg
            })?;

            Self::validate_response(response)
        }

        pub fn validate_response(response: Vec<u8>) -> Result<WatchtowerOnChainHash, String> {
            if response.len() != 64 {
                return Err("Invalid root hash length, expected 64 bytes".to_string());
            }

            let root_hash_str = core::str::from_utf8(&response)
                .map_err(|_| "Response contains invalid UTF8 bytes".to_string())?;

            let mut data: [u8; 32] = [0; 32];
            hex::decode_to_slice(root_hash_str.trim(), &mut data[..])
                .map_err(|_| "Response contains invalid hex string".to_string())?;

            Ok(H256::from_slice(&data))
        }

        fn submit_ocw_vote(
            node: T::AccountId,
            signing_key: T::SignerId,
            instance: SummarySource,
            root_id: RootId<BlockNumberFor<T>>,
            vote_is_valid: bool,
        ) -> Result<(), &'static str> {
            let consensus_key = (instance, root_id.clone());
            if VoteConsensusReached::<T>::get(&consensus_key) {
                return Ok(());
            }

            let current_block = frame_system::Pallet::<T>::block_number();
            if let Some(start_block) = VotingStartBlock::<T>::get(&consensus_key) {
                let voting_deadline = start_block + VotingPeriod::<T>::get();
                if current_block > voting_deadline {
                    return Ok(());
                }
            }

            let data_to_sign = (WATCHTOWER_OCW_CONTEXT, &instance, &root_id, vote_is_valid);
            let signature = match signing_key.sign(&data_to_sign.encode()) {
                Some(sig) => sig,
                None => {
                    return Err("Failed to sign vote data");
                },
            };

            let call = Call::ocw_vote {
                node: node.clone(),
                signing_key: signing_key.clone(),
                summary_instance: instance,
                root_id: root_id.clone(),
                vote_is_valid,
                signature,
            };

            match SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()) {
                Ok(()) => Ok(()),
                Err(_e) => Err("Failed to submit vote transaction to local pool"),
            }
        }

        fn try_reach_consensus(
            summary_instance: SummarySource,
            root_id: RootId<BlockNumberFor<T>>,
        ) -> DispatchResult {
            let consensus_key = (summary_instance, root_id.clone());
            if VoteConsensusReached::<T>::get(&consensus_key) {
                return Ok(());
            }

            let current_block = frame_system::Pallet::<T>::block_number();
            if let Some(start_block) = VotingStartBlock::<T>::get(&consensus_key) {
                let voting_deadline = start_block + VotingPeriod::<T>::get();
                if current_block > voting_deadline {
                    VoteCounters::<T>::remove(summary_instance, &root_id);
                    let _ = VoterHistory::<T>::clear_prefix(&consensus_key, u32::MAX, None);
                    VotingStartBlock::<T>::remove(&consensus_key);
                    PendingValidationRootHash::<T>::remove(&consensus_key);
                    ConsensusThreshold::<T>::remove(&consensus_key);
                    return Err(Error::<T>::VotingPeriodExpired.into());
                }
            }

            let required_for_consensus =
                ConsensusThreshold::<T>::get(&consensus_key).ok_or(Error::<T>::VotingNotStarted)?;

            let (yes_votes, no_votes) = VoteCounters::<T>::get(summary_instance, root_id.clone());

            let consensus_result;
            let consensus_reached;
            if yes_votes >= required_for_consensus {
                consensus_result = VotingStatus::Accepted;
                consensus_reached = true;
            } else if no_votes >= required_for_consensus {
                consensus_result = VotingStatus::Rejected;
                consensus_reached = true;
            } else {
                return Ok(());
            }

            if consensus_reached {
                VoteConsensusReached::<T>::insert(&consensus_key, true);

                Self::deposit_event(Event::WatchtowerConsensusReached {
                    summary_instance,
                    root_id,
                    consensus_result: consensus_result.clone(),
                });

                T::VoteStatusNotifier::on_voting_completed(
                    root_id.clone(),
                    consensus_result.clone(),
                )
                .map_err(|_e| Error::<T>::SummaryUpdateFailed)?;

                VoteCounters::<T>::remove(summary_instance, &root_id);
                let _ = VoterHistory::<T>::clear_prefix(&consensus_key, u32::MAX, None);
                VotingStartBlock::<T>::remove(&consensus_key);
                PendingValidationRootHash::<T>::remove(&consensus_key);
                ConsensusThreshold::<T>::remove(&consensus_key);
            }

            Ok(())
        }

        fn internal_submit_vote(
            voter: T::AccountId,
            summary_instance: SummarySource,
            root_id: RootId<BlockNumberFor<T>>,
            vote_is_valid: bool,
        ) -> DispatchResult {
            let consensus_key = (summary_instance, root_id.clone());
            let current_block = frame_system::Pallet::<T>::block_number();

            ensure!(!VoteConsensusReached::<T>::get(&consensus_key), {
                Error::<T>::ConsensusAlreadyReached
            });

            // Check if consensus is mathematically already reached before casting this vote
            if let Some(required_consensus) = ConsensusThreshold::<T>::get(&consensus_key) {
                let (current_yes_votes, current_no_votes) =
                    VoteCounters::<T>::get(summary_instance, root_id.clone());

                // If either yes or no votes have already reached consensus threshold, reject new
                // votes
                if current_yes_votes >= required_consensus || current_no_votes >= required_consensus
                {
                    return Err(Error::<T>::ConsensusAlreadyReached.into());
                }
            }

            let voting_start_block = VotingStartBlock::<T>::get(&consensus_key);
            if let Some(start_block) = voting_start_block {
                let voting_deadline = start_block + VotingPeriod::<T>::get();
                if current_block > voting_deadline {
                    return Err(Error::<T>::VotingPeriodExpired.into());
                }
            } else {
                let total_authorized_watchtowers =
                    T::NodeManager::get_authorized_watchtowers_count();
                let required_for_consensus = (total_authorized_watchtowers * 2) / 3;

                VotingStartBlock::<T>::insert(&consensus_key, current_block);
                ConsensusThreshold::<T>::insert(&consensus_key, required_for_consensus);
            }

            // Check if voter has already voted
            ensure!(
                VoterHistory::<T>::get(&consensus_key, &voter).is_none(),
                Error::<T>::AlreadyVoted
            );

            // Record the vote and update counters
            VoterHistory::<T>::insert(&consensus_key, &voter, ());
            VoteCounters::<T>::mutate(
                summary_instance,
                root_id.clone(),
                |(yes_votes, no_votes)| {
                    if vote_is_valid {
                        *yes_votes += 1;
                    } else {
                        *no_votes += 1;
                    }
                },
            );

            Self::deposit_event(Event::WatchtowerVoteSubmitted {
                voter: voter.clone(),
                summary_instance,
                root_id: root_id.clone(),
                vote_is_valid,
            });

            // Check for consensus immediately after each vote
            Self::try_reach_consensus(summary_instance, root_id.clone()).map_err(|e| e)?;

            Ok(())
        }

        fn get_node_from_signing_key() -> Option<(T::AccountId, T::SignerId)> {
            match T::NodeManager::get_node_from_local_signing_keys() {
                Some((node, signing_key)) => Some((node, signing_key)),
                None => None,
            }
        }

        pub fn offchain_signature_is_valid<D: Encode>(
            data: &D,
            signer: &T::SignerId,
            signature: &<T::SignerId as sp_runtime::RuntimeAppPublic>::Signature,
        ) -> bool {
            let signature_valid =
                data.using_encoded(|encoded_data| signer.verify(&encoded_data, &signature));

            signature_valid
        }

        pub fn get_voting_status(
            instance: SummarySource,
            root_id: RootId<BlockNumberFor<T>>,
        ) -> Option<(BlockNumberFor<T>, BlockNumberFor<T>, u32, u32)> {
            let consensus_key = (instance, root_id.clone());

            if let Some(start_block) = VotingStartBlock::<T>::get(&consensus_key) {
                let current_block = frame_system::Pallet::<T>::block_number();
                let deadline = start_block + VotingPeriod::<T>::get();

                if current_block > deadline {
                    Self::cleanup_voting_session(instance, root_id);
                    return None;
                }

                let (yes_votes, no_votes) = VoteCounters::<T>::get(instance, root_id);
                Some((start_block, deadline, yes_votes, no_votes))
            } else {
                None
            }
        }

        pub fn get_voting_period() -> BlockNumberFor<T> {
            VotingPeriod::<T>::get()
        }

        pub fn is_voting_active(
            instance: SummarySource,
            root_id: RootId<BlockNumberFor<T>>,
        ) -> bool {
            let consensus_key = (instance, root_id.clone());

            if VoteConsensusReached::<T>::get(&consensus_key) {
                return false;
            }

            if let Some(start_block) = VotingStartBlock::<T>::get(&consensus_key) {
                let current_block = frame_system::Pallet::<T>::block_number();
                let deadline = start_block + VotingPeriod::<T>::get();

                if current_block <= deadline {
                    true
                } else {
                    Self::cleanup_voting_session(instance, root_id);
                    false
                }
            } else {
                false
            }
        }

        pub fn cleanup_expired_votes(
            instance: SummarySource,
            root_id: RootId<BlockNumberFor<T>>,
        ) -> DispatchResult {
            Self::internal_cleanup_expired_votes(instance, root_id)
        }

        fn cleanup_voting_session(instance: SummarySource, root_id: RootId<BlockNumberFor<T>>) {
            let consensus_key = (instance, root_id.clone());

            VoteCounters::<T>::remove(instance, &root_id);
            let _ = VoterHistory::<T>::clear_prefix(&consensus_key, u32::MAX, None);
            VotingStartBlock::<T>::remove(&consensus_key);
            PendingValidationRootHash::<T>::remove(&consensus_key);
            ConsensusThreshold::<T>::remove(&consensus_key);

            Self::deposit_event(Event::ExpiredVotingSessionCleaned {
                summary_instance: instance,
                root_id,
            });
        }
    }
}

pub struct ExternalNotifier<T>(sp_std::marker::PhantomData<T>);

pub type SummarySourceId = u8;

impl<T: Config> sp_avn_common::ExternalNotification<BlockNumberFor<T>> for ExternalNotifier<T> {
    fn on_summary_ready_for_validation(
        instance_id: SummarySourceId,
        root_id: sp_avn_common::RootId<BlockNumberFor<T>>,
        root_hash: sp_core::H256,
    ) -> DispatchResult {
        let summary_instance = match instance_id {
            1 => SummarySource::EthereumBridge, // EthereumInstanceId
            2 => SummarySource::AnchorStorage,  // AvnInstanceId
            _ => {
                return Err(DispatchError::Other("UnknownSummaryInstance"));
            },
        };

        Pallet::<T>::notify_summary_ready_for_validation(summary_instance, root_id, root_hash)
    }
}
