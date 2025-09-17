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
pub use prediction_market_primitives::watchtower::*;
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

pub const OCW_LOCK_PREFIX: &[u8] = b"pallet-watchtower::lock::";
pub const OCW_LOCK_TIMEOUT_MS: u64 = 10000;
pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);
pub const WATCHTOWER_OCW_CONTEXT: &[u8] = b"watchtower_ocw_vote";
pub const WATCHTOWER_VOTE_PROVIDE_TAG: &[u8] = b"WatchtowerVoteProvideTag";
pub const DEFAULT_VOTING_PERIOD_BLOCKS: u32 = 100;

pub mod types;
pub use types::*;

pub type ProposalSourceOf<T> = ProposalSource<<T as Config>::ProposalKind>;
pub type ProposalRequestOf<T> = ProposalRequest<<T as Config>::ProposalKind>;
pub type ProposalOf<T> = Proposal<T, <T as Config>::ProposalKind>;

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

        /// The type of proposal kinds used by internal proposals, defined by the runtime
        type ProposalKind: Parameter
            + Member
            + MaxEncodedLen
            + TypeInfo
            + Clone
            + Eq
            + core::fmt::Debug;

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
        type WatchtowerHooks: WatchtowerHooks<Proposal = ProposalOf<Self>>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;

        /// Minimum allowed voting period in blocks
        #[pallet::constant]
        type MinVotingPeriod: Get<BlockNumberFor<Self>> + sp_std::fmt::Debug;

        /// Maximum proposal title length
        #[pallet::constant]
        type MaxTitleLen: Get<u32> + sp_std::fmt::Debug;

        /// Maximum length of inline proposal data
        #[pallet::constant]
        type MaxInlineLen: Get<u32> + sp_std::fmt::Debug;

        /// Maximum length of URI for proposals
        #[pallet::constant]
        type MaxUriLen: Get<u32> + sp_std::fmt::Debug;
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
        StorageMap<_, Blake2_128Concat, ProposalId, ProposalOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn voting_status)]
    pub type VotingStatus<T: Config> =
        StorageMap<_, Blake2_128Concat, ProposalId, VotingStatusEnum, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new proposal has been submitted
        ProposalSubmitted { proposal: ProposalOf<T> },
        /// A vote has been cast on a proposal
        Voted { voter: T::AccountId, proposal_id: ProposalId, aye: bool },
        /// Consensus has been reached on a proposal
        ConsensusReached { proposal_id: ProposalId, consensus_result: VotingStatusEnum },
        /// Voting period has been updated
        VotingPeriodUpdated { old_period: BlockNumberFor<T>, new_period: BlockNumberFor<T> },
        /// An expired voting session has been cleaned up
        ExpiredVotingSessionCleaned { proposal_id: ProposalId },
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
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(0)]
        pub fn submit_external_proposal(
            origin: OriginFor<T>,
            proposal: ProposalRequestOf<T>,
        ) -> DispatchResult {
            let proposer = T::ExternalProposerOrigin::ensure_origin(origin)?;

            Self::add_proposal(proposer, proposal)?;
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        fn add_proposal(
            proposer: Option<T::AccountId>,
            proposal: ProposalRequestOf<T>,
        ) -> DispatchResult {
            let proposal = to_proposal::<T, T::ProposalKind>(proposal, proposer.clone())?;
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

            Self::deposit_event(Event::ProposalSubmitted { proposal: proposal.clone() });

            T::WatchtowerHooks::on_proposal_submitted(proposal_id, proposal)?;

            Ok(())
        }
    }

    impl<T: Config> WatchtowerInterface for Pallet<T> {
        type ProposalKind = T::ProposalKind;
        type AccountId = T::AccountId;

        fn get_voting_status(proposal_id: ProposalId) -> VotingStatusEnum {
            VotingStatus::<T>::get(proposal_id)
        }

        fn get_proposer(proposal_id: ProposalId) -> Option<Self::AccountId> {
            Proposals::<T>::get(proposal_id).map(|proposal| proposal.proposer)?
        }

        fn submit_proposal(
            proposer: Option<Self::AccountId>,
            proposal: ProposalRequestOf<T>,
        ) -> DispatchResult {
            Self::add_proposal(proposer, proposal)
        }
    }
}
