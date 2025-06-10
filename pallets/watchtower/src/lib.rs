#![cfg_attr(not(feature = "std"), no_std)]
use frame_support::{dispatch::DispatchResult, pallet_prelude::*, traits::{IsType, IsSubType}};
use frame_system::{
    pallet_prelude::*,
    offchain::{SendTransactionTypes, SubmitTransaction},
    ensure_none,
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
use sp_avn_common::{RootId as PalletSummaryRootIdGeneric, SummaryStatus as PalletSummaryStatusGeneric,};

use sp_core::H256;
use sp_runtime::{
    traits::{AtLeast32Bit, Dispatchable, ValidateUnsigned},
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
pub const HTTP_TIMEOUT_MS: u64 = 5000;
pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);
pub const WATCHTOWER_OCW_CONTEXT: &[u8] = b"watchtower_ocw_vote";
pub const WATCHTOWER_VOTE_PROVIDE_TAG: &[u8] = b"WatchtowerVoteProvideTag";
pub const DEFAULT_VOTING_PERIOD_BLOCKS: u32 = 100; // ~10 minutes with 6s blocks

pub type AVN<T> = avn::Pallet<T>;

#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, Clone, Copy, RuntimeDebug)]
pub enum SummarySourceInstance {
    EthereumBridge,
    AnchorStorage,
}

pub type WatchtowerRootId<BlockNumber> = PalletSummaryRootIdGeneric<BlockNumber>;
pub type WatchtowerOnChainHash = H256;
pub type WatchtowerSummaryStatus = PalletSummaryStatusGeneric;

pub trait SummaryServices<TSystemConfig: frame_system::Config> {
    fn update_summary_status(
        instance: SummarySourceInstance,
        root_id: WatchtowerRootId<BlockNumberFor<TSystemConfig>>,
        status: WatchtowerSummaryStatus,
    ) -> DispatchResult;
}

pub trait EventInterpreter<SystemRuntimeEvent, BlockNumber: AtLeast32Bit, OnChainHash> {
    fn interpret_summary_ready_event(
        event: &SystemRuntimeEvent,
    ) -> Option<(SummarySourceInstance, WatchtowerRootId<BlockNumber>, OnChainHash)>;
}

pub trait NodeManagerInterface<AccountId, SignerId, MaxWatchtowers: Get<u32>> {
    fn get_authorized_watchtowers() -> Result<BoundedVec<AccountId, MaxWatchtowers>, DispatchError>;

    fn is_authorized_watchtower(
        who: &AccountId,
    ) -> bool;
    
    fn get_node_signing_key(node: &AccountId) -> Option<SignerId>;
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
        SendTransactionTypes<Call<Self>>
        + frame_system::Config
        + pallet_avn::Config
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

        type SignerId: Member
            + Parameter
            + sp_runtime::RuntimeAppPublic
            + Ord
            + MaxEncodedLen;

        type SummaryServiceProvider: SummaryServices<Self>;
        type EventInterpreter: EventInterpreter<
            <Self as frame_system::Config>::RuntimeEvent,
            BlockNumberFor<Self>,
            WatchtowerOnChainHash,
        >;
        type NodeManager: NodeManagerInterface<Self::AccountId, Self::SignerId, Self::MaxWatchtowers>;
        type MaxWatchtowers: Get<u32>;
        
    }

    #[pallet::storage]
    #[pallet::getter(fn individual_votes)]
    pub type IndividualWatchtowerVotes<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, SummarySourceInstance,
        Blake2_128Concat, WatchtowerRootId<BlockNumberFor<T>>,
        BoundedVec<(T::AccountId, bool), T::MaxWatchtowers>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn consensus_reached_flag)]
    pub type VoteConsensusReached<T: Config> = StorageMap<
        _,
        Blake2_128Concat, (SummarySourceInstance, WatchtowerRootId<BlockNumberFor<T>>),
        bool,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn voting_start_block)]
    pub type VotingStartBlock<T: Config> = StorageMap<
        _,
        Blake2_128Concat, (SummarySourceInstance, WatchtowerRootId<BlockNumberFor<T>>),
        BlockNumberFor<T>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn voting_period)]
    pub type VotingPeriod<T: Config> = StorageValue<
        _,
        BlockNumberFor<T>,
        ValueQuery,
        DefaultVotingPeriod<T>,
    >;



    #[pallet::type_value]
    pub fn DefaultVotingPeriod<T: Config>() -> BlockNumberFor<T> {
        DEFAULT_VOTING_PERIOD_BLOCKS.into()
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        VerificationResultSubmitted {
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            result: WatchtowerSummaryStatus,
            submitter_ocw: bool,
        },
        VerificationProcessingError {
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            reason: VerificationError,
        },
        WatchtowerVoteSubmitted {
            voter: T::AccountId,
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            vote_is_valid: bool,
        },
        WatchtowerConsensusReached {
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            consensus_result: WatchtowerSummaryStatus,
        },
        VotingPeriodUpdated {
            old_period: BlockNumberFor<T>,
            new_period: BlockNumberFor<T>,
        },
    }

    #[derive(Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, Clone, Debug)]
    pub enum VerificationError {
        LockAcquisitionFailed,
        HttpCallFailed,
        SubmitTxFailed,
        SummaryNotReadyForWatchtower,
        DataConversionError,
        RecalculationResponseError,
    }

    #[pallet::error]
    pub enum Error<T> {
        SummaryUpdateFailed,
        InvalidVerificationSubmission,
        NotAuthorizedWatchtower,
        AlreadyVoted,
        ConsensusAlreadyReached,
        FailedToGetAuthorizedWatchtowers,
        TooFewWatchtowersToFormConsensus,
        TooManyVotes,
        VotingPeriodExpired,
        VotingNotStarted,
        InvalidVotingPeriod,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(block_number: BlockNumberFor<T>)
        {
            log::debug!(target: "runtime::watchtower::ocw", "Watchtower OCW running for block {:?}", block_number);

            for record in frame_system::Pallet::<T>::read_events_no_consensus() {
                if let Some((instance, root_id, onchain_root_hash)) =
                    T::EventInterpreter::interpret_summary_ready_event(&record.event)
                {
                    log::info!(target: "runtime::watchtower::ocw", "[{:?}] Detected SummaryReadyForValidation for root: {:?}, onchain_hash: {:?}", instance, root_id, onchain_root_hash);
                    Self::perform_ocw_recalculation(instance, root_id, onchain_root_hash);
                }
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::submit_watchtower_vote())]
        pub fn submit_watchtower_vote(
            origin: OriginFor<T>,
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
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
        #[pallet::weight(<T as pallet::Config>::WeightInfo::submit_watchtower_vote())]
        pub fn offchain_submit_watchtower_vote(
            origin: OriginFor<T>,
            node: T::AccountId,
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            vote_is_valid: bool,
            _signature: <T::SignerId as sp_runtime::RuntimeAppPublic>::Signature,
        ) -> DispatchResult {
            log::error!(
                target: "runtime::watchtower::execute",
                "!!! [EXECUTE_VOTE_STARTED] Watchtower vote EXECUTION has BEGUN for node {:?}, root {:?}, vote {} !!!",
                node, root_id, vote_is_valid
            );

            ensure_none(origin).map_err(|e| {
                log::error!(
                    target: "runtime::watchtower::execute",
                    "[EXECUTE_VOTE] FAILED (Origin Check): ensure_none failed: {:?}. Node: {:?}, Root: {:?}",
                    e, node, root_id
                );
                e
            })?;
            
            log::debug!(
                target: "runtime::watchtower::execute",
                "[EXECUTE_VOTE] Origin check passed, verifying node authorization. Node: {:?}, Root: {:?}",
                node, root_id
            );

            ensure!(
                T::NodeManager::is_authorized_watchtower(&node),
                {
                    log::error!(
                        target: "runtime::watchtower::execute",
                        "[EXECUTE_VOTE] FAILED (Authorization): Node {:?} is not an authorized watchtower. Root: {:?}",
                        node, root_id
                    );
                    Error::<T>::NotAuthorizedWatchtower
                }
            );
            
            log::debug!(
                target: "runtime::watchtower::execute",
                "[EXECUTE_VOTE] Node authorization passed, calling internal_submit_vote. Node: {:?}, Root: {:?}",
                node, root_id
            );

            Self::internal_submit_vote(node.clone(), summary_instance, root_id.clone(), vote_is_valid)
                .map_err(|e| {
                    log::error!(
                        target: "runtime::watchtower::execute",
                        "[EXECUTE_VOTE] FAILED (Internal Submit): internal_submit_vote failed for node {:?}, root {:?}: {:?}" ,
                        node, root_id, e
                    );
                    e
                })?;

            log::info!(
                target: "runtime::watchtower::execute",
                "[EXECUTE_VOTE] SUCCESS: Watchtower vote executed successfully for node {:?}, root {:?}.",
                node, root_id
            );

            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::submit_watchtower_vote())]
        pub fn set_voting_period(
            origin: OriginFor<T>,
            new_period: BlockNumberFor<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // Validate the new voting period (must be at least 10 blocks)
            let min_period: BlockNumberFor<T> = 10u32.into();
            ensure!(
                new_period >= min_period,
                Error::<T>::InvalidVotingPeriod
            );

            let old_period = VotingPeriod::<T>::get();
            VotingPeriod::<T>::put(new_period);

            Self::deposit_event(Event::VotingPeriodUpdated {
                old_period,
                new_period,
            });

            log::info!(
                target: "runtime::watchtower::admin",
                "Voting period updated from {:?} to {:?} blocks",
                old_period, new_period
            );

            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::submit_watchtower_vote())]
        pub fn query_voting_info(
            origin: OriginFor<T>,
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            let consensus_key = (summary_instance, root_id.clone());
            let current_block = frame_system::Pallet::<T>::block_number();
            let voting_period = VotingPeriod::<T>::get();
            
            if let Some(start_block) = VotingStartBlock::<T>::get(&consensus_key) {
                let deadline = start_block + voting_period;
                let votes = IndividualWatchtowerVotes::<T>::get(summary_instance, root_id.clone());
                let consensus_reached = VoteConsensusReached::<T>::get(&consensus_key);
                
                log::info!(
                    target: "runtime::watchtower::query",
                    "Voting info for {:?}: Start: {:?}, Current: {:?}, Deadline: {:?}, Votes: {}, Consensus: {}",
                    consensus_key, start_block, current_block, deadline, votes.len(), consensus_reached
                );
            } else {
                log::info!(
                    target: "runtime::watchtower::query",
                    "No voting started for {:?}. Current voting period: {:?} blocks",
                    consensus_key, voting_period
                );
            }

            Ok(())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            if let Call::offchain_submit_watchtower_vote { 
                node, 
                summary_instance, 
                root_id, 
                vote_is_valid, 
                signature 
            } = call {
                log::debug!(
                    target: "runtime::watchtower::validate",
                    "[VALIDATE_UNSIGNED] Validating watchtower vote: node={:?}, instance={:?}, root_id={:?}, vote={:?}, source={:?}",
                    node, summary_instance, root_id, vote_is_valid, source
                );

                // Allow Local (OCW), External (propagated), and InBlock (when block author includes it)
                match source {
                    TransactionSource::Local | TransactionSource::External | TransactionSource::InBlock => {
                        // Source is acceptable, proceed with other validations
                        log::debug!(
                            target: "runtime::watchtower::validate",
                            "[VALIDATE_UNSIGNED] Source {:?} is acceptable.",
                            source
                        );
                    }
                }

                // Validate the node is authorized
                if !T::NodeManager::is_authorized_watchtower(node) {
                    log::error!(
                        target: "runtime::watchtower::validate",
                        "[VALIDATE_UNSIGNED] REJECTED (Pre-Auth): Node {:?} is not an authorized watchtower.",
                        node
                    );
                    return InvalidTransaction::Call.into();
                }

                let signing_key = match T::NodeManager::get_node_signing_key(node) {
                    Some(key) => key,
                    None => {
                        log::error!(
                            target: "runtime::watchtower::validate",
                            "[VALIDATE_UNSIGNED] REJECTED (Pre-Key): No signing key found for node {:?}.",
                            node
                        );
                        return InvalidTransaction::Call.into();
                    },
                };

                if Self::offchain_signature_is_valid(
                    &(WATCHTOWER_OCW_CONTEXT, summary_instance, root_id, vote_is_valid),
                    &signing_key,
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
                        signature.encode()[0..8].to_vec() 
                    );
                    
                    let provides_tag = unique_payload_for_provides.encode();
                    
                    log::info!(
                        target: "runtime::watchtower::validate",
                        "[VALIDATE_UNSIGNED] ACCEPTED for node {:?} at block {:?}. Source: {:?}. Provides tag hash: {:?}. Vote: {:?}",
                        node, current_block, source, sp_io::hashing::blake2_256(&provides_tag), vote_is_valid
                    );
                    
                    ValidTransaction::with_tag_prefix("WatchtowerOCW")
                        .priority(TransactionPriority::MAX)
                        .and_provides(vec![provides_tag])
                        .longevity(64_u64)
                        .propagate(true)
                        .build()
                } else {
                    log::error!(
                        target: "runtime::watchtower::validate",
                        "[VALIDATE_UNSIGNED] REJECTED (Signature Invalid): Invalid signature for node {:?}.",
                        node
                    );
                    InvalidTransaction::BadSigner.into()
                }
            } else {
                log::warn!(
                    target: "runtime::watchtower::validate",
                    "[VALIDATE_UNSIGNED] WARNING: Received non-watchtower-vote call for unsigned validation: {:?}.",
                    call
                );
                InvalidTransaction::Call.into()
            }
        }
    }

    impl<T: Config> Pallet<T> {
        fn perform_ocw_recalculation(
            instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            onchain_root_hash: WatchtowerOnChainHash,
        ) {
            let maybe_node_info = Self::get_node_from_signing_key();
            let (node, signing_key) = match maybe_node_info {
                Some(node_info) => node_info,
                None => {
                    log::debug!(
                        target: "runtime::watchtower::ocw",
                        "No registered node found for OCW operations"
                    );
                    return;
                }
            };

            match Self::try_ocw_process_recalculation(instance, root_id.clone(), onchain_root_hash) {
                Ok(recalculated_hash_matches) => {
                    log::info!(
                        target: "runtime::watchtower::ocw",
                        "OCW recalculation for {:?} from instance {:?}: Onchain hash matches recalculated hash: {}.",
                        root_id, instance, recalculated_hash_matches
                    );
                    
                    if let Err(e) = Self::submit_ocw_vote(node, signing_key, instance, root_id, recalculated_hash_matches) {
                        log::error!(
                            target: "runtime::watchtower::ocw",
                            "Failed to submit OCW vote for {:?} from instance {:?}: {:?}",
                            root_id, instance, e
                        );
                    }
                },
                Err(e) => {
                    log::error!(
                        target: "runtime::watchtower::ocw",
                        "OCW recalculation processing error for {:?} from instance {:?}: {:?}",
                        root_id, instance, e
                    );
                    Self::deposit_event(Event::VerificationProcessingError { summary_instance: instance, root_id, reason: e });
                }
            }
        }

        fn try_ocw_process_recalculation(
            instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            on_chain_hash: WatchtowerOnChainHash,
        ) -> Result<bool, VerificationError> {
            log::info!(target: "runtime::watchtower::ocw", "[{:?}] Attempting recalculation for root: {:?}, on_chain_hash: {:?}", instance, root_id, on_chain_hash);

            let mut lock_identifier_vec = OCW_LOCK_PREFIX.to_vec();
            lock_identifier_vec.extend_from_slice(&instance.encode());
            lock_identifier_vec.extend_from_slice(&root_id.encode());

            let mut lock = AVN::<T>::get_ocw_locker(&lock_identifier_vec);

            let result: Result<bool, VerificationError> = match lock.try_lock() {
                Ok(guard) => {
                    log::debug!(target: "runtime::watchtower::ocw", "[{:?}] Lock acquired for root: {:?}", instance, root_id);

                    match Self::fetch_recalculated_root_hash_sync(
                        root_id.range.from_block,
                        root_id.range.to_block,
                    ) {
                        Ok(recalculated_hash) => {
                            log::info!(target: "runtime::watchtower::ocw", "[{:?}] Recalculated hash for {:?}: {:?}. On-chain hash: {:?}", instance, root_id, recalculated_hash, on_chain_hash);
                            guard.forget();
                            Ok(recalculated_hash == on_chain_hash)
                        },
                        Err(e) => {
                            log::error!(target: "runtime::watchtower::ocw", "[{:?}] HTTP call failed for {:?}: {:?}", instance, root_id, e);
                            Self::deposit_event(Event::VerificationProcessingError {
                                summary_instance: instance,
                                root_id,
                                reason: VerificationError::HttpCallFailed,
                            });
                            Err(VerificationError::HttpCallFailed)
                        }
                    }
                }
                Err(_lock_error) => {
                    log::warn!(target: "runtime::watchtower::ocw", "[{:?}] Failed to acquire lock for root: {:?}. Might be processed by another worker.", instance, root_id);
                    Self::deposit_event(Event::VerificationProcessingError {
                        summary_instance: instance,
                        root_id,
                        reason: VerificationError::LockAcquisitionFailed,
                    });
                    Err(VerificationError::LockAcquisitionFailed)
                }
            };
            result
        }

        fn fetch_recalculated_root_hash_sync(
            from_block: BlockNumberFor<T>,
            to_block: BlockNumberFor<T>,
        ) -> Result<WatchtowerOnChainHash, String> {
            let from_block_u32: u32 = from_block.try_into().map_err(|_| {
                let err_msg = format!(
                    "From_block number {:?} too large for u32 for URL construction",
                    from_block
                );
                log::error!(target: "runtime::watchtower::ocw", "{}", err_msg);
                err_msg
            })?;
            let to_block_u32: u32 = to_block.try_into().map_err(|_| {
                let err_msg = format!(
                    "To_block number {:?} too large for u32 for URL construction",
                    to_block
                );
                log::error!(target: "runtime::watchtower::ocw", "{}", err_msg);
                err_msg
            })?;

            let mut url_path = "roothash/".to_string();
            url_path.push_str(&from_block_u32.to_string());
            url_path.push_str("/");
            url_path.push_str(&to_block_u32.to_string());

            log::debug!(target: "runtime::watchtower::ocw", "Fetching recalculated root hash using AVN service, path: {}", url_path);

            let response = AVN::<T>::get_data_from_service(url_path)
                .map_err(|dispatch_err| {
                    let err_msg = format!("AVN service call failed: {:?}", dispatch_err);
                    log::error!(target: "runtime::watchtower::ocw", "{}", err_msg);
                    err_msg
                })?;

            Self::validate_response(response).map_err(|e| {
                log::error!(target: "runtime::watchtower::ocw", "Error validating service response: {:?}", e);
                format!("Response validation failed: {:?}", e)
            })
        }

        pub fn validate_response(response: Vec<u8>) -> Result<WatchtowerOnChainHash, DispatchError> {
            if response.len() != 64 {
                log::error!(
                    target: "runtime::watchtower::ocw",
                    "Invalid root hash length: {}, expected 64",
                    response.len()
                );
                return Err(DispatchError::Other("InvalidRootHashLength"));
            }

            let root_hash_str = core::str::from_utf8(&response)
                .map_err(|_| {
                    log::error!(target: "runtime::watchtower::ocw", "Invalid UTF8 bytes in response");
                    DispatchError::Other("InvalidUTF8Bytes")
                })?;

            let mut data: [u8; 32] = [0; 32];
            hex::decode_to_slice(root_hash_str.trim(), &mut data[..])
                .map_err(|_| {
                    log::error!(
                        target: "runtime::watchtower::ocw",
                        "Invalid hex string in response: '{}'",
                        root_hash_str
                    );
                    DispatchError::Other("InvalidHexString")
                })?;

            Ok(H256::from_slice(&data))
        }

        fn submit_ocw_vote(
            node: T::AccountId,
            signing_key: T::SignerId,
            instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            vote_is_valid: bool,
        ) -> Result<(), &'static str> {
            log::debug!(
                target: "runtime::watchtower::ocw",
                "Submitting OCW vote for {:?} from instance {:?}, vote: {}",
                root_id, instance, vote_is_valid
            );

            let consensus_key = (instance, root_id.clone());
            if VoteConsensusReached::<T>::get(&consensus_key) {
                log::warn!(
                    target: "runtime::watchtower::ocw",
                    "Consensus already reached for {:?}, skipping vote submission",
                    consensus_key
                );
                return Ok(());
            }

            let current_block = frame_system::Pallet::<T>::block_number();
            if let Some(start_block) = VotingStartBlock::<T>::get(&consensus_key) {
                let voting_deadline = start_block + VotingPeriod::<T>::get();
                if current_block > voting_deadline {
                    log::warn!(
                        target: "runtime::watchtower::ocw",
                        "Voting period expired for {:?}, skipping vote submission. Current: {:?}, Deadline: {:?}",
                        consensus_key, current_block, voting_deadline
                    );
                    return Ok(());
                }
            }

            let data_to_sign = (WATCHTOWER_OCW_CONTEXT, &instance, &root_id, vote_is_valid);
            let signature = match signing_key.sign(&data_to_sign.encode()) {
                Some(sig) => sig,
                None => {
                    log::error!(
                        target: "runtime::watchtower::ocw",
                        "[SUBMIT_OCW_VOTE] ERROR: Failed to sign OCW vote data for root {:?} by node {:?}.",
                        root_id, node
                    );
                    return Err("Failed to sign vote data");
                }
            };

            let call = Call::offchain_submit_watchtower_vote {
                node: node.clone(),
                summary_instance: instance,
                root_id: root_id.clone(),
                vote_is_valid,
                signature,
            };

            log::debug!(
                target: "runtime::watchtower::ocw",
                "[SUBMIT_OCW_VOTE] Attempting to submit unsigned transaction for node {:?}, root {:?}, vote {}",
                node, root_id, vote_is_valid
            );

            match SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()) {
                Ok(()) => {
                    log::info!(
                        target: "runtime::watchtower::ocw",
                        "[SUBMIT_OCW_VOTE] SUCCESS: OCW vote TX submitted to LOCAL POOL for root {:?}, vote {}. Node: {:?}.",
                        root_id, vote_is_valid, node
                    );
                    Ok(())
                }
                Err(e) => {
                    log::error!(
                        target: "runtime::watchtower::ocw",
                        "[SUBMIT_OCW_VOTE] FAILED to submit OCW vote TX to local pool for root {:?}, node {:?}: {:?}",
                        root_id, node, e
                    );
                    Err("Failed to submit vote transaction to local pool")
                }
            }
        }

        fn try_reach_consensus(
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
        ) -> DispatchResult {
            let consensus_key = (summary_instance, root_id.clone());
            if VoteConsensusReached::<T>::get(&consensus_key) {
                return Ok(());
            }

            let current_block = frame_system::Pallet::<T>::block_number();
            if let Some(start_block) = VotingStartBlock::<T>::get(&consensus_key) {
                let voting_deadline = start_block + VotingPeriod::<T>::get();
                if current_block > voting_deadline {
                    log::warn!(
                        target: "runtime::watchtower",
                        "[{:?}] Voting period expired for {:?}. Cleaning up votes. Current block: {:?}, deadline: {:?}",
                        summary_instance, root_id, current_block, voting_deadline
                    );
                    IndividualWatchtowerVotes::<T>::remove(summary_instance, &root_id);
                    VotingStartBlock::<T>::remove(&consensus_key);
                    return Err(Error::<T>::VotingPeriodExpired.into());
                }
            }

            let authorized_watchtowers = T::NodeManager::get_authorized_watchtowers()
                .map_err(|_| Error::<T>::FailedToGetAuthorizedWatchtowers)?;

            let total_authorized_watchtowers = authorized_watchtowers.len() as u32;
            let required_for_consensus = (total_authorized_watchtowers * 2 + 2) / 3;

            let current_votes = IndividualWatchtowerVotes::<T>::get(summary_instance, root_id.clone());

            let mut valid_votes = Vec::new();
            for (voter, vote) in current_votes.iter() {
                if authorized_watchtowers.contains(voter) {
                    valid_votes.push((voter.clone(), *vote));
                }
            }

            log::debug!(target: "runtime::watchtower", "[{:?}] RootID {:?}: Total authorized: {}, Required for consensus: {}, Valid votes: {}",
                summary_instance, root_id, total_authorized_watchtowers, required_for_consensus, valid_votes.len());

            let total_votes = valid_votes.len() as u32;
            
            if total_votes == 0 {
                log::debug!(target: "runtime::watchtower", "[{:?}] No votes yet for {:?}", summary_instance, root_id);
                return Ok(());
            }

            let agreeing_votes = valid_votes.iter().filter(|(_, vote)| *vote).count() as u32;
            let disagreeing_votes = valid_votes.iter().filter(|(_, vote)| !*vote).count() as u32;

            let consensus_result;
            let consensus_reached;
            if agreeing_votes >= required_for_consensus {
                consensus_result = WatchtowerSummaryStatus::Accepted;
                consensus_reached = true;
                log::info!(target: "runtime::watchtower", "[{:?}] Consensus ACCEPTED for {:?}. Agreeing: {}/{}, Disagreeing: {}", 
                    summary_instance, root_id, agreeing_votes, required_for_consensus, disagreeing_votes);
            } else if disagreeing_votes >= required_for_consensus {
                consensus_result = WatchtowerSummaryStatus::Rejected;
                consensus_reached = true;
                log::info!(target: "runtime::watchtower", "[{:?}] Consensus REJECTED for {:?}. Agreeing: {}, Disagreeing: {}/{}", 
                    summary_instance, root_id, agreeing_votes, disagreeing_votes, required_for_consensus);
            } else {
                log::debug!(target: "runtime::watchtower", "[{:?}] No consensus yet for {:?}. Agreeing: {}/{}, Disagreeing: {}/{}", 
                    summary_instance, root_id, agreeing_votes, required_for_consensus, disagreeing_votes, required_for_consensus);
                return Ok(());
            }

            if consensus_reached {
                T::SummaryServiceProvider::update_summary_status(summary_instance, root_id.clone(), consensus_result.clone())
                    .map_err(|e| {
                        log::error!(
                            target: "runtime::watchtower",
                            "Failed to set summary status for {:?} in instance {:?}: {:?}",
                            root_id, summary_instance, e
                        );
                        Error::<T>::SummaryUpdateFailed
                    })?;

                VoteConsensusReached::<T>::insert(&consensus_key, true);
                
                VotingStartBlock::<T>::remove(&consensus_key);
                
                Self::deposit_event(Event::WatchtowerConsensusReached {
                    summary_instance,
                    root_id,
                    consensus_result,
                });
                
                log::info!(
                    target: "runtime::watchtower",
                    "[{:?}] Consensus reached and summary status updated for {:?}",
                    summary_instance, root_id
                );
            }
            
            Ok(())
        }

        fn internal_submit_vote(
            voter: T::AccountId,
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            vote_is_valid: bool,
        ) -> DispatchResult {
            log::info!(
                target: "runtime::watchtower::vote",
                "Starting internal_submit_vote: voter={:?}, instance={:?}, root_id={:?}, vote={:?}",
                voter, summary_instance, root_id, vote_is_valid
            );

            let consensus_key = (summary_instance, root_id.clone());
            let current_block = frame_system::Pallet::<T>::block_number();
            
            log::debug!(
                target: "runtime::watchtower::vote",
                "Checking if consensus already reached for key: {:?}",
                consensus_key
            );

            ensure!(
                !VoteConsensusReached::<T>::get(&consensus_key),
                {
                    log::error!(
                        target: "runtime::watchtower::vote",
                        "FAILED: Consensus already reached for {:?}",
                        consensus_key
                    );
                    Error::<T>::ConsensusAlreadyReached
                }
            );

            let voting_start_block = VotingStartBlock::<T>::get(&consensus_key);
            if let Some(start_block) = voting_start_block {
                let voting_deadline = start_block + VotingPeriod::<T>::get();
                if current_block > voting_deadline {
                    log::error!(
                        target: "runtime::watchtower::vote",
                        "FAILED: Voting period expired for {:?}. Current block: {:?}, deadline: {:?}",
                        consensus_key, current_block, voting_deadline
                    );
                    return Err(Error::<T>::VotingPeriodExpired.into());
                }
                log::debug!(
                    target: "runtime::watchtower::vote",
                    "Voting period check passed. Current block: {:?}, deadline: {:?}",
                    current_block, voting_deadline
                );
            } else {
                VotingStartBlock::<T>::insert(&consensus_key, current_block);
                log::info!(
                    target: "runtime::watchtower::vote",
                    "Initialized voting period for {:?} at block {:?}",
                    consensus_key, current_block
                );
            }

            log::debug!(
                target: "runtime::watchtower::vote",
                "Consensus not yet reached, proceeding with vote storage"
            );

            IndividualWatchtowerVotes::<T>::try_mutate(
                summary_instance,
                root_id.clone(),
                |votes| -> DispatchResult {
                    log::debug!(
                        target: "runtime::watchtower::vote",
                        "Current votes before mutation: {:?}",
                        votes
                    );

                    if votes.iter().any(|(acc, _)| acc == &voter) {
                        log::error!(
                            target: "runtime::watchtower::vote",
                            "FAILED: Voter {:?} has already voted",
                            voter
                        );
                        return Err(Error::<T>::AlreadyVoted.into());
                    }
                    
                    votes.try_push((voter.clone(), vote_is_valid))
                        .map_err(|_| {
                            log::error!(
                                target: "runtime::watchtower::vote",
                                "FAILED: Too many votes, cannot add vote for {:?}",
                                voter
                            );
                            Error::<T>::TooManyVotes
                        })?;

                    log::info!(
                        target: "runtime::watchtower::vote",
                        "Successfully added vote for {:?}. New votes: {:?}",
                        voter, votes
                    );
                    
                    Ok(())
                }
            )?;

            log::debug!(
                target: "runtime::watchtower::vote",
                "Vote stored successfully, depositing event"
            );

            Self::deposit_event(Event::WatchtowerVoteSubmitted {
                voter: voter.clone(),
                summary_instance,
                root_id: root_id.clone(),
                vote_is_valid,
            });

            log::debug!(
                target: "runtime::watchtower::vote",
                "Event deposited, attempting to reach consensus"
            );

            Self::try_reach_consensus(summary_instance, root_id.clone()).map_err(|e| {
                log::error!(
                    target: "runtime::watchtower::vote",
                    "FAILED: try_reach_consensus failed for {:?}: {:?}",
                    root_id, e
                );
                e
            })?;

            log::info!(
                target: "runtime::watchtower::vote",
                "Successfully completed internal_submit_vote for voter: {:?}",
                voter
            );

            Ok(())
        }

        fn get_node_from_signing_key() -> Option<(T::AccountId, T::SignerId)> {
            let local_keys: Vec<T::SignerId> = T::SignerId::all();
            log::debug!(
                target: "runtime::watchtower::ocw",
                "Local signing keys available: {}",
                local_keys.len()
            );

            let authorized_watchtowers = match T::NodeManager::get_authorized_watchtowers() {
                Ok(watchtowers) => {
                    log::debug!(
                        target: "runtime::watchtower::ocw",
                        "Found {} authorized watchtowers",
                        watchtowers.len()
                    );
                    watchtowers
                },
                Err(_) => {
                    log::error!(
                        target: "runtime::watchtower::ocw",
                        "Failed to get authorized watchtowers"
                    );
                    return None;
                }
            };

            for local_key in local_keys.iter() {
                log::debug!(
                    target: "runtime::watchtower::ocw",
                    "Checking local key against authorized watchtowers"
                );
                
                for node in authorized_watchtowers.iter() {
                    if let Some(node_signing_key) = T::NodeManager::get_node_signing_key(node) {
                        log::debug!(
                            target: "runtime::watchtower::ocw",
                            "Comparing local key with watchtower node signing key"
                        );
                        if *local_key == node_signing_key {
                            log::info!(
                                target: "runtime::watchtower::ocw",
                                "Found matching watchtower node for OCW operations"
                            );
                            return Some((node.clone(), node_signing_key));
                        }
                    } else {
                        log::debug!(
                            target: "runtime::watchtower::ocw",
                            "No signing key found for watchtower node"
                        );
                    }
                }
            }

            log::warn!(
                target: "runtime::watchtower::ocw",
                "No matching watchtower node found for local signing keys. This may indicate:\n\
                 1. Local keystore doesn't have signing keys for any registered watchtowers\n\
                 2. Registered watchtowers use different signing keys than what's in local keystore\n\
                 Local keys: {}, Authorized watchtowers: {}",
                local_keys.len(),
                authorized_watchtowers.len()
            );
            
            if !local_keys.is_empty() && !authorized_watchtowers.is_empty() {
                log::warn!(
                    target: "runtime::watchtower::ocw",
                    "Debug: Local keys available but no matches found. \
                     Consider checking if the node is properly registered with the correct signing key."
                );
            }
            
            None
        }

        pub fn offchain_signature_is_valid<D: Encode>(
            data: &D,
            signer: &T::SignerId,
            signature: &<T::SignerId as sp_runtime::RuntimeAppPublic>::Signature,
        ) -> bool {
            let signature_valid =
                data.using_encoded(|encoded_data| signer.verify(&encoded_data, &signature));

            log::trace!(
                target: "runtime::watchtower::ocw",
                "Validating OCW signature: Result: {}",
                signature_valid
            );
            signature_valid
        }

        pub fn get_voting_status(
            instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
        ) -> Option<(BlockNumberFor<T>, BlockNumberFor<T>, u32)> {
            let consensus_key = (instance, root_id.clone());
            
            if let Some(start_block) = VotingStartBlock::<T>::get(&consensus_key) {
                let deadline = start_block + VotingPeriod::<T>::get();
                let votes = IndividualWatchtowerVotes::<T>::get(instance, root_id);
                let vote_count = votes.len() as u32;
                
                Some((start_block, deadline, vote_count))
            } else {
                None
            }
        }

        pub fn get_voting_period() -> BlockNumberFor<T> {
            VotingPeriod::<T>::get()
        }

        pub fn is_voting_active(
            instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
        ) -> bool {
            let consensus_key = (instance, root_id.clone());
            
            if VoteConsensusReached::<T>::get(&consensus_key) {
                return false;
            }
            
            if let Some(start_block) = VotingStartBlock::<T>::get(&consensus_key) {
                let current_block = frame_system::Pallet::<T>::block_number();
                let deadline = start_block + VotingPeriod::<T>::get();
                current_block <= deadline
            } else {
                false
            }
        }

        pub fn cleanup_expired_votes(
            instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
        ) -> DispatchResult {
            let consensus_key = (instance, root_id.clone());
            let current_block = frame_system::Pallet::<T>::block_number();
            
            if let Some(start_block) = VotingStartBlock::<T>::get(&consensus_key) {
                let voting_deadline = start_block + VotingPeriod::<T>::get();
                if current_block > voting_deadline {
                    log::info!(
                        target: "runtime::watchtower",
                        "Cleaning up expired votes for {:?}. Current block: {:?}, deadline: {:?}",
                        consensus_key, current_block, voting_deadline
                    );
                    
                    IndividualWatchtowerVotes::<T>::remove(instance, &root_id);
                    VotingStartBlock::<T>::remove(&consensus_key);
                    return Ok(());
                }
            }
            
            Err(Error::<T>::VotingNotStarted.into())
        }
    }
}
