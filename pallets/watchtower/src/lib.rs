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
    WeightInfo,
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
use parity_scale_codec::{Decode, Encode};
pub use sp_avn_common::watchtower::*;
use sp_core::{MaxEncodedLen, H256};
pub use sp_runtime::{
    traits::{AtLeast32Bit, Dispatchable, ValidateUnsigned},
    transaction_validity::{
        InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity,
        ValidTransaction,
    },
    Perbill,
};
use sp_std::prelude::*;

pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);
pub const DEFAULT_VOTING_PERIOD_BLOCKS: u32 = 100;

pub mod types;
pub use types::*;

pub mod queue;
pub use queue::*;

pub use pallet::*;
#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: SendTransactionTypes<Call<Self>> + frame_system::Config {
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

        /// A trait to notify the result of voting
        type VoteStatusNotifier: VoteStatusNotifier;

        /// Access control for “external” (non-pallet-originated) proposals.
        type ExternalProposerOrigin: EnsureOrigin<
            Self::RuntimeOrigin,
            Success = Option<Self::AccountId>,
        >;

        /// The SignerId type used in NodeManager
        type SignerId: Member + Parameter + sp_runtime::RuntimeAppPublic + Ord + MaxEncodedLen;

        /// Interface for accessing NodeManager pallet functionality
        type NodeManager: NodeManagerInterface<Self::AccountId, Self::SignerId>;

        /// Hooks for other pallets to implement custom logic on certain events
        type WatchtowerHooks: WatchtowerHooks<Proposal<Self>>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;

        /// Minimum allowed voting period in blocks
        #[pallet::constant]
        type MinVotingPeriod: Get<BlockNumberFor<Self>>;

        /// Maximum proposal title length
        #[pallet::constant]
        type MaxTitleLen: Get<u32>;

        /// Maximum length of inline proposal data
        #[pallet::constant]
        type MaxInlineLen: Get<u32>;

        /// Maximum length of URI for proposals
        #[pallet::constant]
        type MaxUriLen: Get<u32>;

        /// Maximum length of Internal proposals
        #[pallet::constant]
        type MaxInternalProposalLen: Get<u32>;
    }

    #[pallet::type_value]
    pub fn DefaultVotingPeriod<T: Config>() -> BlockNumberFor<T> {
        DEFAULT_VOTING_PERIOD_BLOCKS.into()
    }

    #[pallet::storage]
    #[pallet::getter(fn voting_period)]
    pub type VotingPeriod<T: Config> =
        StorageValue<_, BlockNumberFor<T>, ValueQuery, DefaultVotingPeriod<T>>;

    #[pallet::storage]
    #[pallet::getter(fn id_by_external_ref)]
    pub type ExternalRef<T: Config> = StorageMap<_, Blake2_128Concat, H256, ProposalId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn proposals)]
    pub type Proposals<T: Config> =
        StorageMap<_, Blake2_128Concat, ProposalId, Proposal<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn proposal_status)]
    pub type ProposalStatus<T: Config> =
        StorageMap<_, Blake2_128Concat, ProposalId, ProposalStatusEnum, ValueQuery>;

    /// The currently active internal proposal being voted on, if any
    #[pallet::storage]
    pub type ActiveInternalProposal<T: Config> = StorageValue<_, Proposal<T>, OptionQuery>;

    #[pallet::storage] // ring slots: physical index -> item id
    pub type InternalProposalQueue<T: Config> =
        StorageMap<_, Blake2_128Concat, (QueueId, u32), ProposalId, OptionQuery>;

    #[pallet::storage] // next to pop
    pub type Head<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage] // next free slot to push
    pub type Tail<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new proposal has been submitted
        ProposalSubmitted {
            proposal: Proposal<T>,
        },
        /// A vote has been cast on a proposal
        Voted {
            voter: T::AccountId,
            proposal_id: ProposalId,
            aye: bool,
        },
        /// Consensus has been reached on a proposal
        ConsensusReached {
            proposal_id: ProposalId,
            consensus_result: ProposalStatusEnum,
        },
        /// Voting period has been updated
        VotingPeriodUpdated {
            old_period: BlockNumberFor<T>,
            new_period: BlockNumberFor<T>,
        },
        /// An expired voting session has been cleaned up
        ExpiredVotingSessionCleaned {
            proposal_id: ProposalId,
        },

        // DUMMY
        InternalVoteSubmitted {
            proposal_id: ProposalId,
            aye: bool,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The title is too large
        InvalidTitle,
        /// The payload is too large for inline storage
        InvalidInlinePayload,
        /// The payload URI is too large
        InvalidUri,
        /// The proposal is not valid
        InvalidProposal,
        /// A proposal with the same external_ref already exists
        DuplicateExternalRef,
        /// A proposal with the same id already exists
        DuplicateProposal,
        /// Inner proposal queue is full
        InnerProposalQueueFull,
        /// Inner proposal queue is corrupt
        QueueCorruptState,
        /// Inner proposal queue is empty
        QueueEmpty,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(0)]
        pub fn submit_external_proposal(
            origin: OriginFor<T>,
            proposal: ProposalRequest,
        ) -> DispatchResult {
            let proposer = T::ExternalProposerOrigin::ensure_origin(origin)?;
            Self::add_proposal(proposer, proposal)?;
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(0)]
        pub fn signed_submit_external_proposal(
            origin: OriginFor<T>,
            proposal: ProposalRequest,
        ) -> DispatchResult {
            let proposer = T::ExternalProposerOrigin::ensure_origin(origin)?;
            // TODO: Complete me
            Self::add_proposal(proposer, proposal)?;
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(0)]
        pub fn vote(origin: OriginFor<T>, proposal_id: ProposalId) -> DispatchResult {
            // TODO: Complete me
            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(0)]
        pub fn signed_vote(origin: OriginFor<T>, proposal_id: ProposalId) -> DispatchResult {
            // TODO: Complete me
            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(0)]
        pub fn internal_vote(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
            aye: bool,
        ) -> DispatchResult {
            // TODO: Complete me
            Self::deposit_event(Event::InternalVoteSubmitted { proposal_id, aye });
            Ok(())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            if let Call::internal_vote { proposal_id, aye } = call {
                let provides_tag = (proposal_id, aye).encode();
                ValidTransaction::with_tag_prefix("internalVote")
                    .priority(TransactionPriority::MAX)
                    .and_provides(vec![provides_tag])
                    .longevity(64_u64)
                    .propagate(true)
                    .build()
            } else {
                InvalidTransaction::Call.into()
            }
        }
    }

    impl<T: Config> Pallet<T> {
        fn add_proposal(
            proposer: Option<T::AccountId>,
            proposal_request: ProposalRequest,
        ) -> DispatchResult {
            let proposal = to_proposal::<T>(proposal_request, proposer.clone())?;
            ensure!(proposal.is_valid(), Error::<T>::InvalidProposal);

            let external_ref = proposal.external_ref;
            // ensure external_ref is unique
            ensure!(
                !ExternalRef::<T>::contains_key(external_ref),
                Error::<T>::DuplicateExternalRef
            );

            let proposal_id = proposal.clone().generate_id();

            // ensure proposal_id is unique
            ensure!(!Proposals::<T>::contains_key(proposal_id), Error::<T>::DuplicateProposal);

            // store proposal
            Proposals::<T>::insert(proposal_id, &proposal);
            ExternalRef::<T>::insert(external_ref, proposal_id);

            if let ProposalSource::Internal(_) = proposal.source {
                if ActiveInternalProposal::<T>::get().is_none() {
                    ActiveInternalProposal::<T>::put(proposal.clone());
                } else {
                    Self::enqueue(proposal_id)?;
                }
            }

            Self::deposit_event(Event::ProposalSubmitted { proposal: proposal.clone() });

            T::WatchtowerHooks::on_proposal_submitted(proposal_id, proposal)?;

            Ok(())
        }
    }

    impl<T: Config> WatchtowerInterface for Pallet<T> {
        type AccountId = T::AccountId;

        fn get_proposal_status(proposal_id: ProposalId) -> ProposalStatusEnum {
            ProposalStatus::<T>::get(proposal_id)
        }

        fn get_proposer(proposal_id: ProposalId) -> Option<Self::AccountId> {
            Proposals::<T>::get(proposal_id).map(|proposal| proposal.proposer)?
        }

        fn submit_proposal(
            proposer: Option<Self::AccountId>,
            proposal: ProposalRequest,
        ) -> DispatchResult {
            Self::add_proposal(proposer, proposal)
        }
    }
}
