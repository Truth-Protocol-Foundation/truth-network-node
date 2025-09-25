#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
};

use frame_support::{
    dispatch::DispatchResult,
    pallet_prelude::*,
    traits::{IsSubType, IsType},
};
use frame_system::{offchain::SendTransactionTypes, pallet_prelude::*, WeightInfo};
use parity_scale_codec::{Decode, Encode};
pub use sp_avn_common::{verify_signature, watchtower::*, InnerCallValidator, Proof};
use sp_core::{MaxEncodedLen, H256};
pub use sp_runtime::{
    traits::{AtLeast32Bit, Dispatchable, ValidateUnsigned},
    transaction_validity::{
        InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity,
        ValidTransaction,
    },
    Perbill,
};
use sp_runtime::{
    traits::{IdentifyAccount, Verify},
    RuntimeAppPublic, Saturating,
};
use sp_std::prelude::*;

pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);
pub const DEFAULT_VOTING_PERIOD_BLOCKS: u32 = 100;
pub const WATCHTOWER_UNSIGNED_VOTE_CONTEXT: &'static [u8] = b"wt_unsigned_vote";
pub const WATCHTOWER_FINALISE_PROPOSAL_CONTEXT: &'static [u8] = b"wt_finalise_proposal";
pub const INVALID_WATCHTOWER: u8 = 2;

