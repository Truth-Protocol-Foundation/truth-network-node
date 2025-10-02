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
pub mod proxy;
pub mod types;
pub use types::*;
pub mod queue;
pub use queue::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod default_weights;
pub use default_weights::WeightInfo;


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
        ProposalSubmitted {
            proposal_id: ProposalId,
            external_ref: H256,
            status: ProposalStatusEnum,
        },
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
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // We don't want external users to add internal proposals to avoid
        // DOSing the internal proposal queue.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::submit_external_proposal())]
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
        #[pallet::weight(<T as Config>::WeightInfo::signed_submit_external_proposal())]
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
    impl<T: Config> Pallet<T> {
        fn add_proposal(
            proposer: Option<T::AccountId>,
            proposal_request: ProposalRequest,
        ) -> DispatchResult {
            let current_block = <frame_system::Pallet<T>>::block_number();
            // Proposal is validated before creating it.
            let mut proposal = to_proposal::<T>(proposal_request, proposer, current_block)?;

            let external_ref = proposal.external_ref;
            ensure!(
                !ExternalRef::<T>::contains_key(external_ref),
                Error::<T>::DuplicateExternalRef
            );

            let proposal_id = proposal.generate_id();
            ensure!(!Proposals::<T>::contains_key(proposal_id), Error::<T>::DuplicateProposal);

            let status: ProposalStatusEnum;
            if let ProposalSource::Internal(_) = proposal.source {
                if ActiveInternalProposal::<T>::get().is_none() {
                    proposal.end_at =
                        Some(current_block.saturating_add(proposal.vote_duration.into()));
                    ActiveInternalProposal::<T>::put(proposal_id);
                    status = ProposalStatusEnum::Active;
                } else {
                    Self::enqueue(proposal_id)?;
                    status = ProposalStatusEnum::Queued;
                }
            } else {
                proposal.end_at = Some(current_block.saturating_add(proposal.vote_duration.into()));
                status = ProposalStatusEnum::Active;
            }

            ProposalStatus::<T>::insert(proposal_id, &status);
            Proposals::<T>::insert(proposal_id, &proposal);
            ExternalRef::<T>::insert(external_ref, proposal_id);

            if status == ProposalStatusEnum::Active {
                T::WatchtowerHooks::on_proposal_submitted(proposal_id, proposal)?;
            }

            Self::deposit_event(Event::ProposalSubmitted { proposal_id, external_ref, status });

            Ok(())
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
