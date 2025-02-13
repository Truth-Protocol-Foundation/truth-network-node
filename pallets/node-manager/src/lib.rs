#![cfg_attr(not(feature = "std"), no_std)]
use common_primitives::constants::REGISTERED_NODE_KEY;
use frame_support::{
    dispatch::DispatchResult,
    pallet_prelude::*,
    storage::{generator::StorageDoubleMap as StorageDoubleMapTrait, PrefixIterator},
    traits::{Currency, ExistenceRequirement, IsSubType, StorageVersion},
    PalletId,
};
use frame_system::{
    offchain::{SendTransactionTypes, SubmitTransaction},
    pallet_prelude::*,
};
use pallet_avn::{self as avn};
use parity_scale_codec::{Decode, Encode, FullCodec};
use sp_application_crypto::RuntimeAppPublic;
use sp_avn_common::event_types::Validator;
use sp_core::MaxEncodedLen;
use sp_runtime::{
    offchain::storage::{MutateStorageError, StorageRetrievalError, StorageValueRef},
    scale_info::TypeInfo,
    traits::{AccountIdConversion, Dispatchable, IdentifyAccount, Verify, Zero},
    transaction_validity::{
        InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity,
        ValidTransaction,
    },
    DispatchError, RuntimeDebug, Saturating,
};
pub mod types;
use crate::types::*;
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
#[path = "tests/mock.rs"]
mod mock;

// Definition of the crypto to use for signing
pub mod sr25519 {
    mod app_sr25519 {
        use sp_application_crypto::{app_crypto, sr25519, KeyTypeId};
        app_crypto!(sr25519, KeyTypeId(*b"nodk"));
    }

    pub type AuthorityId = app_sr25519::Public;
}

#[cfg(not(feature = "std"))]
use sp_std::prelude::*;

const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);
/// A type alias for a unique identifier of a node
pub(crate) type NodeId<T> = <T as frame_system::Config>::AccountId;
#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    /// Map of registered nodes
    #[pallet::storage]
    pub type NodeRegistry<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        NodeId<T>,
        NodeInfo<T::SignerId, T::AccountId>,
        OptionQuery,
    >;

    /// Total registered nodes.
    /// Note: This is mainly used for performance reasons. It is better to have a single value storage
    /// than iterate over a huge map.
    #[pallet::storage]
    pub type TotalRegisteredNodes<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Registry of nodes with their owners.
    #[pallet::storage]
    pub type OwnedNodes<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId, // OwnerAddress
        Blake2_128Concat,
        NodeId<T>,
        (),
        OptionQuery,
    >;

    /// The admin account that can register new nodes
    #[pallet::storage]
    pub type NodeRegistrar<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    // Pallet Events
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new node has been registered
        NodeRegistered { owner: T::AccountId, node: NodeId<T> },
        /// A new node registrar has been set
        NodeRegistrarSet { new_registrar: T::AccountId },
    }

    // Pallet Errors
    #[pallet::error]
    pub enum Error<T> {
        /// The node registrar account is invalid
        InvalidRegistrar,
        /// The node registrar account is not set
        RegistrarNotSet,
        /// Node has already been registered
        DuplicateNode,
    }

    #[pallet::config]
    pub trait Config:
        frame_system::Config + avn::Config + SendTransactionTypes<Call<Self>>
    {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>>
            + Into<<Self as frame_system::Config>::RuntimeEvent>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The currency type for this module.
        type Currency: Currency<Self::AccountId>;
        // The identifier type for an offchain transaction signer.
        type SignerId: Member
            + Parameter
            + RuntimeAppPublic
            + Ord
            + MaybeSerializeDeserialize
            + MaxEncodedLen;
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
        /// The weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register a new node
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::register_node())]
        pub fn register_node(
            origin: OriginFor<T>,
            node: NodeId<T>,
            owner: T::AccountId,
            signing_key: T::SignerId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let registrar = NodeRegistrar::<T>::get().ok_or(Error::<T>::RegistrarNotSet)?;
            ensure!(who == registrar, Error::<T>::InvalidRegistrar);
            ensure!(!<NodeRegistry<T>>::contains_key(&node), Error::<T>::DuplicateNode);

            <OwnedNodes<T>>::insert(&owner, &node, ());
            <NodeRegistry<T>>::insert(
                &node,
                NodeInfo::<T::SignerId, T::AccountId>::new(owner.clone(), signing_key),
            );
            <TotalRegisteredNodes<T>>::mutate(|n| *n = n.saturating_add(1));
            Self::deposit_event(Event::NodeRegistered { owner, node });

            Ok(())
        }
        // Implement me
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        // Implement me
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        // Implement me
    }

    impl<T: Config> Pallet<T> {
        // Implement me
    }
}
