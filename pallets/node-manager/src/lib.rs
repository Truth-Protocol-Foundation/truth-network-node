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
    offchain::{SendTransactionTypes, Signer, SubmitTransaction},
    pallet_prelude::*,
};
use pallet_avn::{self as avn};
use parity_scale_codec::{Decode, Encode, FullCodec};
use sp_application_crypto::RuntimeAppPublic;
use sp_avn_common::{event_types::Validator, verify_multi_signature};
use sp_core::MaxEncodedLen;
use sp_runtime::{
    offchain::storage::StorageValueRef,
    scale_info::TypeInfo,
    traits::{AccountIdConversion, Dispatchable, IdentifyAccount, Verify},
    transaction_validity::{
        InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity,
        ValidTransaction,
    },
    DispatchError, Perbill, RuntimeDebug, Saturating,
};

// Definition of the crypto to use for signing
pub mod sr25519 {
    mod app_sr25519 {
        use sp_application_crypto::{app_crypto, KeyTypeId, sr25519};
        app_crypto!(sr25519, KeyTypeId(*b"nodk"));
    }

    pub type AuthorityId = app_sr25519::Public;
}

#[cfg(not(feature = "std"))]
use sp_std::prelude::*;

const OCW_ID: &'static [u8; 22] = b"node_manager::last_run";
const PAYOUT_REWARD_CONTEXT: &'static [u8] = b"NodeManager_RewardPayout";
const HEARTBEAT_CONTEXT: &'static [u8] = b"NodeManager_hearbeat";
const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

pub type AVN<T> = avn::Pallet<T>;
pub type Author<T> =
    Validator<<T as avn::Config>::AuthorityId, <T as frame_system::Config>::AccountId>;
pub use pallet::*;

