//Copyright 2025 Truth Network.

#![cfg(test)]

use crate::{
    migration::{v3, RewardFundingUpgrade},
    mock::*,
    *,
};
use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade};

const OLD_REWARD_AMOUNT: u128 = 7 * REWARD_AMOUNT;

fn set_v3_state(oldest_unpaid: RewardPeriodIndex, current: RewardPeriodIndex) {
    StorageVersion::new(3).put::<NodeManager>();
    v3::RewardAmount::<TestRuntime>::put(OLD_REWARD_AMOUNT);
    OldestUnpaidRewardPeriodIndex::<TestRuntime>::put(oldest_unpaid);
    RewardPeriod::<TestRuntime>::mutate(|p| p.current = current);
}

fn old_pot(total_reward: u128, uptime_threshold: u32) -> v3::RewardPotInfo<u128> {
    v3::RewardPotInfo { total_reward, uptime_threshold }
}

#[test]
fn existing_reward_pots_are_migrated_as_funded_and_payable() {
    let mut ext = ExtBuilder::build_default().with_genesis_config().as_externality();
    ext.execute_with(|| {
        Timestamp::set_timestamp(1_000_000);
        set_v3_state(3, 5);
        v3::RewardPot::<TestRuntime>::insert(3, old_pot(100, 11));
        v3::RewardPot::<TestRuntime>::insert(4, old_pot(250, 12));

        RewardFundingUpgrade::<TestRuntime>::on_runtime_upgrade();

        assert_eq!(RewardPot::<TestRuntime>::get(3), Some(RewardPotInfo::new(100, 11, 0, true)));
        assert_eq!(RewardPot::<TestRuntime>::get(4), Some(RewardPotInfo::new(250, 12, 0, true)));
        // Periods snapshotted under the old rules can be paid straight away
        assert!(NodeManager::get_payable_reward_pot(3).is_ok());
        assert!(NodeManager::get_payable_reward_pot(4).is_ok());

        assert_eq!(OutstandingRewardToPay::<TestRuntime>::get(), 350);
        assert!(!v3::RewardAmount::<TestRuntime>::exists());
        assert_eq!(NodeManager::on_chain_storage_version(), StorageVersion::new(4));
    });
}

#[test]
fn unpaid_periods_without_a_pot_are_backfilled_with_the_old_reward_amount() {
    let mut ext = ExtBuilder::build_default().with_genesis_config().as_externality();
    ext.execute_with(|| {
        set_v3_state(2, 5);
        v3::RewardPot::<TestRuntime>::insert(3, old_pot(100, 11));

        RewardFundingUpgrade::<TestRuntime>::on_runtime_upgrade();

        let threshold =
            NodeManager::calculate_uptime_threshold(RewardPeriod::<TestRuntime>::get().length);
        for period in [2u64, 4u64] {
            assert_eq!(
                RewardPot::<TestRuntime>::get(period),
                Some(RewardPotInfo::new(OLD_REWARD_AMOUNT, threshold, 0, true))
            );
        }
        assert_eq!(RewardPot::<TestRuntime>::get(3), Some(RewardPotInfo::new(100, 11, 0, true)));
        // The current period is not backfilled: it has to be funded once it ends
        assert!(RewardPot::<TestRuntime>::get(5).is_none());
        assert_eq!(OutstandingRewardToPay::<TestRuntime>::get(), 100 + 2 * OLD_REWARD_AMOUNT);
    });
}

#[test]
fn migration_does_nothing_if_not_on_v3() {
    let mut ext = ExtBuilder::build_default().with_genesis_config().as_externality();
    ext.execute_with(|| {
        StorageVersion::new(4).put::<NodeManager>();
        v3::RewardAmount::<TestRuntime>::put(OLD_REWARD_AMOUNT);
        RewardPot::<TestRuntime>::insert(1, RewardPotInfo::new(100, 11, 42, false));

        RewardFundingUpgrade::<TestRuntime>::on_runtime_upgrade();

        assert_eq!(RewardPot::<TestRuntime>::get(1), Some(RewardPotInfo::new(100, 11, 42, false)));
        assert_eq!(v3::RewardAmount::<TestRuntime>::get(), OLD_REWARD_AMOUNT);
        assert_eq!(OutstandingRewardToPay::<TestRuntime>::get(), 0);
    });
}
