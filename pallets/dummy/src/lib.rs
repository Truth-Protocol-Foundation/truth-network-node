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

use sp_runtime::{traits::Hash, RuntimeAppPublic, SaturatedConversion};

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

        type WatchtowerInterface: WatchtowerInterface<AccountId = Self::AccountId>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A summary watchtower proposal was submitted.
        SummaryProposalSubmitted { external_ref: H256 },
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
        pub fn submit_root_for_validation(origin: OriginFor<T>) -> DispatchResult {
            let current_block = <frame_system::Pallet<T>>::block_number();
            let external_ref: T::Hash = T::Hashing::hash_of(&current_block);
            let inner_payload = RootId::<BlockNumberFor<T>>::new(
                RootRange::<BlockNumberFor<T>>::new(current_block, current_block + 10u32.into()),
                17u64,
            );

            let x = ProposalRequest {
                title: "Dummy Proposal".as_bytes().to_vec(),
                external_ref: H256::from_slice(&external_ref.as_ref()),
                threshold: Perbill::from_percent(50),
                payload: RawPayload::Inline(inner_payload.encode()),
                source: ProposalSource::Internal(ProposalType::Anchor),
                decision_rule: DecisionRule::SimpleMajority,
                created_at: current_block.saturated_into::<u32>(),
                vote_duration: Some(100u32),
            };

            T::WatchtowerInterface::submit_proposal(None, x.clone())?;

            // let block_number = <frame_system::Pallet<T>>::block_number();
            // let external_ref = H256::from_slice(&block_number.encode());

            // Self::deposit_event(Event::SummaryProposalSubmitted { external_ref });

            Ok(())
        }
    }

    //  impl<T: Config> Pallet<T> {

    // }
}

#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum RuntimeProposalKind {
    Summary,
    Anchor,
    Governance,
    Other(u8),
}