pub(crate) type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
pub(crate) type RewardPeriodIndex = u64;

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
        T::AccountId, // node account
        NodeInfo<T::SignerId, T::AccountId>,
        OptionQuery,
    >;

    /// Registry of nodes with their owners.
    #[pallet::storage]
    pub type OwnedNodes<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId, // OwnerAddress
        Blake2_128Concat,
        T::AccountId, // NodeAddress
        (),
        OptionQuery,
    >;

    /// The admin account that can register new nodes
    #[pallet::storage]
    pub type NodeRegistrar<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// The maximum batch size to pay rewards
    #[pallet::storage]
    pub type MaxBatchSize<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// The hearbeat period length in blocks
    #[pallet::storage]
    pub type HeartbeatPeriod<T: Config> = StorageValue<_, u32, ValueQuery>;

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
        T::AccountId, // node account
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
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                _phantom: Default::default(),
                max_batch_size: 0,
                reward_period: 0,
                heartbeat_period: 0,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            MaxBatchSize::<T>::set(self.max_batch_size.clone());
            HeartbeatPeriod::<T>::set(self.heartbeat_period.clone());

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
        NodeRegistered { owner: T::AccountId, node: T::AccountId },
        /// A new reward period  (in blocks) was set.
        RewardPeriodSet { period_index: u64, old_reward_period: u32, new_reward_period: u32 },
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
            node: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// An error occurred while paying a reward.
        ErrorPayingReward {
            reward_period: RewardPeriodIndex,
            owner: Option<T::AccountId>,
            node: T::AccountId,
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
        HeartbeatReceived { reward_period_index: RewardPeriodIndex, node: T::AccountId },
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
        /// The total reward for the period was not found
        TotalRewardNotFound,
        /// The total uptime for the period was not found
        TotalUptimeNotFound,
        /// The node uptime for the period was not found
        NodeUptimeNotFound,
        /// The node owner was not found
        NodeOwnerNotFound,
        /// The reward payment request is invalid
        InvalidRewardPaymentRequest,
        /// Reward payment is already in progress
        PaymentInProgress,
        /// Heartbeat has already been submitted
        DuplicateHeartbeat,
        /// Heartbeat submission is not valid
        InvalidHeartbeat,
        /// The node is not registered
        NodeNotRegistered,
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
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register a new node
        #[pallet::call_index(0)]
        #[pallet::weight(1)]
        pub fn register_node(
            origin: OriginFor<T>,
            node: T::AccountId,
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
            Self::deposit_event(Event::NodeRegistered { owner, node });

            Ok(())
        }

        /// Set admin configurations
        #[pallet::call_index(1)]
        #[pallet::weight(1)]
        pub fn set_admin_config(
            origin: OriginFor<T>,
            config: AdminConfig<T::AccountId>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            match config {
                AdminConfig::NodeRegistrar(registrar) => {
                    <NodeRegistrar<T>>::set(Some(registrar.clone()));
                    Self::deposit_event(Event::NodeRegistrarSet { new_registrar: registrar });
                },
                AdminConfig::RewardPeriod(period) => {
                    let heartbeat = <HeartbeatPeriod<T>>::get();
                    ensure!(period > heartbeat, Error::<T>::RewardPeriodInvalid);
                    let mut reward_period = RewardPeriod::<T>::get();
                    let (index, old_period) = (reward_period.current, reward_period.length);
                    reward_period.length = period;
                    <RewardPeriod<T>>::put(reward_period);
                    Self::deposit_event(Event::RewardPeriodSet {
                        period_index: index,
                        old_reward_period: old_period,
                        new_reward_period: period,
                    });
                },
                AdminConfig::BatchSize(size) => {
                    ensure!(size > 0, Error::<T>::BatchSizeInvalid);
                    <MaxBatchSize<T>>::put(size.clone());
                    Self::deposit_event(Event::BatchSizeSet { new_size: size });
                },
                AdminConfig::Heartbeat(period) => {
                    let reward_period = RewardPeriod::<T>::get().length;
                    ensure!(period > 0, Error::<T>::HeartbeatPeriodZero);
                    ensure!(period < reward_period, Error::<T>::HeartbeatPeriodInvalid);
                    <HeartbeatPeriod<T>>::put(period.clone());
                    Self::deposit_event(Event::HeartbeatPeriodSet { new_heartbeat_period: period });
                },
            }

            Ok(())
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

            let last_paid_pointer = LastPaidPointer::<T>::get();
            let oldest_period = OldestUnpaidRewardPeriodIndex::<T>::get();
            let current_period = RewardPeriod::<T>::get().current;

            // Only pay for completed periods whose payment has not started
            ensure!(reward_period_index == oldest_period && oldest_period < current_period, Error::<T>::InvalidRewardPaymentRequest);
            ensure!(last_paid_pointer.is_none(), Error::<T>::PaymentInProgress);

            let total_hearbeats = TotalUptime::<T>::get(&oldest_period);
            let maybe_node_uptime = NodeUptime::<T>::iter_prefix(oldest_period).next();

            if total_hearbeats == 0 && maybe_node_uptime.is_none() {
                // No nodes to pay for this period so complete it
                Self::complete_reward_payout(oldest_period);
                return Ok(());
            }

            ensure!(total_hearbeats > 0, Error::<T>::TotalUptimeNotFound);
            ensure!(maybe_node_uptime.is_some(), Error::<T>::NodeUptimeNotFound);

            let total_reward = RewardPot::<T>::get(&oldest_period)
                .ok_or(Error::<T>::TotalRewardNotFound)?
                .total_reward;

            let max_batch_size = MaxBatchSize::<T>::get();
            let maybe_last_paid_pointer = LastPaidPointer::<T>::get();
            let mut paid_nodes = Vec::new();
            let mut last_node_paid: Option<T::AccountId> = None;
            let mut iter;

            match maybe_last_paid_pointer {
                Some(pointer) => {
                    iter = Self::get_iterator_from_last_paid(oldest_period, pointer)?;
                },
                None => {
                    iter = NodeUptime::<T>::iter_prefix(oldest_period);
                },
            }

            for (node, uptime) in iter.by_ref().take(max_batch_size as usize) {
                let reward_amount = Self::calculate_reward(uptime.count, &total_hearbeats, &total_reward);
                Self::pay_reward(&oldest_period, node.clone(), reward_amount);

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
        #[pallet::weight(10_000)]
        pub fn offchain_submit_heartbeat(
            origin: OriginFor<T>,
            node: T::AccountId,
            reward_period_index: RewardPeriodIndex,
            heartbeat_count: u64,
            _signature: <T::SignerId as RuntimeAppPublic>::Signature,
        ) -> DispatchResult {
            ensure_none(origin)?;

            ensure!(<NodeRegistry<T>>::contains_key(&node), Error::<T>::NodeNotRegistered);
            let current_reward_period = RewardPeriod::<T>::get().current;
            let maybe_uptime_info = <NodeUptime<T>>::get(reward_period_index, &node);

            ensure!(current_reward_period == reward_period_index, Error::<T>::InvalidHeartbeat);

            if let Some(uptime_info) = maybe_uptime_info {
                let expected_submission = uptime_info.last_reported + BlockNumberFor::<T>::from(HeartbeatPeriod::<T>::get());
                ensure!(frame_system::Pallet::<T>::block_number() > expected_submission, Error::<T>::DuplicateHeartbeat);
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
                let pot_balance = Self::reward_pot_balance();
                let total_heartbeats = <TotalUptime<T>>::get(reward_period_index);
                <RewardPot<T>>::insert(
                    reward_period_index,
                    RewardPotInfo::<BalanceOf<T>>::new(pot_balance, total_heartbeats),
                );

                Self::deposit_event(Event::NewRewardPeriodStarted {
                    reward_period_index: reward_period.current,
                    reward_period_length: reward_period.length,
                    previous_period_reward: pot_balance,
                });
            }

            // TODO: Benchmark me
            Weight::zero()
        }

        fn offchain_worker(n: BlockNumberFor<T>) {
            log::info!("🌐 OCW for node manager");

            let reward_period_index = OldestUnpaidRewardPeriodIndex::<T>::get();

            let maybe_author = Self::try_get_node_author(n);
            if let Some(ref author) = maybe_author {
                // This is an author node
                if Self::offchain_trigger_payment().unwrap_or(false) {
                    log::info!("🌐 Triggering payment for period: {:?}", reward_period_index);

                    let signature = author
                        .key
                        .sign(&(PAYOUT_REWARD_CONTEXT, reward_period_index).encode());

                    match signature {
                        Some(signature) => {
                            let call = Call::<T>::offchain_pay_nodes {
                                reward_period_index,
                                author: author.clone(),
                                signature,
                            };

                            if let Err(e) = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()) {
                                log::error!("💔 Error submitting transaction to trigger payment. Period: {:?}, Error: {:?}", reward_period_index, e);
                            }
                        },
                        None => {
                            log::error!("💔 Error signing payment transaction. Period: {:?}", reward_period_index);
                        },
                    }
                }

                // If this is an author node, we don't need to send a heartbeat
                return;
            }

            let maybe_node_key = Self::get_node();
            if let Some((node, signing_key)) = maybe_node_key {
                if Self::should_send_hearbeat(n, reward_period_index, &node) {
                    log::info!("🌐 Sending heartbeat");

                    let heartbeat_count = <NodeUptime<T>>::get(reward_period_index, &node)
                        .map(|info| info.count)
                        .unwrap_or(0);

                    log::info!("🌐 Heartbeat count: {:?}", heartbeat_count);
                    let signature = signing_key
                        .sign(&(HEARTBEAT_CONTEXT, heartbeat_count, reward_period_index).encode());

                    match signature {
                        Some(signature) => {
                            let call = Call::<T>::offchain_submit_heartbeat {
                                node,
                                reward_period_index,
                                heartbeat_count,
                                signature,
                            };

                            if let Err(e) = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()) {
                                log::error!("💔 Error submitting heartbeat transaction. Period: {:?}, Heartbeat count: {:?}, Error: {:?}", reward_period_index, heartbeat_count, e);
                            }
                            log::info!("🌐 transaction sent");
                        },
                        None => {
                            log::error!("💔 Error signing heartbeat transaction. Period: {:?}", reward_period_index);
                        },
                    }
                }
            }
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;
        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::offchain_pay_nodes { reward_period_index, author, signature } =>
                    if AVN::<T>::signature_is_valid(
                        &(PAYOUT_REWARD_CONTEXT, reward_period_index),
                        &author,
                        signature,
                    ) {
                        ValidTransaction::with_tag_prefix("NodeManagerPayout")
                            .and_provides((call, reward_period_index))
                            .priority(TransactionPriority::max_value())
                            .build()
                    } else {
                        InvalidTransaction::Custom(1u8).into()
                    },
                Call::offchain_submit_heartbeat { node, reward_period_index, heartbeat_count, signature } => {
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

            log::error!(
                "🪲 Validating signature: [ data {:?} - account {:?} - signature {:?} ] Result: {}",
                data.encode(),
                signer.encode(),
                signature,
                signature_valid
            );
            return signature_valid
        }

        fn try_get_node_author(block_number: BlockNumberFor<T>) -> Option<Author<T>> {
            let setup_result = AVN::<T>::pre_run_setup(block_number, OCW_ID.to_vec());
            if let Err(_) = setup_result {
                return None;
            }

            let (this_author, _) = setup_result.expect("We have an author");
            let is_primary = AVN::<T>::is_primary_for_block(block_number, &this_author.account_id);

            if is_primary.is_err() {
                log::error!("💔 Error checking if author is Primary");
                return None;
            }

            return Some(this_author);
        }

        fn offchain_trigger_payment() -> Result<bool, ()> {
            let oldest_period = OldestUnpaidRewardPeriodIndex::<T>::get();
            let current_period = RewardPeriod::<T>::get().current;
            let last_paid_pointer = LastPaidPointer::<T>::get();

            if let Some(ref pointer) = last_paid_pointer {
                // payment is in progress
                if oldest_period != pointer.period_index {
                    log::error!("💔 Reward payment in progress for period {:?}, but oldest period is {:?}. Aborting",
                        pointer.period_index,
                        oldest_period
                    );

                    return Err(());
                }
            }

            if oldest_period < current_period && last_paid_pointer.is_none() {
                log::info!(
                    "👷 Triggering payment for period: {:?}. Current period: {:?}",
                    oldest_period,
                    current_period
                );

                return Ok(true);
            }

            return Ok(false);
        }

        fn get_node() -> Option<(T::AccountId, T::SignerId)> {
            let registered_node_id = StorageValueRef::persistent(REGISTERED_NODE_KEY)
                .get::<T::AccountId>()
                .ok()
                .flatten()?;

            let mut local_keys = T::SignerId::all();
            local_keys.sort();

            if let Some(node_info) = NodeRegistry::<T>::get(&registered_node_id) {
                // If the registered node’s signing key is present locally, return it.
                if local_keys.binary_search(&node_info.signing_key).is_ok() {
                    return Some((registered_node_id, node_info.signing_key));
                } else {
                    log::warn!("🔐 Signing key not found for registered node.");
                }
            }

            // we couldn't find a signing key for the registered_node_id, so check all registered nodes
            // This is potentially very slow so only do it as a last resort
            return NodeRegistry::<T>::iter()
                .filter_map(move |(node_id, info)| {
                    local_keys.binary_search(&info.signing_key).ok().map(|_| (node_id, info.signing_key))
                })
                .nth(0)
        }

        fn should_send_hearbeat(block_number: BlockNumberFor<T>, reward_period_index: RewardPeriodIndex, node: &T::AccountId) -> bool {
            let heartbeat_period = HeartbeatPeriod::<T>::get();
            if heartbeat_period > 0 {
                // let last_submission = StorageValueRef::persistent(HEARTBEAT_CONTEXT)
                //     .get::<BlockNumberFor<T>>()
                //     .unwrap_or(BlockNumberFor::<T>::zero());
                match <NodeUptime<T>>::get(reward_period_index, node) {
                    Some(uptime_info) => {
                        let last_submission = uptime_info.last_reported;
                        return block_number >= last_submission + BlockNumberFor::<T>::from(heartbeat_period);
                    },
                    None => {
                        return true;
                    },
                }
            }

            return false;
        }

        fn get_iterator_from_last_paid(
            oldest_period: RewardPeriodIndex,
            last_paid_pointer: PaymentPointer<T::AccountId>,
        ) -> Result<PrefixIterator<(T::AccountId, UptimeInfo<BlockNumberFor<T>>)>, DispatchError> {
            ensure!(
                last_paid_pointer.period_index == oldest_period,
                Error::<T>::InvalidPeriodPointer
            );
            ensure!(
                NodeUptime::<T>::contains_key(oldest_period, &last_paid_pointer.node),
                Error::<T>::InvalidNodePointer
            );

            // Start iteration just after `(oldest_period, last_paid_pointer.node)`.
            let final_key = last_paid_pointer.get_final_key::<T>();
            Ok(NodeUptime::<T>::iter_prefix_from(oldest_period, final_key))
        }

        fn calculate_reward(
            uptime: u64,
            total_uptime: &u64,
            total_reward: &BalanceOf<T>,
        ) -> BalanceOf<T> {
            let fraction = Perbill::from_rational(uptime, *total_uptime);
            fraction * *total_reward
        }

        fn pay_reward(period: &RewardPeriodIndex, node: T::AccountId, amount: BalanceOf<T>) {
            let handle_error = |e, owner| {
                log::error!("💔 Error paying reward. Owner: {:?}, Reward period: {:?}, Node {:?}, Amount: {:?}. Error: {:?}",
                   owner, period, node, amount, e
                );
                Self::deposit_event(Event::ErrorPayingReward {
                    reward_period: *period,
                    node: node.clone(),
                    owner,
                    amount,
                    error: e,
                });
            };

            let node_owner = match <NodeRegistry<T>>::get(&node) {
                Some(info) => info.owner,
                None => {
                    handle_error(Error::<T>::NodeOwnerNotFound.into(), None);
                    return;
                },
            };

            let reward_pot_account_id = Self::compute_reward_account_id();

            let result = T::Currency::transfer(
                &reward_pot_account_id,
                &node_owner,
                amount,
                ExistenceRequirement::KeepAlive,
            );

            match result {
                Ok(_) => Self::deposit_event(Event::RewardPaid {
                    reward_period: *period,
                    owner: node_owner,
                    node,
                    amount,
                }),
                Err(e) => handle_error(e, Some(node_owner)),
            }
        }

        fn remove_paid_nodes(
            period_index: RewardPeriodIndex,
            paid_nodes_to_remove: Vec<T::AccountId>,
        ) {
            // Remove the paid nodes. We do this separatly to avoid changing the map while iterating
            // it
            for node in &paid_nodes_to_remove {
                NodeUptime::<T>::remove(period_index, node);
            }
        }

        fn complete_reward_payout(period_index: RewardPeriodIndex) {
            // We finished paying all nodes for this period
            OldestUnpaidRewardPeriodIndex::<T>::put(period_index.saturating_add(1));
            LastPaidPointer::<T>::kill();
            <TotalUptime<T>>::remove(period_index);
            <RewardPot<T>>::remove(period_index);
            Self::deposit_event(Event::RewardPayoutCompleted { reward_period_index: period_index });
        }

        fn update_last_paid_pointer(
            period_index: RewardPeriodIndex,
            last_node_paid: Option<T::AccountId>,
        ) {
            // We have more to pay next time.
            if let Some(node) = last_node_paid {
                // Remember where we left off.
                LastPaidPointer::<T>::put(PaymentPointer { period_index, node });
            } else {
                // After a payment round, we didn't pay anyone but there are still nodes to pay
                // This should never happen so start over again
                LastPaidPointer::<T>::kill();
            };
        }

        /// The account ID of the reward pot.
        pub fn compute_reward_account_id() -> T::AccountId {
            T::RewardPotId::get().into_account_truncating()
        }

        /// The total amount of funds stored in this pallet
        pub fn reward_pot_balance() -> BalanceOf<T> {
            // Must never be less than 0 but better be safe.
            <T as pallet::Config>::Currency::free_balance(&Self::compute_reward_account_id())
                .saturating_sub(<T as pallet::Config>::Currency::minimum_balance())
        }
    }

    #[derive(Encode, Decode, Default, Clone, PartialEq, Debug, Eq, TypeInfo, MaxEncodedLen)]
    pub struct PaymentPointer<AccountId> {
        pub period_index: RewardPeriodIndex,
        pub node: AccountId,
    }

    impl<AccountId: Clone + FullCodec + MaxEncodedLen + TypeInfo> PaymentPointer<AccountId> {
        /// Return the *final* storage key for NodeUptime<(period, node)>.
        /// This positions iteration beyond (period,node), preventing double payments.
        pub fn get_final_key<T: Config<AccountId = AccountId>>(&self) -> Vec<u8> {
            crate::pallet::NodeUptime::<T>::storage_double_map_final_key(
                self.period_index,
                self.node.clone(),
            )
        }
    }

    #[derive(Encode, Decode, Default, Clone, PartialEq, Debug, Eq, TypeInfo, MaxEncodedLen)]
    pub struct NodeInfo<SignerId, AccountId> {
        pub owner: AccountId,
        pub signing_key: SignerId,
    }

    impl<AccountId: Clone + FullCodec + MaxEncodedLen + TypeInfo, SignerId: Clone + FullCodec + MaxEncodedLen + TypeInfo> NodeInfo<SignerId, AccountId> {
        pub fn new(owner: AccountId, signing_key: SignerId) -> NodeInfo<SignerId, AccountId> {
            NodeInfo { owner, signing_key }
        }
    }

    #[derive(Encode, Decode, TypeInfo, Debug, Clone, PartialEq)]
    pub enum AdminConfig<AccountId> {
        NodeRegistrar(AccountId),
        RewardPeriod(u32),
        BatchSize(u32),
        Heartbeat(u32),
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
/// The current era index and transition information
pub struct RewardPeriodInfo<BlockNumber> {
    /// Current era index
    pub current: RewardPeriodIndex,
    /// The first block of the current era
    pub first: BlockNumber,
    /// The length of the current era in number of blocks
    pub length: u32,
}

impl<
        B: Copy + sp_std::ops::Add<Output = B> + sp_std::ops::Sub<Output = B> + From<u32> + PartialOrd,
    > RewardPeriodInfo<B>
{
    pub fn new(current: RewardPeriodIndex, first: B, length: u32) -> RewardPeriodInfo<B> {
        RewardPeriodInfo { current, first, length }
    }

    /// Check if the reward period should be updated
    pub fn should_update(&self, now: B) -> bool {
        now - self.first >= self.length.into()
    }

    /// New reward period
    pub fn update(&mut self, now: B) {
        self.current = self.current.saturating_add(1u64);
        self.first = now;
    }
}

impl<
        B: Copy + sp_std::ops::Add<Output = B> + sp_std::ops::Sub<Output = B> + From<u32> + PartialOrd,
    > Default for RewardPeriodInfo<B>
{
    fn default() -> RewardPeriodInfo<B> {
        RewardPeriodInfo::new(1u64, 1u32.into(), 20u32)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct RewardPotInfo<Balance> {
    /// The total reward to pay out
    pub total_reward: Balance,
    /// The total uptime for the reward period
    pub total_uptime: u64,
}

impl<Balance: Copy> RewardPotInfo<Balance> {
    pub fn new(total_reward: Balance, total_uptime: u64) -> RewardPotInfo<Balance> {
        RewardPotInfo { total_reward, total_uptime }
    }
}

#[derive(Copy, Clone, PartialEq, Default, Eq, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct UptimeInfo<BlockNumber> {
    /// Number of uptime reported
    pub count: u64,
    /// Block number when the uptime was last reported
    pub last_reported: BlockNumber,
}

impl<BlockNumber: Copy> UptimeInfo<BlockNumber> {
    pub fn new(count: u64, last_reported: BlockNumber) -> UptimeInfo<BlockNumber> {
        UptimeInfo { count, last_reported }
    }
}
