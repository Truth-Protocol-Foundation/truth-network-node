use frame_support::{
    pallet_prelude::*,
    traits::{Get, GetStorageVersion, OnRuntimeUpgrade},
    weights::Weight,
};

use crate::*;

#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;

pub(crate) mod v3 {
    use super::*;
    use frame_support::storage_alias;

    /// V3 type for [`crate::RewardPot`].
    #[storage_alias]
    pub type RewardPot<T: crate::Config> = StorageMap<
        crate::Pallet<T>,
        Blake2_128Concat,
        RewardPeriodIndex,
        RewardPotInfo<BalanceOf<T>>,
        OptionQuery,
    >;

    #[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct RewardPotInfo<Balance> {
        pub total_reward: Balance,
        pub uptime_threshold: u32,
    }

    /// V3 global reward amount, removed in v4.
    #[storage_alias]
    pub type RewardAmount<T: crate::Config> =
        StorageValue<crate::Pallet<T>, BalanceOf<T>, ValueQuery>;
}

/// v3 -> v4: per period reward amounts, set with `set_reward_amount`.
///
/// Ended, unpaid periods keep their snapshotted amount (or get the old `RewardAmount` if they have
/// no pot) as funded with a closed update window, so they are paid as before.
/// `OutstandingRewardToPay` is seeded from them and `RewardAmount` is removed.
pub struct RewardFundingUpgrade<T>(PhantomData<T>);
impl<T: Config> OnRuntimeUpgrade for RewardFundingUpgrade<T> {
    fn on_runtime_upgrade() -> Weight {
        let current = Pallet::<T>::current_storage_version();
        let onchain = Pallet::<T>::on_chain_storage_version();

        if onchain == 3 && current == 4 {
            return migrate_to_per_period_reward_amount::<T>();
        }

        log::info!(
            "ℹ️  Node manager RewardFundingUpgrade skipped. Current storage version {:?} / onchain {:?}",
            current,
            onchain
        );
        T::DbWeight::get().reads(1)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        if Pallet::<T>::on_chain_storage_version() != 3 {
            return Ok(Vec::new());
        }

        let pots: Vec<(RewardPeriodIndex, v3::RewardPotInfo<BalanceOf<T>>)> =
            v3::RewardPot::<T>::iter().collect();
        Ok((pots, v3::RewardAmount::<T>::get()).encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(input: Vec<u8>) -> Result<(), TryRuntimeError> {
        if input.is_empty() {
            return Ok(());
        }

        let (old_pots, old_reward_amount): (
            Vec<(RewardPeriodIndex, v3::RewardPotInfo<BalanceOf<T>>)>,
            BalanceOf<T>,
        ) = Decode::decode(&mut input.as_slice()).map_err(|_| "Invalid pre_upgrade state")?;

        let mut expected_outstanding = BalanceOf::<T>::zero();
        for (period, old_pot) in old_pots {
            let pot = RewardPot::<T>::get(period).ok_or("RewardPot entry lost")?;
            ensure!(pot.total_reward == old_pot.total_reward, "RewardPot amount changed");
            ensure!(pot.uptime_threshold == old_pot.uptime_threshold, "Uptime threshold changed");
            ensure!(pot.funded, "Migrated RewardPot not funded");
            ensure!(pot.reward_end_time == 0, "Migrated RewardPot update window not closed");
            expected_outstanding = expected_outstanding.saturating_add(pot.total_reward);
        }

        let oldest = OldestUnpaidRewardPeriodIndex::<T>::get();
        let current_period = RewardPeriod::<T>::get().current;
        for period in oldest..current_period {
            let pot = RewardPot::<T>::get(period).ok_or("Unpaid period without RewardPot")?;
            ensure!(pot.funded, "Unpaid period not funded");
        }

        let backfilled = OutstandingRewardToPay::<T>::get().saturating_sub(expected_outstanding);
        ensure!(
            OutstandingRewardToPay::<T>::get() >= expected_outstanding,
            "OutstandingRewardToPay too low"
        );
        ensure!(
            backfilled.is_zero() || !old_reward_amount.is_zero(),
            "OutstandingRewardToPay has unexpected backfilled amount"
        );
        ensure!(!v3::RewardAmount::<T>::exists(), "RewardAmount not removed");
        ensure!(Pallet::<T>::on_chain_storage_version() == 4, "Storage version not updated");

        Ok(())
    }
}

fn migrate_to_per_period_reward_amount<T: Config>() -> Weight {
    let old_reward_amount = v3::RewardAmount::<T>::take();
    let mut outstanding = BalanceOf::<T>::zero();
    let mut translated = 0u64;

    RewardPot::<T>::translate::<v3::RewardPotInfo<BalanceOf<T>>, _>(|_period, old| {
        translated.saturating_inc();
        outstanding = outstanding.saturating_add(old.total_reward);
        Some(RewardPotInfo::new(old.total_reward, old.uptime_threshold, 0, true))
    });

    let reward_period = RewardPeriod::<T>::get();
    let oldest = OldestUnpaidRewardPeriodIndex::<T>::get();
    let mut backfilled = 0u64;
    for period in oldest..reward_period.current {
        if !RewardPot::<T>::contains_key(period) {
            backfilled.saturating_inc();
            outstanding = outstanding.saturating_add(old_reward_amount);
            RewardPot::<T>::insert(
                period,
                RewardPotInfo::new(
                    old_reward_amount,
                    Pallet::<T>::calculate_uptime_threshold(reward_period.length),
                    0,
                    true,
                ),
            );
        }
    }

    OutstandingRewardToPay::<T>::put(outstanding);
    STORAGE_VERSION.put::<Pallet<T>>();

    log::info!(
        "✅ Node manager migrated to per period reward amounts. Translated {:?} reward pots, backfilled {:?}, outstanding {:?}",
        translated,
        backfilled,
        outstanding
    );

    let period_reads = reward_period.current.saturating_sub(oldest);
    T::DbWeight::get().reads_writes(
        translated.saturating_add(period_reads).saturating_add(4),
        translated.saturating_add(backfilled).saturating_add(3),
    )
}
