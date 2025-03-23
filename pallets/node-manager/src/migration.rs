use frame_support::{
    pallet_prelude::*,
    traits::{Get, GetStorageVersion, OnRuntimeUpgrade},
    weights::Weight,
};

use crate::*;

#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;

mod v1 {
    use super::*;
    use frame_support::storage_alias;

    #[derive(
        Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen, Default,
    )]
    pub struct RewardPeriodInfo<BlockNumber> {
        pub current: RewardPeriodIndex,
        pub first: BlockNumber,
        pub length: u32,
    }

    /// V2 type for [`crate::ActiveEthRange`].
    #[storage_alias]
    pub type RewardPeriod<T: crate::Config> =
        StorageValue<crate::Pallet<T>, RewardPeriodInfo<BlockNumberFor<T>>, ValueQuery>;
}

pub struct RewardPeriodInfoUpgrade<T>(PhantomData<T>);
impl<T: Config> OnRuntimeUpgrade for RewardPeriodInfoUpgrade<T> {
    fn on_runtime_upgrade() -> Weight {
        let current = Pallet::<T>::current_storage_version();
        let onchain = Pallet::<T>::on_chain_storage_version();

        log::info!(
            "ℹ️  Node manager `RewardPeriodUpgrade` invoked with current storage version {:?} / onchain {:?}",
            current,
            onchain
        );

        let mut consumed_weight = Weight::zero();
        if onchain == 1 {
            consumed_weight.saturating_accrue(update_reward_period::<T>());
        }

        consumed_weight
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        let old_reward_period = v1::RewardPeriod::<T>::get();
        Ok(old_reward_period.encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(input: Vec<u8>) -> Result<(), TryRuntimeError> {
        let v2_reward_info: v1::RewardPeriodInfo<BlockNumberFor<T>> =
            Decode::decode(&mut input.as_slice()).expect("v1 RewardPeriodInfo is invalid");

        let current_reward_period = RewardPeriod::<T>::get();
        assert_eq!(current_reward_period.current, v2_reward_info.current);
        assert_eq!(current_reward_period.first, v2_reward_info.first);
        assert_eq!(current_reward_period.length, v2_reward_info.length);
        assert_eq!(current_reward_period.uptime_threshold, u32::MAX);

        assert_eq!(<MinUptimeThreshold<T>>::get(), Some(Pallet::<T>::get_default_threshold()));

        let current = Pallet::<T>::current_storage_version();
        let onchain = Pallet::<T>::on_chain_storage_version();
        assert!(onchain == 2 && current == 2);
        Ok(())
    }
}

// Set the min uptime to a very high number. The next reward period will adjust it.
fn update_reward_period<T: Config>() -> Weight {
    let old_reward_period = v1::RewardPeriod::<T>::take();

    RewardPeriod::<T>::put(RewardPeriodInfo::<BlockNumberFor<T>> {
        current: old_reward_period.current,
        first: old_reward_period.first,
        length: old_reward_period.length,
        uptime_threshold: u32::MAX,
    });

    <MinUptimeThreshold<T>>::put(Pallet::<T>::get_default_threshold());

    STORAGE_VERSION.put::<Pallet<T>>();

    log::info!("✅ RewardPeriodInfo updated successfully");
    return T::DbWeight::get().reads_writes(3, 1);
}
