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
    weights::WeightMeter,
};
use frame_system::{offchain::SendTransactionTypes, pallet_prelude::*};
use parity_scale_codec::{Decode, Encode};
pub use sp_avn_common::{verify_signature, watchtower::*, InnerCallValidator, Proof};
use sp_core::{MaxEncodedLen, H256};
pub use sp_runtime::{
    traits::{AtLeast32Bit, Dispatchable, ValidateUnsigned},
    transaction_validity::{
        InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity,
        ValidTransaction,
    },
    Perbill, SaturatedConversion,
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

pub mod proxy;
pub mod types;
pub mod vote;
pub use types::*;
pub mod queue;
pub use queue::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod default_weights;
pub use default_weights::WeightInfo;

#[cfg(test)]
#[path = "tests/add_proposal.rs"]
mod add_proposal;
#[cfg(test)]
#[path = "tests/admin.rs"]
mod admin;
#[cfg(test)]
#[path = "tests/mock.rs"]
mod mock;
#[cfg(test)]
#[path = "tests/voting.rs"]
mod voting;

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
    pub type MinVotingPeriod<T: Config> =
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
        ProposalId,
        Blake2_128Concat,
        T::AccountId, // Voter
        bool,         // voted aye or nay
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

    /// Completed or Expired proposals that need to be removed from storage.
    #[pallet::storage]
    pub type ProposalsToRemove<T: Config> =
        StorageMap<_, Blake2_128Concat, ProposalId, (), OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new proposal has been submitted
        ProposalSubmitted { proposal_id: ProposalId, external_ref: H256 },
        /// A vote has been cast on a proposal
        VoteSubmitted { voter: T::AccountId, proposal_id: ProposalId, aye: bool, vote_weight: u32 },
        /// Consensus has been reached on a proposal
        VotingEnded {
            proposal_id: ProposalId,
            external_ref: H256,
            consensus_result: ProposalStatusEnum,
        },
        /// Voting period has been updated
        VotingPeriodUpdated { old_period: BlockNumberFor<T>, new_period: BlockNumberFor<T> },
        /// A completed or expired proposal has been cleaned from storage
        ProposalCleaned { proposal_id: ProposalId },
        /// Minimum voting period has been updated
        MinVotingPeriodSet { new_period: BlockNumberFor<T> },
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
        /// The voting period for the proposal has not yet ended
        ProposalVotingPeriodNotEnded,
        /// The voting period is shorter than the minimum allowed
        VotingPeriodTooShort,
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
            let owner = ensure_signed(origin)?;
            Self::process_vote(&owner, proposal_id, aye)?;
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
            let owner = ensure_signed(origin)?;
            ensure!(owner == proof.signer, Error::<T>::SenderIsNotSigner);
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

            Self::process_vote(&owner, proposal_id, aye)?;

            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(0)]
        pub fn unsigned_vote(
            origin: OriginFor<T>,
            proposal_id: ProposalId,
            aye: bool,
            watchtower: T::AccountId,
            signature: <T::SignerId as RuntimeAppPublic>::Signature,
        ) -> DispatchResult {
            ensure_none(origin)?;

            let voter_signing_key = match T::Watchtowers::get_node_signing_key(&watchtower) {
                Some(key) => key,
                None => return Err(Error::<T>::VoterSigningKeyNotFound.into()),
            };

            if !Self::offchain_signature_is_valid(
                &(WATCHTOWER_UNSIGNED_VOTE_CONTEXT, proposal_id, aye, &watchtower),
                &voter_signing_key,
                &signature,
            ) {
                return Err(Error::<T>::UnauthorizedUnsignedTransaction.into())
            }
            // We allow unsigned votes for both internal and external proposals.
            // For now we expect that only internal proposals should be voted on by the OCW but it
            // might change in the future.
            Self::process_vote(&watchtower, proposal_id, aye)?;
            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::finalise_proposal())]
        pub fn finalise_proposal(origin: OriginFor<T>, proposal_id: ProposalId) -> DispatchResult {
            // Anyone can call this to finalise voting
            ensure_signed(origin)?;

            let proposal = Proposals::<T>::get(proposal_id).ok_or(Error::<T>::ProposalNotFound)?;
            ensure!(
                ProposalStatus::<T>::get(proposal_id) == ProposalStatusEnum::Active,
                Error::<T>::ProposalNotActive
            );
            let current_block = <frame_system::Pallet<T>>::block_number();
            ensure!(
                Self::proposal_expired(current_block, &proposal),
                Error::<T>::ProposalVotingPeriodNotEnded
            );

            Self::finalise_expired_voting(proposal_id, &proposal)?;

            Ok(())
        }

        /// Set admin configurations
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::set_admin_config_voting())]
        pub fn set_admin_config(
            origin: OriginFor<T>,
            config: AdminConfig<BlockNumberFor<T>>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            match config {
                AdminConfig::MinVotingPeriod(period) => {
                    <MinVotingPeriod<T>>::mutate(|p| *p = period);
                    Self::deposit_event(Event::MinVotingPeriodSet { new_period: period });
                    return Ok(Some(<T as Config>::WeightInfo::set_admin_config_voting()).into());
                },
            }
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::unsigned_vote { proposal_id, aye: _, watchtower, signature: _ } => {
                    if T::Watchtowers::is_authorized_watchtower(watchtower) == false {
                        return InvalidTransaction::Custom(INVALID_WATCHTOWER).into()
                    }

                    ValidTransaction::with_tag_prefix("wt_unsignedVote")
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
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            let mut total_weight: Weight = Weight::zero();
            // Check if the active proposal has expired and finalise it if needed
            // TODO: benchmarks active_proposal_expiry_status and use that weight
            total_weight = total_weight
                .saturating_add(<T as Config>::WeightInfo::active_proposal_expiry_status());

            if let Some((proposal_id, active_proposal, expired)) =
                Self::active_proposal_expiry_status(now)
            {
                if !expired {
                    return total_weight
                }

                Self::finalise_expired_voting(proposal_id, &active_proposal).unwrap_or_else(|e| {
                    log::error!(
                        "🪲 Failed to finalise voting for internal proposal {}: {:?}",
                        proposal_id,
                        e
                    );
                });

                total_weight = total_weight
                    .saturating_add(<T as Config>::WeightInfo::finalise_expired_voting());
                return total_weight
            };

            total_weight
        }

        fn on_idle(n: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
            Self::cleanup_proposals(n, remaining_weight)
        }
    }

    impl<T: Config> Pallet<T> {
        fn add_proposal(
            proposer: Option<T::AccountId>,
            proposal_request: ProposalRequest,
        ) -> DispatchResult {
            // Proposal is validated before creating it.
            let mut proposal = to_proposal::<T>(proposal_request, proposer)?;

            let external_ref = proposal.external_ref;
            ensure!(
                !ExternalRef::<T>::contains_key(external_ref),
                Error::<T>::DuplicateExternalRef
            );

            let proposal_id = proposal.generate_id();
            ensure!(!Proposals::<T>::contains_key(proposal_id), Error::<T>::DuplicateProposal);

            let current_block = <frame_system::Pallet<T>>::block_number();
            let proposal_active: bool;
            if let ProposalSource::Internal(_) = proposal.source {
                if ActiveInternalProposal::<T>::get().is_none() {
                    proposal.end_at =
                        Some(current_block.saturating_add(proposal.vote_duration.into()));
                    ActiveInternalProposal::<T>::put(proposal_id);
                    ProposalStatus::<T>::insert(proposal_id, ProposalStatusEnum::Active);
                    proposal_active = true;
                } else {
                    Self::enqueue(proposal_id)?;
                    ProposalStatus::<T>::insert(proposal_id, ProposalStatusEnum::Queued);
                    proposal_active = false;
                }
            } else {
                proposal.end_at = Some(current_block.saturating_add(proposal.vote_duration.into()));
                ProposalStatus::<T>::insert(proposal_id, ProposalStatusEnum::Active);
                proposal_active = true;
            }

            Proposals::<T>::insert(proposal_id, &proposal);
            ExternalRef::<T>::insert(external_ref, proposal_id);

            Self::deposit_event(Event::ProposalSubmitted { proposal_id, external_ref });

            if proposal_active {
                T::WatchtowerHooks::on_proposal_submitted(proposal_id, proposal)?;
            }

            Ok(())
        }

        fn process_vote(
            voter: &T::AccountId,
            proposal_id: ProposalId,
            aye: bool,
        ) -> DispatchResult {
            let proposal = Proposals::<T>::get(proposal_id).ok_or(Error::<T>::ProposalNotFound)?;
            ensure!(
                ProposalStatus::<T>::get(proposal_id) == ProposalStatusEnum::Active,
                Error::<T>::ProposalNotActive
            );

            // Do this before validating vote uniqueness
            let current_block = <frame_system::Pallet<T>>::block_number();
            if Self::proposal_expired(current_block, &proposal) {
                // Voting ended but we haven't finalised it yet
                return Self::finalise_expired_voting(proposal_id, &proposal);
            }

            ensure!(!Voters::<T>::contains_key(proposal_id, voter), Error::<T>::AlreadyVoted);

            let vote_weight;
            match proposal.source {
                ProposalSource::Internal(_) => {
                    ensure!(
                        T::Watchtowers::is_authorized_watchtower(voter),
                        Error::<T>::UnauthorizedVoter
                    );

                    vote_weight = 1;
                },
                ProposalSource::External => {
                    ensure!(
                        T::Watchtowers::is_watchtower_owner(voter),
                        Error::<T>::UnauthorizedVoter
                    );

                    vote_weight = T::Watchtowers::get_watchtower_voting_weight(voter);
                    // This should not happen but just in case
                    ensure!(vote_weight > 0, Error::<T>::UnauthorizedVoter);
                },
            };

            Voters::<T>::insert(proposal_id, voter, aye);
            Votes::<T>::mutate(proposal_id, |vote| {
                if aye {
                    vote.ayes = vote.ayes.saturating_add(vote_weight);
                } else {
                    vote.nays = vote.nays.saturating_add(vote_weight);
                }
            });

            Self::deposit_event(Event::VoteSubmitted {
                voter: voter.clone(),
                proposal_id,
                aye,
                vote_weight,
            });

            Self::finalise_voting_if_required(proposal_id, &proposal, current_block)
        }

        fn cleanup_proposals(_n: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
            let mut meter = WeightMeter::with_limit(remaining_weight);
            let dbw = <T as frame_system::Config>::DbWeight::get();
            const MAX_VOTERS: usize = 250;

            // Check if we have enough weight to read the keys
            if meter.try_consume(dbw.reads(1)).is_err() {
                return meter.consumed()
            }

            let Some(proposal_id) = ProposalsToRemove::<T>::iter_keys().next() else {
                // Nothing to clean
                return meter.consumed();
            };

            // Avoid deleting while iterating. Its safer to do it in 2 steps
            let mut to_delete: Vec<T::AccountId> = Vec::new();
            for (who, _) in Voters::<T>::iter_prefix(&proposal_id).take(MAX_VOTERS) {
                // read for this item
                if meter.try_consume(dbw.reads(1)).is_err() {
                    break;
                }
                to_delete.push(who);
            }

            for who in to_delete.iter() {
                if meter.try_consume(dbw.writes(1)).is_err() {
                    break;
                }
                Voters::<T>::remove(&proposal_id, who);
            }

            // Check if we have finished removing all votes
            if meter.try_consume(dbw.reads(1)).is_err() {
                return meter.consumed()
            }

            if Voters::<T>::iter_prefix(proposal_id).next().is_none() {
                // We have removed all votes, now we can remove the proposal and its data
                if meter.try_consume(dbw.writes(4)).is_err() {
                    return meter.consumed()
                }

                Proposals::<T>::remove(proposal_id);
                Votes::<T>::remove(proposal_id);
                ProposalsToRemove::<T>::remove(proposal_id);

                Self::deposit_event(Event::ProposalCleaned { proposal_id });
            }

            meter.consumed()
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
