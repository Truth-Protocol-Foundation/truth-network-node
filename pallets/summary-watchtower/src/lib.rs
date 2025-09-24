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
use pallet_watchtower::{NodesInterface, Payload, Proposal};
use parity_scale_codec::{Decode, Encode};
pub use sp_avn_common::{watchtower::*, RootId, RootRange};
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

// pub mod types;
// pub use types::*;

pub use pallet::*;
#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        SendTransactionTypes<Call<Self>> + frame_system::Config + pallet_watchtower::Config
    {
        type RuntimeCall: Parameter
            + Dispatchable<RuntimeOrigin = <Self as frame_system::Config>::RuntimeOrigin>
            + IsSubType<Call<Self>>
            + From<Call<Self>>
            + From<pallet_watchtower::Call<Self>>;

        type RuntimeEvent: From<Event<Self>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>
            + Clone
            + Eq
            + PartialEq
            + core::fmt::Debug;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn voting_period)]
    pub type ActiveRequest<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn id_by_external_ref)]
    pub type RequestQueue<T: Config> =
        StorageMap<_, Blake2_128Concat, H256, ProposalId, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A summary watchtower proposal was submitted.
        SummaryVerificationRequested { proposal_id: ProposalId, root_id: RootId<BlockNumberFor<T>> },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The summary proposal is invalid
        InvalidSummaryProposal,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(0)]
        pub fn test(origin: OriginFor<T>) -> DispatchResult {
            Ok(())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(block_number: BlockNumberFor<T>) {
            log::debug!(target: "runtime::watchtower::ocw", "Watchtower OCW running for block {:?}", block_number);

            // let aye = true;
            // let proposal_id: ProposalId = H256::repeat_byte(1);

            // let call = pallet_watchtower::Call::internal_vote {
            //     proposal_id,
            //     aye,
            //     voter: Default::default(),
            //     signature: Default::default()
            // };

            // match SubmitTransaction::<T,
            // pallet_watchtower::Call<T>>::submit_unsigned_transaction(call.into()) {
            //     Ok(()) => (),
            //     Err(_e) => {
            //         log::debug!("Error submitting vote from Summary Watchtower OCW for block
            // {:?}", block_number);     }
            // };
        }
    }

    impl<T: Config> Pallet<T> {
        fn process_proposal(
            proposer: Option<T::AccountId>,
            proposal_id: ProposalId,
            proposal: Proposal<T>,
        ) -> DispatchResult {
            // decode payload as inline with rootId. So something like: Payload::Inline(rootId)

            let root_id = match &proposal.payload {
                Payload::Inline(data) =>
                    match RootId::<BlockNumberFor<T>>::decode(&mut &data[..]) {
                        Ok(root_id) => root_id,
                        Err(_) => {
                            log::error!(
                                "Summary Watchtower: Invalid inline payload for proposal {:?}",
                                proposal_id
                            );
                            return Err(Error::<T>::InvalidSummaryProposal.into());
                        },
                    },
                _ => {
                    log::error!(
                        "Summary Watchtower: URI payloads are not supported for proposal {:?}",
                        proposal_id
                    );
                    return Err(Error::<T>::InvalidSummaryProposal.into());
                },
            };

            Self::deposit_event(Event::SummaryVerificationRequested { proposal_id, root_id });

            Ok(())
        }
    }

    impl<T: Config> WatchtowerHooks<Proposal<T>> for Pallet<T> {
        /// Called when Watchtower raises an alert/notification.
        fn on_proposal_submitted(proposal_id: ProposalId, proposal: Proposal<T>) -> DispatchResult {
            log::warn!("Summary Watchtower: New proposal submitted: {:?}", proposal);
            Self::process_proposal(None, proposal_id, proposal)
        }

        fn on_consensus_reached(
            proposal_id: ProposalId,
            external_ref: &H256,
            approved: bool,
        ) -> DispatchResult {
            log::warn!("Summary Watchtower: Consensus reached on proposal {:?} with external ref {:?} and approval status {:?}",
                proposal_id,
                external_ref,
                approved
            );

            Ok(())
        }

        fn on_cancelled(proposal_id: ProposalId, external_ref: &H256) -> DispatchResult {
            log::warn!(
                "Summary Watchtower: Proposal {:?} with external ref {:?} was cancelled",
                proposal_id,
                external_ref
            );
            Ok(())
        }
    }
}
