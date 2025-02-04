#![cfg_attr(not(feature = "std"), no_std)]
use frame_support::{
    dispatch::DispatchResult,
    PalletId, pallet_prelude::*,
    storage::{generator::StorageDoubleMap as StorageDoubleMapTrait, PrefixIterator},
    traits::{Currency, IsSubType, StorageVersion},
};
use frame_system::{
    offchain::{SendSignedTransaction, Signer},
    pallet_prelude::*,
};
use parity_scale_codec::{Decode, Encode, FullCodec};
use sp_application_crypto::RuntimeAppPublic;
use sp_core::MaxEncodedLen;
use sp_runtime::{
    offchain::storage::StorageValueRef,
    Saturating,
    scale_info::TypeInfo,
    traits::{AccountIdConversion, Dispatchable, Zero},
};
use pallet_avn::{self as avn};
use common_primitives::constants::REGISTERED_NODE_KEY;

#[cfg(not(feature = "std"))]
use sp_std::prelude::*;

const OCW_ID: &'static [u8; 22] = b"node_manager::last_run";
const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

pub type AVN<T> = avn::Pallet<T>;
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
    pub(super) type NodeRegistry<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId, // node account
        (),
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

    /// Map of reward pot amounts for each reward period.
    #[pallet::storage]
    pub(super) type RewardPot<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        RewardPeriodIndex,
        BalanceOf<T>,
        OptionQuery,
    >;

    /// Tracks the current reward period.
    #[pallet::storage]
    #[pallet::getter(fn current_reward_period)]
    pub(super) type CurrentRewardPeriodIndex<T: Config> = StorageValue<_, RewardPeriodIndex, ValueQuery>;

    /// The earliest reward period that has not been fully paid.
    #[pallet::storage]
    #[pallet::getter(fn oldest_unpaid_period)]
    pub(super) type OldestUnpaidRewardPeriodIndex<T: Config> = StorageValue<_, RewardPeriodIndex, ValueQuery>;

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
        u64,          // uptime measure
        ValueQuery,
    >;

    /// The admin account that can register new nodes
    #[pallet::storage]
    pub type NodeRegistrar<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// The reward period length in blocks.
    #[pallet::storage]
    pub type RewardPeriod<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// The maximum batch size to pay rewards
    #[pallet::storage]
    pub type MaxBatchSize<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// The hearbeat period length in blocks
    #[pallet::storage]
    pub type HeartBeatPeriod<T: Config> = StorageValue<_, u32, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub _phantom: sp_std::marker::PhantomData<T>,
        pub max_batch_size: u32,
        pub reward_period: u32,
        pub heart_beat_period: u32,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                _phantom: Default::default(),
                max_batch_size: 0,
                reward_period: 0,
                heart_beat_period: 0,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            RewardPeriod::<T>::set(self.reward_period.clone());
            MaxBatchSize::<T>::set(self.max_batch_size.clone());
            HeartBeatPeriod::<T>::set(self.heart_beat_period.clone());
        }
    }

    // Pallet Events
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new node has been registered
        NodeRegistered { owner: T::AccountId, node: T::AccountId },
        /// A new reward period  (in blocks) was set.
        RewardPeriodSet { new_reward_period: u32 },
        /// A new reward period was initialized.
        NewRewardPeriodStarted { reward_period_index: RewardPeriodIndex, previous_period_reward: BalanceOf<T> },
        /// We finished paying all nodes for a particular period.
        RewardPayoutCompleted { reward_period_index: RewardPeriodIndex },
        /// Node received a reward.
        RewardPaid { recipient: T::AccountId, amount: BalanceOf<T> },
        /// A new node registrar has been set
        NodeRegistrarSet { new_registrar: T::AccountId },
        /// A new reward payment batch size has been set
        BatchSizeSet { new_size: u32 },
        /// A new heartbeat period (in blocks) was set.
        HeartBeatPeriodSet { new_heart_beat_period: u32 },
    }

    // Pallet Errors
    #[pallet::error]
    pub enum Error<T> {
        InvalidNodePointer,
        InvalidPeriodPointer,
        RegistrarNotSet,
        DuplicateNode,
    }

    #[pallet::config]
    pub trait Config: frame_system::Config + avn::Config {
        type RuntimeEvent: From<Event<Self>>
            + Into<<Self as frame_system::Config>::RuntimeEvent>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type RuntimeCall: Parameter
            + Dispatchable<RuntimeOrigin = <Self as frame_system::Config>::RuntimeOrigin>
            + IsSubType<Call<Self>>
            + From<Call<Self>>;
        type Currency: Currency<Self::AccountId>;
        // Offchain worker specifics
        /// The identifier type for an authority.
        type AuthorityId: Member
            + Parameter
            + RuntimeAppPublic
            + Ord
            + MaybeSerializeDeserialize
            + MaxEncodedLen;

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
            owner: T::AccountId,
            node: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let registrar = NodeRegistrar::<T>::get().ok_or(Error::<T>::RegistrarNotSet)?;
            ensure!(who == registrar, Error::<T>::InvalidNodePointer);

            // TODO: Is there a better way to check for uniqueness without duplicating node storage?
            ensure!(!<NodeRegistry<T>>::contains_key(&node), Error::<T>::DuplicateNode);

            <OwnedNodes<T>>::insert(&owner, &node, ());
            <NodeRegistry<T>>::insert(&node, ());
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
                    <RewardPeriod<T>>::put(period.clone());
                    Self::deposit_event(Event::RewardPeriodSet { new_reward_period: period });
                },
                AdminConfig::BatchSize(size) => {
                    <MaxBatchSize<T>>::put(size.clone());
                    Self::deposit_event(Event::BatchSizeSet { new_size: size });
                },
                AdminConfig::HeartBeat(period) => {
                    <HeartBeatPeriod<T>>::put(period.clone());
                    Self::deposit_event(Event::HeartBeatPeriodSet { new_heart_beat_period: period });
                },
            }

            Ok(())
        }

        /// Offchain call: pay and remove up to `MAX_BATCH_SIZE` nodes in the oldest unpaid period.
        #[pallet::call_index(2)]
        #[pallet::weight(10_000)]
        pub fn offchain_pay_nodes(origin: OriginFor<T>) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            // TODO: Validate transaction
            // Ensure its coming from the node

            let max_batch_size = MaxBatchSize::<T>::get();
            let oldest_period = OldestUnpaidRewardPeriodIndex::<T>::get();
            let current_period = CurrentRewardPeriodIndex::<T>::get();

            // Only pay for completed periods.
            if oldest_period >= current_period {
                return Ok(());
            }

            let mut paid_nodes = Vec::new();
            let mut last_node_paid: Option<T::AccountId> = None;

            // Decide how we start iterating.
            let mut iter = Self::get_iterator(oldest_period)?;
            for (node, uptime) in iter.by_ref().take(max_batch_size as usize) {
                let reward_amount = Self::calculate_reward(uptime);
                Self::pay_reward(node.clone(), reward_amount)?;

                last_node_paid = Some(node.clone());
                paid_nodes.push(node.clone());
                Self::deposit_event(Event::RewardPaid {
                    recipient: node.clone(),
                    amount: reward_amount,
                });
            }

            Self::remove_paid_nodes(oldest_period, paid_nodes)?;

            if iter.next().is_some() {
                Self::update_last_paid_pointer(oldest_period, last_node_paid);
            } else {
                Self::complete_reward_payout(oldest_period)?;
            }

            Ok(())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        // Keep this logic light and bounded
        fn on_initialize(n: BlockNumberFor<T>) -> Weight {
            let reward_period: u32 = RewardPeriod::<T>::get();

            if Self::is_new_reward_period(n, reward_period) {
                let current = CurrentRewardPeriodIndex::<T>::get();
                if let Some(new_period) = current.checked_add(1) {
                    CurrentRewardPeriodIndex::<T>::put(new_period);

                    // take a snap shot of the reward pot amount to pay for the previous reward period
                    let pot_balance = Self::reward_pot_balance();
                    <RewardPot<T>>::insert(current, &pot_balance);

                    Self::deposit_event(Event::NewRewardPeriodStarted {
                        reward_period_index: new_period,
                        previous_period_reward: pot_balance
                    });
                }
            }

            // TODO: Benchmark me
            Weight::zero()
        }

        fn offchain_worker(n: BlockNumberFor<T>) {
            log::info!("👷 OCW for node manager");

            let can_run_ocw_as_validator = Self::can_run_ocw_as_validator(n);

            if can_run_ocw_as_validator && Self::offchain_trigger_payment().unwrap_or(false) {
                // trigger payment
                log::info!("👷 Triggering payment for period: {:?}", OldestUnpaidRewardPeriodIndex::<T>::get());
            }

            if Self::should_send_hearbeat(n) {
                // send heartbeat
                log::info!("👷 Sending heartbeat");
            }
        }
    }

    impl<T: Config> Pallet<T> {
        fn can_run_ocw_as_validator(block_number: BlockNumberFor<T>) -> bool {
            let setup_result = AVN::<T>::pre_run_setup(block_number, OCW_ID.to_vec());
            if let Err(_) = setup_result {
                return false;
            }

            let (this_validator, _) = setup_result.expect("We have a validator");
            let is_primary =
                AVN::<T>::is_primary_for_block(block_number, &this_validator.account_id);

            if is_primary.is_err() {
                log::error!("💔 Error checking if validator is Primary");
                return false;
            }

            return true;
        }

        fn offchain_trigger_payment() -> Result<bool, ()> {
            let oldest_period = OldestUnpaidRewardPeriodIndex::<T>::get();
            let current_period = CurrentRewardPeriodIndex::<T>::get();
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

        fn is_new_reward_period(block_number: BlockNumberFor<T>, reward_period: u32) -> bool {
            if reward_period > 0 {
                let period_bn = BlockNumberFor::<T>::from(reward_period);

                if block_number % period_bn == BlockNumberFor::<T>::zero() {
                    return true;
                }
            }

            return false;
        }

        fn should_send_hearbeat(block_number: BlockNumberFor<T>) -> bool {
            let maybe_registered_node = StorageValueRef::persistent(REGISTERED_NODE_KEY).get::<bool>();
            let registered_node = match maybe_registered_node {
                Ok(Some(is_registered_node)) => is_registered_node,
                _ => false,
            };

            if registered_node {
                let heart_beat_period = HeartBeatPeriod::<T>::get();
                if heart_beat_period > 0 {
                    let period_bn = BlockNumberFor::<T>::from(heart_beat_period);

                    if block_number % period_bn == BlockNumberFor::<T>::zero() {
                        return true;
                    }
                }
            }

            return false;
        }

        fn get_iterator(
            oldest_period: RewardPeriodIndex,
        ) -> Result<PrefixIterator<(T::AccountId, RewardPeriodIndex)>, DispatchError> {
            let maybe_pointer = LastPaidPointer::<T>::get();
            if let Some(pointer) = maybe_pointer {
                // Validate pointer.
                ensure!(pointer.period_index == oldest_period, Error::<T>::InvalidPeriodPointer);
                ensure!(
                    NodeUptime::<T>::contains_key(oldest_period, &pointer.node),
                    Error::<T>::InvalidNodePointer
                );

                // Start iteration just after `(oldest_period, pointer.node)`.
                let final_key = pointer.get_final_key::<T>();
                Ok(NodeUptime::<T>::iter_prefix_from(oldest_period, final_key))
            } else {
                // No pointer => start from the beginning.
                Ok(NodeUptime::<T>::iter_prefix_from(oldest_period, Vec::new()))
            }
        }

        // TODO: Implement this
        fn calculate_reward(_uptime: u64) -> BalanceOf<T>  {
            10u32.into()
        }

        fn pay_reward(_node: T::AccountId, _amount: BalanceOf<T>) -> DispatchResult {
            Ok(())
        }

        fn remove_paid_nodes(
            period_index: RewardPeriodIndex,
            paid_nodes_to_remove: Vec<T::AccountId>,
        ) -> DispatchResult {
            // Remove the paid nodes. We do this separatly to avoid changing the map while iterating
            // it
            for node in &paid_nodes_to_remove {
                NodeUptime::<T>::remove(period_index, node);
            }

            Ok(())
        }

        fn complete_reward_payout(period_index: RewardPeriodIndex) -> DispatchResult {
            // We finished paying all nodes for this period
            OldestUnpaidRewardPeriodIndex::<T>::put(period_index.saturating_add(1));
            LastPaidPointer::<T>::kill();
            Self::deposit_event(Event::RewardPayoutCompleted { reward_period_index: period_index });

            Ok(())
        }

        fn update_last_paid_pointer(period_index: RewardPeriodIndex, last_node_paid: Option<T::AccountId>) {
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

    #[derive(Encode, Decode, TypeInfo, Debug, Clone, PartialEq)]
    pub enum AdminConfig<AccountId> {
        NodeRegistrar(AccountId),
        RewardPeriod(u32),
        BatchSize(u32),
        HeartBeat(u32),
    }
}
