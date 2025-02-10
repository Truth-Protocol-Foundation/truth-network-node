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
pub mod offchain;
pub mod types;
use crate::types::*;
pub mod default_weights;
pub use default_weights::WeightInfo;

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

const HEARTBEAT_CONTEXT: &'static [u8] = b"NodeManager_heartbeat";
const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);
pub(crate) type RewardPeriodIndex = u64;
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

    // This is mainly used for performance reasons. It is better to have a single value storage than
    // iterate over a huge map.
    /// Total registered nodes.
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

    /// The heartbeat period in blocks
    #[pallet::storage]
    pub type HeartbeatPeriod<T: Config> = StorageValue<_, u32, ValueQuery>;
    /// Tracks the current reward period.
    #[pallet::storage]
    #[pallet::getter(fn current_reward_period)]
    pub(super) type RewardPeriod<T: Config> =
        StorageValue<_, RewardPeriodInfo<BlockNumberFor<T>>, ValueQuery>;

    /// DoubleMap storing each node's uptime for a given reward period.
    #[pallet::storage]
    #[pallet::getter(fn node_uptime)]
    pub(super) type NodeUptime<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        RewardPeriodIndex,
        Blake2_128Concat,
        NodeId<T>,
        UptimeInfo<BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// The total uptime for each reward period.
    #[pallet::storage]
    pub(super) type TotalUptime<T: Config> =
        StorageMap<_, Blake2_128Concat, RewardPeriodIndex, u64, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub _phantom: sp_std::marker::PhantomData<T>,
        pub reward_period: u32,
        pub heartbeat_period: u32,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                _phantom: Default::default(),
                reward_period: 0,
                heartbeat_period: 0,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            HeartbeatPeriod::<T>::set(self.heartbeat_period);

            let reward_period: RewardPeriodInfo<BlockNumberFor<T>> =
                RewardPeriodInfo::new(0u64, 0u32.into(), self.reward_period);
            <RewardPeriod<T>>::put(reward_period);
        }
    }

    // Pallet Events
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new node has been registered
        NodeRegistered { owner: T::AccountId, node: NodeId<T> },
        /// A new reward period (in blocks) was set.
        RewardPeriodLengthSet {
            period_index: u64,
            old_reward_period_length: u32,
            new_reward_period_length: u32,
        },
        /// A new reward period was initialized.
        NewRewardPeriodStarted {
            reward_period_index: RewardPeriodIndex,
            reward_period_length: u32,
            previous_period_reward: BalanceOf<T>,
        },
        /// A new node registrar has been set
        NodeRegistrarSet { new_registrar: T::AccountId },
        /// A new heartbeat period (in blocks) was set.
        HeartbeatPeriodSet { new_heartbeat_period: u32 },
        /// A new heartbeat has been received
        HeartbeatReceived { reward_period_index: RewardPeriodIndex, node: NodeId<T> },
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
        /// The signing key of the node is invalid
        InvalidSigningKey,
        /// The reward period is invalid
        RewardPeriodInvalid,
        /// The heartbeat period is invalid
        HeartbeatPeriodInvalid,
        /// The heartbeat period is 0
        HeartbeatPeriodZero,
        /// The total uptime for the period was not found
        TotalUptimeNotFound,
        /// The node uptime for the period was not found
        NodeUptimeNotFound,
        /// The node owner was not found
        NodeOwnerNotFound,
        /// The reward payment request is invalid
        InvalidRewardPaymentRequest,
        /// Heartbeat has already been submitted
        DuplicateHeartbeat,
        /// Heartbeat submission is not valid
        InvalidHeartbeat,
        /// The node is not registered
        NodeNotRegistered,
        /// Failed to aquire a lock on the Offchain db
        FailedToAcquireOcwDbLock,
    }

    #[pallet::config]
    pub trait Config:
        frame_system::Config + avn::Config + SendTransactionTypes<Call<Self>>
    {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>>
            + Into<<Self as frame_system::Config>::RuntimeEvent>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The overarching call type.
        type RuntimeCall: Parameter
            + Dispatchable<RuntimeOrigin = <Self as frame_system::Config>::RuntimeOrigin>
            + IsSubType<Call<Self>>
            + From<Call<Self>>;
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

        /// Set admin configurations
        #[pallet::call_index(1)]
        #[pallet::weight(
            <T as Config>::WeightInfo::register_node()
            .max(<T as Config>::WeightInfo::set_admin_config_registrar())
            .max(<T as Config>::WeightInfo::set_admin_config_reward_period())
            .max(<T as Config>::WeightInfo::set_admin_config_reward_batch_size())
            .max(<T as Config>::WeightInfo::set_admin_config_reward_heartbeat())
            .max(<T as Config>::WeightInfo::set_admin_config_reward_amount())
        )]
        pub fn set_admin_config(
            origin: OriginFor<T>,
            config: AdminConfig<T::AccountId, BalanceOf<T>>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            match config {
                AdminConfig::NodeRegistrar(registrar) => {
                    <NodeRegistrar<T>>::set(Some(registrar.clone()));
                    Self::deposit_event(Event::NodeRegistrarSet { new_registrar: registrar });
                    return Ok(Some(<T as Config>::WeightInfo::set_admin_config_registrar()).into());
                },
                AdminConfig::RewardPeriod(period) => {
                    let heartbeat = <HeartbeatPeriod<T>>::get();
                    ensure!(period > heartbeat, Error::<T>::RewardPeriodInvalid);
                    let mut reward_period = RewardPeriod::<T>::get();
                    let (index, old_period) = (reward_period.current, reward_period.length);
                    reward_period.length = period;
                    <RewardPeriod<T>>::put(reward_period);
                    Self::deposit_event(Event::RewardPeriodLengthSet {
                        period_index: index,
                        old_reward_period_length: old_period,
                        new_reward_period_length: period,
                    });
                    return Ok(Some(<T as Config>::WeightInfo::set_admin_config_reward_period()).into());
                },
                AdminConfig::Heartbeat(period) => {
                    let reward_period = RewardPeriod::<T>::get().length;
                    ensure!(period > 0, Error::<T>::HeartbeatPeriodZero);
                    ensure!(period < reward_period, Error::<T>::HeartbeatPeriodInvalid);
                    <HeartbeatPeriod<T>>::put(period.clone());
                    Self::deposit_event(Event::HeartbeatPeriodSet { new_heartbeat_period: period });
                    return Ok(Some(<T as Config>::WeightInfo::set_admin_config_reward_heartbeat()).into());
                },
            }
        }
        // Implement me

        /// Offchain call: Submit heartbeat to show node is still alive
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::offchain_submit_heartbeat())]
        pub fn offchain_submit_heartbeat(
            origin: OriginFor<T>,
            node: NodeId<T>,
            reward_period_index: RewardPeriodIndex,
            // This helps prevent signature re-use
            heartbeat_count: u64,
            _signature: <T::SignerId as RuntimeAppPublic>::Signature,
        ) -> DispatchResult {
            ensure_none(origin)?;

            ensure!(<NodeRegistry<T>>::contains_key(&node), Error::<T>::NodeNotRegistered);
            let current_reward_period = RewardPeriod::<T>::get().current;
            let maybe_uptime_info = <NodeUptime<T>>::get(reward_period_index, &node);

            ensure!(current_reward_period == reward_period_index, Error::<T>::InvalidHeartbeat);

            if let Some(uptime_info) = maybe_uptime_info {
                let expected_submission = uptime_info.last_reported +
                    BlockNumberFor::<T>::from(HeartbeatPeriod::<T>::get());
                ensure!(
                    frame_system::Pallet::<T>::block_number() > expected_submission,
                    Error::<T>::DuplicateHeartbeat
                );
                ensure!(heartbeat_count == uptime_info.count, Error::<T>::InvalidHeartbeat);
            } else {
                ensure!(heartbeat_count == 0, Error::<T>::InvalidHeartbeat);
            }

            <NodeUptime<T>>::mutate(&current_reward_period, &node, |maybe_info| {
                if let Some(info) = maybe_info.as_mut() {
                    info.count = info.count.saturating_add(1);
                    info.last_reported = frame_system::Pallet::<T>::block_number();
                } else {
                    *maybe_info = Some(UptimeInfo {
                        count: 1,
                        last_reported: frame_system::Pallet::<T>::block_number(),
                    });
                }
            });

            <TotalUptime<T>>::mutate(&current_reward_period, |total| {
                *total = total.saturating_add(1);
            });

            Self::deposit_event(Event::HeartbeatReceived {
                reward_period_index: current_reward_period,
                node,
            });

            Ok(())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        // Keep this logic light and bounded
        fn on_initialize(n: BlockNumberFor<T>) -> Weight {
            let mut reward_period = RewardPeriod::<T>::get();
            let reward_period_index = reward_period.current;

            if reward_period.should_update(n) {
                reward_period.update(n);
                RewardPeriod::<T>::put(reward_period);

                // take a snapshot of the reward pot amount to pay for the previous reward period
                let reward_amount = RewardAmount::<T>::get();
                let total_heartbeats = <TotalUptime<T>>::get(reward_period_index);
                <RewardPot<T>>::insert(
                    reward_period_index,
                    RewardPotInfo::<BalanceOf<T>>::new(reward_amount, total_heartbeats),
                );

                Self::deposit_event(Event::NewRewardPeriodStarted {
                    reward_period_index: reward_period.current,
                    reward_period_length: reward_period.length,
                    previous_period_reward: reward_amount,
                });

                return <T as Config>::WeightInfo::on_initialise_with_new_reward_period();
            }

            return <T as Config>::WeightInfo::on_initialise_no_reward_period();
        }
        fn offchain_worker(n: BlockNumberFor<T>) {
            log::info!("🌐 OCW for node manager");
            Self::send_heartbeat_if_required(n);
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;
        fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            // Discard unsinged tx's not coming from the local OCW.
            match source {
                TransactionSource::Local | TransactionSource::InBlock => { /* allowed */ },
                _ => return InvalidTransaction::Call.into(),
            }
            match call {
                Call::offchain_submit_heartbeat {
                    node,
                    reward_period_index,
                    heartbeat_count,
                    signature,
                } => {
                    let node_info = NodeRegistry::<T>::get(&node);
                    match node_info {
                        Some(info) => {
                            if Self::signature_is_valid(
                                &(HEARTBEAT_CONTEXT, heartbeat_count, reward_period_index),
                                &info.signing_key,
                                signature,
                            ) {
                                return ValidTransaction::with_tag_prefix("NodeManagerHeartbeat")
                                    .and_provides(call)
                                    .priority(TransactionPriority::max_value())
                                    .build();
                            } else {
                                return InvalidTransaction::Custom(2u8).into();
                            }
                        },
                        _ => InvalidTransaction::Custom(3u8).into(),
                    }
                },
                _ => InvalidTransaction::Call.into(),
            }
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn signature_is_valid<D: Encode>(
            data: &D,
            signer: &T::SignerId,
            signature: &<T::SignerId as RuntimeAppPublic>::Signature,
        ) -> bool {
            let signature_valid =
                data.using_encoded(|encoded_data| signer.verify(&encoded_data, &signature));

            log::info!(
                "🪲 Validating signature: [ data {:?} - account {:?} - signature {:?} ] Result: {}",
                data.encode(),
                signer.encode(),
                signature,
                signature_valid
            );
            return signature_valid
        }
    }
}