pub mod vote;
pub use vote::*;
pub mod offchain;
pub use offchain::*;
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

        /// The SignerId type used in Watchtowers
        type SignerId: Member + Parameter + sp_runtime::RuntimeAppPublic + Ord + MaxEncodedLen;

        /// A type that can be used to verify signatures
        type Public: IdentifyAccount<AccountId = Self::AccountId>;

        /// The signature type used by accounts/transactions.
        #[cfg(not(feature = "runtime-benchmarks"))]
        type Signature: Verify<Signer = Self::Public> + Member + Decode + Encode + TypeInfo;

        #[cfg(feature = "runtime-benchmarks")]
        type Signature: Verify<Signer = Self::Public>
            + Member
            + Decode
            + Encode
            + TypeInfo
            + From<sp_core::sr25519::Signature>;

        /// Interface for accessing registered watchtowers
        type Watchtowers: NodesInterface<Self::AccountId, Self::SignerId>;

        /// Hooks for other pallets to implement custom logic on certain events
        type WatchtowerHooks: WatchtowerHooks<Proposal<Self>>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;

        /// The lifetime (in blocks) of a signed transaction.
        #[pallet::constant]
        type SignedTxLifetime: Get<u32>;

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

    #[pallet::storage]
    #[pallet::getter(fn votes)]
    pub type Votes<T: Config> = StorageMap<_, Blake2_128Concat, ProposalId, Vote, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn voters)]
    pub type Voters<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId, // Voter
        Blake2_128Concat,
        ProposalId,
        bool, // voted aye or nay
        ValueQuery,
    >;

    /// The currently active internal proposal being voted on, if any
    #[pallet::storage]
    pub type ActiveInternalProposal<T: Config> = StorageValue<_, ProposalId, OptionQuery>;

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
        ProposalSubmitted { proposal: Proposal<T> },
        /// A vote has been cast on a proposal
        ExternalVoteSubmitted {
            voter: T::AccountId,
            proposal_id: ProposalId,
            aye: bool,
            vote_weight: u32,
        },
        // Keeping 2 events instead of one with source to make it easier to filter for Dapps
        /// An internal vote has been submitted
        InternalVoteSubmitted {
            voter: T::AccountId,
            proposal_id: ProposalId,
            aye: bool,
            vote_weight: u32,
        },
        /// Consensus has been reached on a proposal
        VotingEnded {
            proposal_id: ProposalId,
            external_ref: H256,
            consensus_result: ProposalStatusEnum,
        },
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
        /// The proposal source is not valid for the chosen extrinsic
        InvalidProposalSource,
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
        /// The signature on the call has expired
        SignedTransactionExpired,
        /// The sender of the signed tx is not the same as the signer in the proof
        SenderIsNotSigner,
        /// The proof on the call is not valid
        UnauthorizedSignedTransaction,
        /// The proposal was not found
        ProposalNotFound,
        /// The voter is not an authorized watchtower
        UnauthorizedVoter,
        /// The proposal is not currently active
        ProposalNotActive,
        /// The voter has already voted
        AlreadyVoted,
        /// The signing key of the voter could not be found
        VoterSigningKeyNotFound,
        /// The signature on the unsigned transaction is not valid
        UnauthorizedUnsignedTransaction,
        /// Failed to acquire offchain db lock
        FailedToAcquireOcwDbLock,
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
            ensure!(
                matches!(proposal.source, ProposalSource::External),
                Error::<T>::InvalidProposalSource
            );

            Self::add_proposal(proposer, proposal)?;
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(0)]
        pub fn signed_submit_external_proposal(
            origin: OriginFor<T>,
            proposal: ProposalRequest,
            block_number: BlockNumberFor<T>,
            proof: Proof<T::Signature, T::AccountId>,
        ) -> DispatchResult {
            let proposer = T::ExternalProposerOrigin::ensure_origin(origin)?;
            ensure!(
                matches!(proposal.source, ProposalSource::External),
                Error::<T>::InvalidProposalSource
            );
            ensure!(proposer == Some(proof.signer.clone()), Error::<T>::SenderIsNotSigner);
            ensure!(
                block_number.saturating_add(T::SignedTxLifetime::get().into()) >
                    frame_system::Pallet::<T>::block_number(),
                Error::<T>::SignedTransactionExpired
            );

            // Create and verify the signed payload
            let signed_payload = Self::encode_signed_submit_external_proposal_params(
                &proof.relayer,
                &proposal,
                &block_number,
            );

            ensure!(
                verify_signature::<T::Signature, T::AccountId>(&proof, &signed_payload).is_ok(),
                Error::<T>::UnauthorizedSignedTransaction
            );

            Self::add_proposal(proposer, proposal)?;
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(0)]
        pub fn vote(origin: OriginFor<T>, proposal_id: ProposalId, aye: bool) -> DispatchResult {
            let voter = ensure_signed(origin)?;
            Self::process_vote(&voter, proposal_id, aye)?;
            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(0)]
        pub fn signed_vote(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
            aye: bool,
            block_number: BlockNumberFor<T>,
            proof: Proof<T::Signature, T::AccountId>,
        ) -> DispatchResult {
            let voter = ensure_signed(origin)?;
            ensure!(voter == proof.signer, Error::<T>::SenderIsNotSigner);
            ensure!(
                block_number.saturating_add(T::SignedTxLifetime::get().into()) >
                    frame_system::Pallet::<T>::block_number(),
                Error::<T>::SignedTransactionExpired
            );

            // Create and verify the signed payload
            let signed_payload = Self::encode_signed_submit_vote_params(
                &proof.relayer,
                &proposal_id,
                &aye,
                &block_number,
            );

            ensure!(
                verify_signature::<T::Signature, T::AccountId>(&proof, &signed_payload).is_ok(),
                Error::<T>::UnauthorizedSignedTransaction
            );

            Self::process_vote(&voter, proposal_id, aye)?;

            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(0)]
        pub fn unsigned_vote(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
            aye: bool,
            voter: T::AccountId,
            signature: <T::SignerId as RuntimeAppPublic>::Signature,
        ) -> DispatchResult {
            ensure_none(origin)?;

            let voter_signing_key = match T::Watchtowers::get_node_signing_key(&voter) {
                Some(key) => key,
                None => return Err(Error::<T>::VoterSigningKeyNotFound.into()),
            };

            if !Self::offchain_signature_is_valid(
                &(WATCHTOWER_UNSIGNED_VOTE_CONTEXT, proposal_id, aye, &voter),
                &voter_signing_key,
                &signature,
            ) {
                return Err(Error::<T>::UnauthorizedUnsignedTransaction.into())
            }
            // We allow unsigned votes for both internal and external proposals.
            // For now we expect that only internal proposals should be voted on by the OCW but it
            // might change in the future.
            Self::process_vote(&voter, proposal_id, aye)?;
            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(0)]
        pub fn unsigned_finalise_proposal(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
            watchtower: T::AccountId,
            signature: <T::SignerId as RuntimeAppPublic>::Signature,
        ) -> DispatchResult {
            ensure_none(origin)?;

            let proposal = Proposals::<T>::get(proposal_id).ok_or(Error::<T>::ProposalNotFound)?;
            ensure!(
                ProposalStatus::<T>::get(proposal_id) == ProposalStatusEnum::Active,
                Error::<T>::ProposalNotActive
            );
            let watchtower_signing_key = match T::Watchtowers::get_node_signing_key(&watchtower) {
                Some(key) => key,
                None => return Err(Error::<T>::VoterSigningKeyNotFound.into()),
            };

            if !Self::offchain_signature_is_valid(
                &(WATCHTOWER_FINALISE_PROPOSAL_CONTEXT, proposal_id, &watchtower),
                &watchtower_signing_key,
                &signature,
            ) {
                return Err(Error::<T>::UnauthorizedUnsignedTransaction.into())
            }

            Self::finalise_voting_if_required(proposal_id, &proposal)
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::unsigned_vote { proposal_id, aye, voter, signature } => {
                    if T::Watchtowers::is_authorized_watchtower(voter) == false {
                        return InvalidTransaction::Custom(INVALID_WATCHTOWER).into()
                    }

                    ValidTransaction::with_tag_prefix("wt_unsignedVote")
                        .priority(TransactionPriority::MAX)
                        .and_provides((voter, proposal_id))
                        .longevity(64_u64)
                        .propagate(true)
                        .build()
                },
                Call::unsigned_finalise_proposal { proposal_id, watchtower, signature } => {
                    if T::Watchtowers::is_authorized_watchtower(watchtower) == false {
                        return InvalidTransaction::Custom(INVALID_WATCHTOWER).into()
                    }

                    ValidTransaction::with_tag_prefix("wt_unsignedFinaliseProposal")
                        .priority(TransactionPriority::MAX)
                        .and_provides((watchtower, proposal_id))
                        .longevity(64_u64)
                        .propagate(true)
                        .build()
                },
                _ => InvalidTransaction::Call.into(),
            }
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(block_number: BlockNumberFor<T>) {
            // Only run if this node is a watchtower with a local signing key
            let Some((watchtower, signing_key)) =
                T::Watchtowers::get_node_from_local_signing_keys()
            else {
                return;
            };

            // Only proceed if there is an active internal proposal
            let Some(proposal_id) = ActiveInternalProposal::<T>::get() else {
                return;
            };

            // Only proceed if the proposal exists
            let Some(active_proposal) = <Proposals<T>>::get(proposal_id) else {
                return;
            };

            // Only finalise if the voting period has ended
            if block_number < active_proposal.end_at {
                return;
            }

            // Only send a new finalise request if we haven't sent one already
            if Self::finalise_internal_vote_submission_in_progress(
                proposal_id,
                watchtower.clone(),
                block_number,
            ) {
                return;
            }

            Self::invoke_finalise_internal_vote(proposal_id, watchtower, signing_key, block_number);
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
            ensure!(
                !ExternalRef::<T>::contains_key(external_ref),
                Error::<T>::DuplicateExternalRef
            );

            let proposal_id = proposal.clone().generate_id();
            ensure!(!Proposals::<T>::contains_key(proposal_id), Error::<T>::DuplicateProposal);

            Proposals::<T>::insert(proposal_id, &proposal);
            ExternalRef::<T>::insert(external_ref, proposal_id);

            if let ProposalSource::Internal(_) = proposal.source {
                if ActiveInternalProposal::<T>::get().is_none() {
                    ActiveInternalProposal::<T>::put(proposal_id);
                    ProposalStatus::<T>::insert(proposal_id, ProposalStatusEnum::Active);
                } else {
                    Self::enqueue(proposal_id)?;
                    ProposalStatus::<T>::insert(proposal_id, ProposalStatusEnum::Queued);
                }
            } else {
                ProposalStatus::<T>::insert(proposal_id, ProposalStatusEnum::Active);
            }

            Self::deposit_event(Event::ProposalSubmitted { proposal: proposal.clone() });
            T::WatchtowerHooks::on_proposal_submitted(proposal_id, proposal)?;

            Ok(())
        }

        fn process_vote(
            voter: &T::AccountId,
            proposal_id: ProposalId,
            aye: bool,
        ) -> DispatchResult {
            let proposal = Proposals::<T>::get(proposal_id).ok_or(Error::<T>::ProposalNotFound)?;
            ensure!(T::Watchtowers::is_authorized_watchtower(voter), Error::<T>::UnauthorizedVoter);
            ensure!(
                ProposalStatus::<T>::get(proposal_id) == ProposalStatusEnum::Active,
                Error::<T>::ProposalNotActive
            );
            ensure!(!Voters::<T>::contains_key(voter, proposal_id), Error::<T>::AlreadyVoted);

            let current_block = <frame_system::Pallet<T>>::block_number();
            if current_block >= proposal.end_at {
                // Voting ended but we haven't finalised it yet
                return Self::finalise_voting_if_required(proposal_id, &proposal);
            }

            let vote_weight = T::Watchtowers::get_watchtower_voting_weight(voter);
            // This should not happen but just in case
            ensure!(vote_weight > 0, Error::<T>::UnauthorizedVoter);

            Voters::<T>::insert(voter, proposal_id, aye);
            Votes::<T>::mutate(proposal_id, |vote| {
                if aye {
                    vote.ayes = vote.ayes.saturating_add(vote_weight);
                } else {
                    vote.nays = vote.nays.saturating_add(vote_weight);
                }
            });

            match proposal.source {
                ProposalSource::Internal(_) => Self::deposit_event(Event::InternalVoteSubmitted {
                    voter: voter.clone(),
                    proposal_id,
                    aye,
                    vote_weight,
                }),
                ProposalSource::External => Self::deposit_event(Event::ExternalVoteSubmitted {
                    voter: voter.clone(),
                    proposal_id,
                    aye,
                    vote_weight,
                }),
            };

            Self::finalise_voting_if_required(proposal_id, &proposal)
        }

        pub fn get_encoded_call_param(
            call: &<T as Config>::RuntimeCall,
        ) -> Option<(&Proof<T::Signature, T::AccountId>, Vec<u8>)> {
            let call = match call.is_sub_type() {
                Some(call) => call,
                None => return None,
            };

            match call {
                Call::signed_submit_external_proposal {
                    ref proof,
                    ref block_number,
                    ref proposal,
                } => {
                    let encoded_data = Self::encode_signed_submit_external_proposal_params(
                        &proof.relayer,
                        proposal,
                        block_number,
                    );

                    Some((proof, encoded_data))
                },
                _ => None,
            }
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

    impl<T: Config> InnerCallValidator for Pallet<T> {
        type Call = <T as Config>::RuntimeCall;

        fn signature_is_valid(call: &Box<Self::Call>) -> bool {
            if let Some((proof, signed_payload)) = Self::get_encoded_call_param(call) {
                return verify_signature::<T::Signature, T::AccountId>(
                    &proof,
                    &signed_payload.as_slice(),
                )
                .is_ok();
            }

            return false;
        }
    }
}
