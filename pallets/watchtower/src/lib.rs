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
    RootId as PalletSummaryRootIdGeneric, SummaryStatus as PalletSummaryStatusGeneric,
};

use sp_core::H256;
use sp_runtime::{
    traits::{Dispatchable, SaturatedConversion, ValidateUnsigned},
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
pub const DEFAULT_VOTING_PERIOD_BLOCKS: u32 = 100;

pub const WATCHTOWER_CHALLENGE_CONTEXT: &[u8] = b"watchtower_challenge";
pub const WATCHTOWER_CHALLENGE_PROVIDE_TAG: &[u8] = b"WatchtowerChallengeProvideTag";

pub type AVN<T> = avn::Pallet<T>;

#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, Clone, Copy, RuntimeDebug)]
pub enum SummarySourceInstance {
    EthereumBridge,
    AnchorStorage,
}

#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, Clone, Copy, RuntimeDebug)]
pub enum ChallengeStatus {
    Pending,
    Accepted,
    Rejected,
}

#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, Clone, Copy, RuntimeDebug)]
pub enum ChallengeResolution {
    BadChallenge,        // Malicious challenge - nodes get punished
    InvalidChallenge,    // Good faith but invalid challenge - no punishment
    SuccessfulChallenge, // Valid challenge - summary should be rejected
}

#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, Clone, Copy, RuntimeDebug)]
pub enum ChallengeAdminTrigger {
    ConsensusReached,    // Consensus reached but challenges exist
    VotingPeriodExpired, // Voting period expired with pending challenges
}

pub type MaxChallengersBound = ConstU32<1000>;

#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, Clone, RuntimeDebug)]
pub struct ChallengeInfo<AccountId, Hash> {
    pub incorrect_root_id: Hash,
    pub correct_root_hash: Hash,
    pub challengers: BoundedVec<AccountId, MaxChallengersBound>,
    pub status: ChallengeStatus,
    pub created_block: u32,
    pub first_challenge_alert_sent: bool,
    pub original_consensus: Option<WatchtowerSummaryStatus>,
}

pub trait ChallengeRewardInterface<AccountId> {
    fn get_failed_challenge_count(node: &AccountId) -> u32;
    fn reset_failed_challenge_count(node: &AccountId);
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

pub trait NodeManagerInterface<AccountId, SignerId, MaxWatchtowers: Get<u32>> {
    fn get_authorized_watchtowers() -> Result<BoundedVec<AccountId, MaxWatchtowers>, DispatchError>;

    fn is_authorized_watchtower(who: &AccountId) -> bool;

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

        type SummaryServiceProvider: SummaryServices<Self>;
        type NodeManager: NodeManagerInterface<
            Self::AccountId,
            Self::SignerId,
            Self::MaxWatchtowers,
        >;
        type MaxWatchtowers: Get<u32>;
        /// The origin that is allowed to resolve challenges by default
        type ChallengeResolutionOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    }

    #[pallet::storage]
    #[pallet::getter(fn individual_votes)]
    pub type IndividualWatchtowerVotes<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        SummarySourceInstance,
        Blake2_128Concat,
        WatchtowerRootId<BlockNumberFor<T>>,
        BoundedVec<(T::AccountId, bool), T::MaxWatchtowers>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn consensus_reached_flag)]
    pub type VoteConsensusReached<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        (SummarySourceInstance, WatchtowerRootId<BlockNumberFor<T>>),
        bool,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn voting_start_block)]
    pub type VotingStartBlock<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        (SummarySourceInstance, WatchtowerRootId<BlockNumberFor<T>>),
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
        (SummarySourceInstance, WatchtowerRootId<BlockNumberFor<T>>),
        WatchtowerOnChainHash,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn challenges)]
    pub type Challenges<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        (SummarySourceInstance, WatchtowerRootId<BlockNumberFor<T>>),
        crate::ChallengeInfo<T::AccountId, WatchtowerOnChainHash>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn failed_challenge_count)]
    pub type FailedChallengeCount<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn total_challenge_count)]
    pub type TotalChallengeCount<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn challenge_resolution_admin)]
    pub type ChallengeResolutionAdmin<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

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
        ChallengeSubmitted {
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            incorrect_root_id: WatchtowerOnChainHash,
            correct_root_hash: WatchtowerOnChainHash,
            challenger: T::AccountId,
            challenge_count: u32,
        },
        ChallengeAccepted {
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            challengers: BoundedVec<T::AccountId, MaxChallengersBound>,
        },
        ChallengeResolved {
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            resolution: ChallengeResolution,
            challengers: BoundedVec<T::AccountId, MaxChallengersBound>,
        },
        FirstChallengeAlert {
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
        },
        ChallengesPresentedToAdmin {
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            challenge_count: u32,
            trigger: ChallengeAdminTrigger,
        },
        SummaryAcceptedWithoutConsensus {
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            positive_votes: u32,
            required_votes: u32,
        },
        ChallengeResolutionAdminUpdated {
            old_admin: Option<T::AccountId>,
            new_admin: Option<T::AccountId>,
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
        ChallengeAlreadyExists,
        ChallengeNotFound,
        AlreadyChallenged,
        ChallengeAlreadyResolved,
        TooManyChallengers,
        InvalidChallengeResolutionAdmin,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(block_number: BlockNumberFor<T>) {
            log::debug!(target: "runtime::watchtower::ocw", "Watchtower OCW running for block {:?}", block_number);

            Self::process_pending_validations();
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::submit_watchtower_vote())]
        pub fn submit_watchtower_vote(
            origin: OriginFor<T>,
            node: T::AccountId,
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            vote_is_valid: bool,
            _signature: <T::SignerId as sp_runtime::RuntimeAppPublic>::Signature,
        ) -> DispatchResult {
            ensure_none(origin).map_err(|e| e)?;

            ensure!(T::NodeManager::is_authorized_watchtower(&node), {
                Error::<T>::NotAuthorizedWatchtower
            });

            ensure!(vote_is_valid, Error::<T>::InvalidVerificationSubmission);

            Self::internal_submit_vote(node.clone(), summary_instance, root_id.clone(), true)
                .map_err(|e| e)?;

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::set_voting_period())]
        pub fn set_voting_period(
            origin: OriginFor<T>,
            new_period: BlockNumberFor<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let min_period: BlockNumberFor<T> = 10u32.into();
            ensure!(new_period >= min_period, Error::<T>::InvalidVotingPeriod);

            let old_period = VotingPeriod::<T>::get();
            VotingPeriod::<T>::put(new_period);

            Self::deposit_event(Event::VotingPeriodUpdated { old_period, new_period });

            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::set_voting_period())]
        pub fn set_challenge_resolution_admin(
            origin: OriginFor<T>,
            new_admin: Option<T::AccountId>,
        ) -> DispatchResult {
            T::ChallengeResolutionOrigin::ensure_origin(origin)?;

            let old_admin = ChallengeResolutionAdmin::<T>::get();

            match new_admin.clone() {
                Some(admin) => ChallengeResolutionAdmin::<T>::put(&admin),
                None => ChallengeResolutionAdmin::<T>::kill(),
            }

            Self::deposit_event(Event::ChallengeResolutionAdminUpdated { old_admin, new_admin });

            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::resolve_challenge())]
        pub fn resolve_challenge(
            origin: OriginFor<T>,
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            resolution: ChallengeResolution,
        ) -> DispatchResult {
            if let Some(admin) = ChallengeResolutionAdmin::<T>::get() {
                let who = ensure_signed(origin)?;
                ensure!(who == admin, Error::<T>::InvalidChallengeResolutionAdmin);
            } else {
                T::ChallengeResolutionOrigin::ensure_origin(origin)?;
            }

            Self::internal_resolve_challenge(summary_instance, root_id, resolution)
        }

        #[pallet::call_index(4)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::submit_challenge())]
        pub fn submit_challenge(
            origin: OriginFor<T>,
            challenger: T::AccountId,
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            incorrect_root_id: WatchtowerOnChainHash,
            correct_root_hash: WatchtowerOnChainHash,
            _signature: <T::SignerId as sp_runtime::RuntimeAppPublic>::Signature,
        ) -> DispatchResult {
            ensure_none(origin)?;

            ensure!(
                T::NodeManager::is_authorized_watchtower(&challenger),
                Error::<T>::NotAuthorizedWatchtower
            );

            Self::internal_submit_challenge(
                challenger,
                summary_instance,
                root_id,
                incorrect_root_id,
                correct_root_hash,
            )
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::submit_watchtower_vote {
                    node,
                    summary_instance,
                    root_id,
                    vote_is_valid,
                    signature,
                } => {
                    if !T::NodeManager::is_authorized_watchtower(node) {
                        return InvalidTransaction::Call.into();
                    }

                    let signing_key = match T::NodeManager::get_node_signing_key(node) {
                        Some(key) => key,
                        None => {
                            return InvalidTransaction::Call.into();
                        },
                    };

                    if Self::offchain_signature_is_valid(
                        &(WATCHTOWER_OCW_CONTEXT, summary_instance, root_id, vote_is_valid),
                        &signing_key,
                        signature,
                    ) {
                        if !vote_is_valid {
                            return InvalidTransaction::Call.into();
                        }

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
                },
                Call::submit_challenge {
                    challenger,
                    summary_instance,
                    root_id,
                    incorrect_root_id,
                    correct_root_hash,
                    signature,
                } => {
                    if !T::NodeManager::is_authorized_watchtower(challenger) {
                        return InvalidTransaction::Call.into();
                    }

                    let signing_key = match T::NodeManager::get_node_signing_key(challenger) {
                        Some(key) => key,
                        None => {
                            return InvalidTransaction::Call.into();
                        },
                    };

                    if Self::offchain_signature_is_valid(
                        &(
                            WATCHTOWER_CHALLENGE_CONTEXT,
                            summary_instance,
                            root_id,
                            incorrect_root_id,
                            correct_root_hash,
                        ),
                        &signing_key,
                        signature,
                    ) {
                        let current_block = frame_system::Pallet::<T>::block_number();
                        let unique_payload_for_provides = (
                            WATCHTOWER_CHALLENGE_PROVIDE_TAG,
                            challenger.clone(),
                            *summary_instance,
                            root_id.clone(),
                            *incorrect_root_id,
                            *correct_root_hash,
                            current_block,
                            source,
                            signature.encode()[0..8].to_vec(),
                        );

                        let provides_tag = unique_payload_for_provides.encode();

                        ValidTransaction::with_tag_prefix("WatchtowerChallenge")
                            .priority(TransactionPriority::MAX)
                            .and_provides(vec![provides_tag])
                            .longevity(64_u64)
                            .propagate(true)
                            .build()
                    } else {
                        InvalidTransaction::BadSigner.into()
                    }
                },
                _ => InvalidTransaction::Call.into(),
            }
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn notify_summary_ready_for_validation(
            instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            root_hash: WatchtowerOnChainHash,
        ) -> DispatchResult {
            let consensus_key = (instance, root_id.clone());

            if VoteConsensusReached::<T>::get(&consensus_key) {
                return Ok(());
            }

            if VotingStartBlock::<T>::get(&consensus_key).is_some() {
                return Ok(());
            }

            if root_hash == sp_core::H256::zero() {
                log::error!(
                    target: "runtime::watchtower::notification",
                    "Received invalid zero root hash for summary: {:?}",
                    consensus_key
                );
                return Err(DispatchError::Other("InvalidRootHash"));
            }

            VotingStartBlock::<T>::insert(
                &consensus_key,
                frame_system::Pallet::<T>::block_number(),
            );
            PendingValidationRootHash::<T>::insert(&consensus_key, root_hash);

            Ok(())
        }

        fn process_pending_validations() {
            let current_block = frame_system::Pallet::<T>::block_number();
            let voting_period = VotingPeriod::<T>::get();

            for (consensus_key, start_block) in VotingStartBlock::<T>::iter() {
                let (instance, root_id) = consensus_key.clone();

                if VoteConsensusReached::<T>::get(&consensus_key) {
                    continue;
                }

                if current_block > start_block + voting_period {
                    log::warn!(
                        target: "runtime::watchtower::ocw",
                        "Voting period expired for {:?}, skipping OCW validation",
                        consensus_key
                    );
                    continue;
                }

                if let Some(root_hash) = PendingValidationRootHash::<T>::get(&consensus_key) {
                    log::info!(
                        target: "runtime::watchtower::ocw",
                        "Processing OCW validation for {:?}, root_hash: {:?}",
                        consensus_key, root_hash
                    );

                    Self::perform_ocw_recalculation(instance, root_id, root_hash);

                    PendingValidationRootHash::<T>::remove(&consensus_key);
                } else {
                    log::warn!(
                        target: "runtime::watchtower::ocw",
                        "No root hash found for active voting period: {:?}",
                        consensus_key
                    );
                }
            }
        }

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
                },
            };

            match Self::try_ocw_process_recalculation(instance, root_id.clone(), onchain_root_hash)
            {
                Ok((recalculated_hash_matches, recalculated_hash)) => {
                    if recalculated_hash_matches {
                        if let Err(e) = Self::submit_ocw_vote(
                            node.clone(),
                            signing_key.clone(),
                            instance,
                            root_id.clone(),
                            true,
                        ) {
                            log::error!(
                                target: "runtime::watchtower::ocw",
                                "Failed to submit OCW vote for {:?} from instance {:?}: {:?}",
                                root_id, instance, e
                            );
                        }
                    } else {
                        if let Err(e) = Self::submit_ocw_challenge(
                            node,
                            signing_key,
                            instance,
                            root_id.clone(),
                            onchain_root_hash, // incorrect_root_id
                            recalculated_hash, // correct_root_hash
                        ) {
                            log::error!(
                                target: "runtime::watchtower::ocw",
                                "Failed to submit OCW challenge for {:?} from instance {:?}: {:?}",
                                root_id, instance, e
                            );
                        }
                    }
                },
                Err(e) => {
                    Self::deposit_event(Event::VerificationProcessingError {
                        summary_instance: instance,
                        root_id,
                        reason: e,
                    });
                },
            }
        }

        fn try_ocw_process_recalculation(
            instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            on_chain_hash: WatchtowerOnChainHash,
        ) -> Result<(bool, WatchtowerOnChainHash), VerificationError> {
            let mut lock_identifier_vec = OCW_LOCK_PREFIX.to_vec();
            lock_identifier_vec.extend_from_slice(&instance.encode());
            lock_identifier_vec.extend_from_slice(&root_id.encode());

            let mut lock = AVN::<T>::get_ocw_locker(&lock_identifier_vec);

            let result: Result<(bool, WatchtowerOnChainHash), VerificationError> =
                match lock.try_lock() {
                    Ok(guard) => {
                        match Self::fetch_recalculated_root_hash_sync(
                            root_id.range.from_block,
                            root_id.range.to_block,
                        ) {
                            Ok(recalculated_hash) => {
                                guard.forget();
                                Ok((recalculated_hash == on_chain_hash, recalculated_hash))
                            },
                            Err(_e) => {
                                Self::deposit_event(Event::VerificationProcessingError {
                                    summary_instance: instance,
                                    root_id,
                                    reason: VerificationError::HttpCallFailed,
                                });
                                Err(VerificationError::HttpCallFailed)
                            },
                        }
                    },
                    Err(_lock_error) => {
                        Self::deposit_event(Event::VerificationProcessingError {
                            summary_instance: instance,
                            root_id,
                            reason: VerificationError::LockAcquisitionFailed,
                        });
                        Err(VerificationError::LockAcquisitionFailed)
                    },
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

            let response = AVN::<T>::get_data_from_service(url_path).map_err(|dispatch_err| {
                let err_msg = format!("AVN service call failed: {:?}", dispatch_err);
                err_msg
            })?;

            Self::validate_response(response)
                .map_err(|e| format!("Response validation failed: {:?}", e))
        }

        pub fn validate_response(
            response: Vec<u8>,
        ) -> Result<WatchtowerOnChainHash, DispatchError> {
            if response.len() != 64 {
                return Err(DispatchError::Other("InvalidRootHashLength"));
            }

            let root_hash_str = core::str::from_utf8(&response)
                .map_err(|_| DispatchError::Other("InvalidUTF8Bytes"))?;

            let mut data: [u8; 32] = [0; 32];
            hex::decode_to_slice(root_hash_str.trim(), &mut data[..])
                .map_err(|_| DispatchError::Other("InvalidHexString"))?;

            Ok(H256::from_slice(&data))
        }

        fn submit_ocw_vote(
            node: T::AccountId,
            signing_key: T::SignerId,
            instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            vote_is_valid: bool,
        ) -> Result<(), &'static str> {
            let consensus_key = (instance, root_id.clone());
            if VoteConsensusReached::<T>::get(&consensus_key) {
                return Ok(());
            }

            // Check if voting period has expired and handle accordingly
            if Self::is_voting_period_expired(instance, root_id.clone()) {
                return Self::handle_expired_voting_period(instance, root_id)
                    .map_err(|_| "Failed to handle expired voting period");
            }

            let data_to_sign = (WATCHTOWER_OCW_CONTEXT, &instance, &root_id, vote_is_valid);
            let signature = match signing_key.sign(&data_to_sign.encode()) {
                Some(sig) => sig,
                None => {
                    return Err("Failed to sign vote data");
                },
            };

            let call = Call::submit_watchtower_vote {
                node: node.clone(),
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

        fn submit_ocw_challenge(
            node: T::AccountId,
            signing_key: T::SignerId,
            instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            incorrect_root_id: WatchtowerOnChainHash,
            correct_root_hash: WatchtowerOnChainHash,
        ) -> Result<(), &'static str> {
            let challenge_key = (instance, root_id.clone());
            if let Some(existing_challenge) = Challenges::<T>::get(&challenge_key) {
                if existing_challenge.challengers.iter().any(|c| c == &node) {
                    return Ok(());
                }

                if existing_challenge.status != ChallengeStatus::Pending {
                    return Ok(());
                }
            }

            let data_to_sign = (
                WATCHTOWER_CHALLENGE_CONTEXT,
                &instance,
                &root_id,
                &incorrect_root_id,
                &correct_root_hash,
            );
            let signature = match signing_key.sign(&data_to_sign.encode()) {
                Some(sig) => sig,
                None => {
                    return Err("Failed to sign challenge data");
                },
            };

            let call = Call::submit_challenge {
                challenger: node.clone(),
                summary_instance: instance,
                root_id: root_id.clone(),
                incorrect_root_id,
                correct_root_hash,
                signature,
            };

            match SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()) {
                Ok(()) => Ok(()),
                Err(_e) => Err("Failed to submit challenge transaction to local pool"),
            }
        }

        pub fn try_reach_consensus(
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
                    let challenge_key = (summary_instance, root_id.clone());
                    if let Some(challenge_info) = Challenges::<T>::get(&challenge_key) {
                        if challenge_info.status == ChallengeStatus::Pending ||
                            challenge_info.status == ChallengeStatus::Accepted
                        {
                            VoteConsensusReached::<T>::insert(&consensus_key, true);

                            let authorized_watchtowers =
                                T::NodeManager::get_authorized_watchtowers()
                                    .map_err(|_| Error::<T>::FailedToGetAuthorizedWatchtowers)?;
                            let total_authorized_watchtowers = authorized_watchtowers.len() as u32;
                            let required_for_consensus = (total_authorized_watchtowers * 2 + 2) / 3;
                            let current_votes = IndividualWatchtowerVotes::<T>::get(
                                summary_instance,
                                root_id.clone(),
                            );
                            let positive_votes = current_votes
                                .iter()
                                .filter(|(voter, vote)| {
                                    *vote && authorized_watchtowers.contains(voter)
                                })
                                .count() as u32;

                            let implied_consensus = if positive_votes >= required_for_consensus {
                                WatchtowerSummaryStatus::Accepted
                            } else {
                                WatchtowerSummaryStatus::PendingChallengeResolution
                            };

                            // Store the implied consensus for later restoration if challenges are
                            // invalid
                            Challenges::<T>::mutate(&challenge_key, |maybe_challenge| {
                                if let Some(challenge) = maybe_challenge {
                                    challenge.original_consensus = Some(implied_consensus.clone());
                                }
                            });

                            Self::deposit_event(Event::ChallengesPresentedToAdmin {
                                summary_instance,
                                root_id: root_id.clone(),
                                challenge_count: challenge_info.challengers.len() as u32,
                                trigger: ChallengeAdminTrigger::VotingPeriodExpired,
                            });

                            Self::deposit_event(Event::WatchtowerConsensusReached {
                                summary_instance,
                                root_id,
                                consensus_result: implied_consensus.clone(),
                            });

                            T::SummaryServiceProvider::update_summary_status(
                                summary_instance,
                                root_id.clone(),
                                implied_consensus,
                            )
                            .map_err(|_e| Error::<T>::SummaryUpdateFailed)?;

                            VotingStartBlock::<T>::remove(&consensus_key);
                            PendingValidationRootHash::<T>::remove(&consensus_key);

                            return Ok(());
                        }
                    }

                    // No pending challenges - standard expiry handling
                    IndividualWatchtowerVotes::<T>::remove(summary_instance, &root_id);
                    VotingStartBlock::<T>::remove(&consensus_key);
                    PendingValidationRootHash::<T>::remove(&consensus_key);
                    return Err(Error::<T>::VotingPeriodExpired.into());
                }
            }

            let authorized_watchtowers = T::NodeManager::get_authorized_watchtowers()
                .map_err(|_| Error::<T>::FailedToGetAuthorizedWatchtowers)?;

            let total_authorized_watchtowers = authorized_watchtowers.len() as u32;
            let required_for_consensus = (total_authorized_watchtowers * 2 + 2) / 3;

            let current_votes =
                IndividualWatchtowerVotes::<T>::get(summary_instance, root_id.clone());

            let mut valid_votes = Vec::new();
            for (voter, vote) in current_votes.iter() {
                if authorized_watchtowers.contains(voter) {
                    valid_votes.push((voter.clone(), *vote));
                }
            }

            let total_votes = valid_votes.len() as u32;

            if total_votes == 0 {
                return Ok(());
            }

            let positive_votes = valid_votes.iter().filter(|(_, vote)| *vote).count() as u32;

            let consensus_reached = positive_votes >= required_for_consensus;

            if consensus_reached {
                VoteConsensusReached::<T>::insert(&consensus_key, true);

                Self::deposit_event(Event::WatchtowerConsensusReached {
                    summary_instance,
                    root_id: root_id.clone(),
                    consensus_result: WatchtowerSummaryStatus::Accepted,
                });

                // Check if there are any pending challenges that need admin notification
                let challenge_key = (summary_instance, root_id.clone());
                if let Some(challenge_info) = Challenges::<T>::get(&challenge_key) {
                    if challenge_info.status == ChallengeStatus::Pending ||
                        challenge_info.status == ChallengeStatus::Accepted
                    {
                        Self::deposit_event(Event::ChallengesPresentedToAdmin {
                            summary_instance,
                            root_id: root_id.clone(),
                            challenge_count: challenge_info.challengers.len() as u32,
                            trigger: ChallengeAdminTrigger::ConsensusReached,
                        });

                        // Store the consensus result for potential future reference
                        Challenges::<T>::mutate(&challenge_key, |maybe_challenge| {
                            if let Some(challenge) = maybe_challenge {
                                challenge.original_consensus =
                                    Some(WatchtowerSummaryStatus::Accepted);
                            }
                        });
                    }
                }

                // Consensus is always acceptance (watchtowers can only vote positively)
                T::SummaryServiceProvider::update_summary_status(
                    summary_instance,
                    root_id.clone(),
                    WatchtowerSummaryStatus::Accepted,
                )
                .map_err(|_e| Error::<T>::SummaryUpdateFailed)?;

                VotingStartBlock::<T>::remove(&consensus_key);
                PendingValidationRootHash::<T>::remove(&consensus_key);
            }

            Ok(())
        }

        fn internal_submit_vote(
            voter: T::AccountId,
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            vote_is_valid: bool,
        ) -> DispatchResult {
            let consensus_key = (summary_instance, root_id.clone());
            let current_block = frame_system::Pallet::<T>::block_number();

            ensure!(!VoteConsensusReached::<T>::get(&consensus_key), {
                Error::<T>::ConsensusAlreadyReached
            });

            let voting_start_block = VotingStartBlock::<T>::get(&consensus_key);
            if let Some(start_block) = voting_start_block {
                let voting_deadline = start_block + VotingPeriod::<T>::get();
                if current_block > voting_deadline {
                    return Err(Error::<T>::VotingPeriodExpired.into());
                }
            } else {
                VotingStartBlock::<T>::insert(&consensus_key, current_block);
            }

            IndividualWatchtowerVotes::<T>::try_mutate(
                summary_instance,
                root_id.clone(),
                |votes| -> DispatchResult {
                    if votes.iter().any(|(acc, _)| acc == &voter) {
                        return Err(Error::<T>::AlreadyVoted.into());
                    }

                    votes
                        .try_push((voter.clone(), vote_is_valid))
                        .map_err(|_| Error::<T>::TooManyVotes)?;

                    Ok(())
                },
            )?;

            Self::deposit_event(Event::WatchtowerVoteSubmitted {
                voter: voter.clone(),
                summary_instance,
                root_id: root_id.clone(),
                vote_is_valid,
            });

            Self::try_reach_consensus(summary_instance, root_id.clone()).map_err(|e| e)?;

            Ok(())
        }

        fn get_node_from_signing_key() -> Option<(T::AccountId, T::SignerId)> {
            let local_keys: Vec<T::SignerId> = T::SignerId::all();

            let authorized_watchtowers = match T::NodeManager::get_authorized_watchtowers() {
                Ok(watchtowers) => watchtowers,
                Err(_) => {
                    return None;
                },
            };

            for local_key in local_keys.iter() {
                for node in authorized_watchtowers.iter() {
                    if let Some(node_signing_key) = T::NodeManager::get_node_signing_key(node) {
                        if *local_key == node_signing_key {
                            return Some((node.clone(), node_signing_key));
                        }
                    } else {
                    }
                }
            }

            if !local_keys.is_empty() && !authorized_watchtowers.is_empty() {}

            None
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

        fn is_voting_period_expired(
            instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
        ) -> bool {
            let consensus_key = (instance, root_id);
            if let Some(start_block) = VotingStartBlock::<T>::get(&consensus_key) {
                let current_block = frame_system::Pallet::<T>::block_number();
                let voting_deadline = start_block + VotingPeriod::<T>::get();
                current_block > voting_deadline
            } else {
                false
            }
        }

        fn handle_expired_voting_period(
            instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
        ) -> DispatchResult {
            let challenge_key = (instance, root_id.clone());

            if let Some(challenge_info) = Challenges::<T>::get(&challenge_key) {
                if challenge_info.status == ChallengeStatus::Pending ||
                    challenge_info.status == ChallengeStatus::Accepted
                {
                    return Self::handle_expired_voting_with_challenges(
                        instance,
                        root_id,
                        challenge_info,
                    );
                }
            }

            // No pending challenges - accept by default but notify admin if insufficient consensus
            Self::handle_expired_voting_without_challenges(instance, root_id)
        }

        fn handle_expired_voting_with_challenges(
            instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            challenge_info: crate::ChallengeInfo<T::AccountId, WatchtowerOnChainHash>,
        ) -> DispatchResult {
            let consensus_key = (instance, root_id.clone());
            let challenge_key = (instance, root_id.clone());

            VoteConsensusReached::<T>::insert(&consensus_key, true);

            let authorized_watchtowers = T::NodeManager::get_authorized_watchtowers()
                .map_err(|_| Error::<T>::FailedToGetAuthorizedWatchtowers)?;
            let total_authorized_watchtowers = authorized_watchtowers.len() as u32;
            let required_for_consensus = (total_authorized_watchtowers * 2 + 2) / 3;
            let current_votes = IndividualWatchtowerVotes::<T>::get(instance, root_id.clone());
            let positive_votes = current_votes
                .iter()
                .filter(|(voter, vote)| *vote && authorized_watchtowers.contains(voter))
                .count() as u32;

            let consensus_reached = positive_votes >= required_for_consensus;

            let implied_consensus = if consensus_reached {
                WatchtowerSummaryStatus::Accepted
            } else {
                WatchtowerSummaryStatus::PendingChallengeResolution
            };

            Challenges::<T>::mutate(&challenge_key, |maybe_challenge| {
                if let Some(challenge) = maybe_challenge {
                    challenge.original_consensus = Some(implied_consensus.clone());
                }
            });

            Self::deposit_event(Event::ChallengesPresentedToAdmin {
                summary_instance: instance.clone(),
                root_id: root_id.clone(),
                challenge_count: challenge_info.challengers.len() as u32,
                trigger: ChallengeAdminTrigger::VotingPeriodExpired,
            });

            Self::deposit_event(Event::WatchtowerConsensusReached {
                summary_instance: instance.clone(),
                root_id: root_id.clone(),
                consensus_result: implied_consensus.clone(),
            });

            T::SummaryServiceProvider::update_summary_status(
                instance,
                root_id.clone(),
                implied_consensus,
            )
            .map_err(|_e| Error::<T>::SummaryUpdateFailed)?;

            VotingStartBlock::<T>::remove(&consensus_key);
            PendingValidationRootHash::<T>::remove(&consensus_key);

            Ok(())
        }

        fn handle_expired_voting_without_challenges(
            instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
        ) -> DispatchResult {
            let consensus_key = (instance, root_id.clone());

            VoteConsensusReached::<T>::insert(&consensus_key, true);

            // Calculate if we had consensus through positive votes
            let authorized_watchtowers = T::NodeManager::get_authorized_watchtowers()
                .map_err(|_| Error::<T>::FailedToGetAuthorizedWatchtowers)?;
            let total_authorized_watchtowers = authorized_watchtowers.len() as u32;
            let required_for_consensus = (total_authorized_watchtowers * 2 + 2) / 3;
            let current_votes = IndividualWatchtowerVotes::<T>::get(instance, root_id.clone());
            let positive_votes = current_votes
                .iter()
                .filter(|(voter, vote)| *vote && authorized_watchtowers.contains(voter))
                .count() as u32;

            let consensus_reached = positive_votes >= required_for_consensus;

            // If we didn't reach consensus through positive votes, notify admin
            if !consensus_reached {
                Self::deposit_event(Event::SummaryAcceptedWithoutConsensus {
                    summary_instance: instance,
                    root_id: root_id.clone(),
                    positive_votes,
                    required_votes: required_for_consensus,
                });
            }

            Self::deposit_event(Event::WatchtowerConsensusReached {
                summary_instance: instance,
                root_id: root_id.clone(),
                consensus_result: WatchtowerSummaryStatus::Accepted,
            });

            T::SummaryServiceProvider::update_summary_status(
                instance,
                root_id.clone(),
                WatchtowerSummaryStatus::Accepted,
            )
            .map_err(|_e| Error::<T>::SummaryUpdateFailed)?;

            VotingStartBlock::<T>::remove(&consensus_key);
            PendingValidationRootHash::<T>::remove(&consensus_key);

            Ok(())
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
                    IndividualWatchtowerVotes::<T>::remove(instance, &root_id);
                    VotingStartBlock::<T>::remove(&consensus_key);
                    return Ok(());
                }
            }

            Err(Error::<T>::VotingNotStarted.into())
        }

        fn internal_submit_challenge(
            challenger: T::AccountId,
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            incorrect_root_id: WatchtowerOnChainHash,
            correct_root_hash: WatchtowerOnChainHash,
        ) -> DispatchResult {
            let challenge_key = (summary_instance, root_id.clone());
            let current_block = frame_system::Pallet::<T>::block_number();
            let current_block_u32 = current_block.saturated_into();

            let mut challenge_info =
                Challenges::<T>::get(&challenge_key).unwrap_or_else(|| crate::ChallengeInfo {
                    incorrect_root_id,
                    correct_root_hash,
                    challengers: BoundedVec::new(),
                    status: ChallengeStatus::Pending,
                    created_block: current_block_u32,
                    first_challenge_alert_sent: false,
                    original_consensus: None,
                });

            ensure!(
                challenge_info.status == ChallengeStatus::Pending ||
                    challenge_info.status == ChallengeStatus::Accepted,
                Error::<T>::ChallengeAlreadyResolved
            );

            ensure!(
                !challenge_info.challengers.iter().any(|c| c == &challenger),
                Error::<T>::AlreadyChallenged
            );

            challenge_info
                .challengers
                .try_push(challenger.clone())
                .map_err(|_| Error::<T>::TooManyChallengers)?;

            let current_total = TotalChallengeCount::<T>::get(&challenger);
            TotalChallengeCount::<T>::insert(&challenger, current_total + 1);

            let challenge_count = challenge_info.challengers.len() as u32;

            // Use the same threshold as consensus (2/3 majority)
            let authorized_watchtowers = T::NodeManager::get_authorized_watchtowers()
                .map_err(|_| Error::<T>::FailedToGetAuthorizedWatchtowers)?;
            let total_authorized_watchtowers = authorized_watchtowers.len() as u32;
            let challenge_threshold = (total_authorized_watchtowers * 2 + 2) / 3;

            if challenge_count == 1 && !challenge_info.first_challenge_alert_sent {
                challenge_info.first_challenge_alert_sent = true;
                Self::deposit_event(Event::FirstChallengeAlert {
                    summary_instance,
                    root_id: root_id.clone(),
                });

                // TODO: In the future, this could trigger a Slack alert via an offchain worker
                // or integration with external alerting systems
            }

            Self::deposit_event(Event::ChallengeSubmitted {
                summary_instance,
                root_id: root_id.clone(),
                incorrect_root_id,
                correct_root_hash,
                challenger,
                challenge_count,
            });

            if challenge_info.status == ChallengeStatus::Pending &&
                challenge_count >= challenge_threshold
            {
                challenge_info.status = ChallengeStatus::Accepted;

                Self::deposit_event(Event::ChallengeAccepted {
                    summary_instance,
                    root_id: root_id.clone(),
                    challengers: challenge_info.challengers.clone(),
                });
            }

            Challenges::<T>::insert(&challenge_key, challenge_info);

            Ok(())
        }

        fn internal_resolve_challenge(
            summary_instance: SummarySourceInstance,
            root_id: WatchtowerRootId<BlockNumberFor<T>>,
            resolution: ChallengeResolution,
        ) -> DispatchResult {
            let challenge_key = (summary_instance, root_id.clone());

            let mut challenge_info =
                Challenges::<T>::get(&challenge_key).ok_or(Error::<T>::ChallengeNotFound)?;

            ensure!(
                challenge_info.status == ChallengeStatus::Pending ||
                    challenge_info.status == ChallengeStatus::Accepted,
                Error::<T>::ChallengeAlreadyResolved
            );

            challenge_info.status = ChallengeStatus::Rejected;

            match resolution {
                ChallengeResolution::BadChallenge => {
                    // Challenge was malicious - punish challengers and restore original consensus
                    for challenger in &challenge_info.challengers {
                        let current_failed = FailedChallengeCount::<T>::get(challenger);
                        FailedChallengeCount::<T>::insert(challenger, current_failed + 1);
                    }
                    // Restore the original consensus (usually Accepted)
                    let final_status = challenge_info
                        .original_consensus
                        .unwrap_or(WatchtowerSummaryStatus::Accepted);
                    T::SummaryServiceProvider::update_summary_status(
                        summary_instance,
                        root_id.clone(),
                        final_status,
                    )
                    .map_err(|_e| Error::<T>::SummaryUpdateFailed)?;
                },
                ChallengeResolution::InvalidChallenge => {
                    // Challenge was good faith but incorrect - no punishment, restore original
                    // consensus
                    let final_status = challenge_info
                        .original_consensus
                        .unwrap_or(WatchtowerSummaryStatus::Accepted);
                    T::SummaryServiceProvider::update_summary_status(
                        summary_instance,
                        root_id.clone(),
                        final_status,
                    )
                    .map_err(|_e| Error::<T>::SummaryUpdateFailed)?;
                },
                ChallengeResolution::SuccessfulChallenge => {
                    // Challenge was valid - the original consensus was wrong, reject the summary
                    T::SummaryServiceProvider::update_summary_status(
                        summary_instance,
                        root_id.clone(),
                        WatchtowerSummaryStatus::Rejected,
                    )
                    .map_err(|_e| Error::<T>::SummaryUpdateFailed)?;
                },
            }

            // TODO: Add logic for successful challenges when challenge is proven correct
            // This would happen if the challenge leads to blocking/invalidating a bad root
            // For now, we only handle resolution of challenges that are deemed incorrect

            Self::deposit_event(Event::ChallengeResolved {
                summary_instance,
                root_id: root_id.clone(),
                resolution,
                challengers: challenge_info.challengers.clone(),
            });

            Challenges::<T>::remove(&challenge_key);

            Ok(())
        }
    }

    impl<T: Config> ChallengeRewardInterface<T::AccountId> for Pallet<T> {
        fn get_failed_challenge_count(node: &T::AccountId) -> u32 {
            FailedChallengeCount::<T>::get(node)
        }

        fn reset_failed_challenge_count(node: &T::AccountId) {
            FailedChallengeCount::<T>::remove(node);
        }
    }
}
