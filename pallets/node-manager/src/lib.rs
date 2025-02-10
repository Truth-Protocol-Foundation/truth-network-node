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
pub mod reward;
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

const PAYOUT_REWARD_CONTEXT: &'static [u8] = b"NodeManager_RewardPayout";
const HEARTBEAT_CONTEXT: &'static [u8] = b"NodeManager_heartbeat";
const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

pub type AVN<T> = avn::Pallet<T>;
pub type Author<T> =
    Validator<<T as avn::Config>::AuthorityId, <T as frame_system::Config>::AccountId>;
pub use pallet::*;

pub(crate) type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
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

    /// The maximum batch size to pay rewards
    #[pallet::storage]
    pub type MaxBatchSize<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// The heartbeat period in blocks
    #[pallet::storage]
    pub type HeartbeatPeriod<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// The total amount to pay out for each period
    #[pallet::storage]
    pub type RewardAmount<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// Map of reward pot amounts for each reward period.
    #[pallet::storage]
    pub(super) type RewardPot<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        RewardPeriodIndex,
        RewardPotInfo<BalanceOf<T>>,
        OptionQuery,
    >;

    /// Tracks the current reward period.
    #[pallet::storage]
    #[pallet::getter(fn current_reward_period)]
    pub(super) type RewardPeriod<T: Config> =
        StorageValue<_, RewardPeriodInfo<BlockNumberFor<T>>, ValueQuery>;

    /// The earliest reward period that has not been fully paid.
    #[pallet::storage]
    #[pallet::getter(fn oldest_unpaid_period)]
    pub(super) type OldestUnpaidRewardPeriodIndex<T: Config> =
        StorageValue<_, RewardPeriodIndex, ValueQuery>;

    /// Stores a `PaymentPointer` for the last node we successfully paid in a given period.
    #[pallet::storage]
    #[pallet::getter(fn last_paid_pointer)]
    pub(super) type LastPaidPointer<T: Config> =
        StorageValue<_, PaymentPointer<T::AccountId>, OptionQuery>;

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
        pub max_batch_size: u32,
        pub reward_period: u32,
        pub heartbeat_period: u32,
        pub reward_amount: BalanceOf<T>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                _phantom: Default::default(),
                max_batch_size: 0,
                reward_period: 0,
                heartbeat_period: 0,
                reward_amount: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            RewardAmount::<T>::set(self.reward_amount);
            MaxBatchSize::<T>::set(self.max_batch_size);
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
        /// We finished paying all nodes for a particular period.
        RewardPayoutCompleted { reward_period_index: RewardPeriodIndex },
        /// Node received a reward.
        RewardPaid {
            reward_period: RewardPeriodIndex,
            owner: T::AccountId,
            node: NodeId<T>,
            amount: BalanceOf<T>,
        },
        /// An error occurred while paying a reward.
        ErrorPayingReward {
            reward_period: RewardPeriodIndex,
            node: NodeId<T>,
            amount: BalanceOf<T>,
            error: DispatchError,
        },
        /// A new node registrar has been set
        NodeRegistrarSet { new_registrar: T::AccountId },
        /// A new reward payment batch size has been set
        BatchSizeSet { new_size: u32 },
        /// A new heartbeat period (in blocks) was set.
        HeartbeatPeriodSet { new_heartbeat_period: u32 },
        /// A new heartbeat has been received
        HeartbeatReceived { reward_period_index: RewardPeriodIndex, node: NodeId<T> },
        /// A new reward amount is set
        RewardAmountSet { new_amount: BalanceOf<T> },
    }

    // Pallet Errors
    #[pallet::error]
    pub enum Error<T> {
        /// The node registrar account is invalid
        InvalidRegistrar,
        /// The node address of the last paid node is not recognised
        InvalidNodePointer,
        /// The period index of the last paid node is invalid
        InvalidPeriodPointer,
        /// The node registrar account is not set
        RegistrarNotSet,
        /// Node has already been registered
        DuplicateNode,
        /// The signing key of the node is invalid
        InvalidSigningKey,
        /// The reward period is invalid
        RewardPeriodInvalid,
        /// The batch size is 0 or invalid
        BatchSizeInvalid,
        /// The heartbeat period is invalid
        HeartbeatPeriodInvalid,
        /// The heartbeat period is 0
        HeartbeatPeriodZero,
        /// The reward pot does not have enough funds to pay rewards
        InsufficientBalanceForReward,
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
        /// The reward amount is 0
        RewardAmountZero,
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

        /// The id of the reward pot.
        #[pallet::constant]
        type RewardPotId: Get<PalletId>;
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
                    return Ok(
                        Some(<T as Config>::WeightInfo::set_admin_config_reward_period()).into()
                    );
                },
                AdminConfig::BatchSize(size) => {
                    ensure!(size > 0, Error::<T>::BatchSizeInvalid);
                    <MaxBatchSize<T>>::put(size.clone());
                    Self::deposit_event(Event::BatchSizeSet { new_size: size });
                    return Ok(
                        Some(<T as Config>::WeightInfo::set_admin_config_reward_batch_size())
                            .into(),
                    );
                },
                AdminConfig::Heartbeat(period) => {
                    let reward_period = RewardPeriod::<T>::get().length;
                    ensure!(period > 0, Error::<T>::HeartbeatPeriodZero);
                    ensure!(period < reward_period, Error::<T>::HeartbeatPeriodInvalid);
                    <HeartbeatPeriod<T>>::put(period.clone());
                    Self::deposit_event(Event::HeartbeatPeriodSet { new_heartbeat_period: period });
                    return Ok(
                        Some(<T as Config>::WeightInfo::set_admin_config_reward_heartbeat()).into()
                    );
                },
                AdminConfig::RewardAmount(amount) => {
                    ensure!(amount > BalanceOf::<T>::zero(), Error::<T>::RewardAmountZero);
                    <RewardAmount<T>>::put(amount.clone());
                    Self::deposit_event(Event::RewardAmountSet { new_amount: amount });
                    return Ok(
                        Some(<T as Config>::WeightInfo::set_admin_config_reward_amount()).into()
                    );
                },
            }
        }

        /// Offchain call: pay and remove up to `MAX_BATCH_SIZE` nodes in the oldest unpaid period.
        #[pallet::call_index(2)]
        #[pallet::weight(10_000)]
        pub fn offchain_pay_nodes(
            origin: OriginFor<T>,
            reward_period_index: RewardPeriodIndex,
            _author: Author<T>,
            _signature: <T::AuthorityId as RuntimeAppPublic>::Signature,
        ) -> DispatchResult {
            ensure_none(origin)?;

            let oldest_period = OldestUnpaidRewardPeriodIndex::<T>::get();
            let current_period = RewardPeriod::<T>::get().current;

            // Only pay for completed periods
            ensure!(
                reward_period_index == oldest_period && oldest_period < current_period,
                Error::<T>::InvalidRewardPaymentRequest
            );

            let total_heartbeats = TotalUptime::<T>::get(&oldest_period);
            let maybe_node_uptime = NodeUptime::<T>::iter_prefix(oldest_period).next();

            if total_heartbeats == 0 && maybe_node_uptime.is_none() {
                // No nodes to pay for this period so complete it
                Self::complete_reward_payout(oldest_period);
                return Ok(());
            }

            ensure!(total_heartbeats > 0, Error::<T>::TotalUptimeNotFound);
            ensure!(maybe_node_uptime.is_some(), Error::<T>::NodeUptimeNotFound);

            let total_reward = Self::get_total_reward(&oldest_period)?;

            let mut paid_nodes = Vec::new();
            let mut last_node_paid: Option<T::AccountId> = None;
            let mut iter;

            match LastPaidPointer::<T>::get() {
                Some(pointer) => {
                    iter = Self::get_iterator_from_last_paid(oldest_period, pointer)?;
                },
                None => {
                    iter = NodeUptime::<T>::iter_prefix(oldest_period);
                },
            }

            for (node, uptime) in iter.by_ref().take(MaxBatchSize::<T>::get() as usize) {
                let reward_amount =
                    Self::calculate_reward(uptime.count, &total_heartbeats, &total_reward)?;
                Self::pay_reward(&oldest_period, node.clone(), reward_amount)?;

                last_node_paid = Some(node.clone());
                paid_nodes.push(node.clone());
            }

            Self::remove_paid_nodes(oldest_period, paid_nodes);

            if iter.next().is_some() {
                Self::update_last_paid_pointer(oldest_period, last_node_paid);
            } else {
                Self::complete_reward_payout(oldest_period);
            }

            Ok(())
        }

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

            let maybe_author = Self::try_get_node_author(n);
            if let Some(author) = maybe_author {
                let oldest_unpaid_period = OldestUnpaidRewardPeriodIndex::<T>::get();
                Self::trigger_payment_if_required(oldest_unpaid_period, author);
                // If this is an author node, we don't need to send a heartbeat
                return;
            }

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
                Call::offchain_pay_nodes { reward_period_index, author, signature } =>
                    if AVN::<T>::signature_is_valid(
                        // Technically this signature can be replayed for the duration of the
                        // reward period but in reality, since we only
                        // accept locally produced transactions and we don'
                        // t propagate them, only an author can submit this transaction and there
                        // is nothing to gain.
                        &(PAYOUT_REWARD_CONTEXT, reward_period_index),
                        &author,
                        signature,
                    ) {
                        ValidTransaction::with_tag_prefix("NodeManagerPayout")
                            .and_provides((call, reward_period_index))
                            .priority(TransactionPriority::max_value())
                            // We don't propagate this transaction,
                            // it ensures only block authors can pay rewards
                            .propagate(false)
                            .build()
                    } else {
                        InvalidTransaction::Custom(1u8).into()
                    },
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
