//Copyright 2025 Truth Network.

#![cfg(test)]

use crate::{
    migration::{v3, v4_testnet, MainnetAlignmentUpgrade},
    mock::*,
    *,
};
use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade};

const OLD_REWARD_AMOUNT: u128 = 7 * REWARD_AMOUNT;

fn set_legacy_state(version: u16, oldest_unpaid: RewardPeriodIndex, current: RewardPeriodIndex) {
    StorageVersion::new(version).put::<NodeManager>();
    v3::RewardAmount::<TestRuntime>::put(OLD_REWARD_AMOUNT);
    OldestUnpaidRewardPeriodIndex::<TestRuntime>::put(oldest_unpaid);
    RewardPeriod::<TestRuntime>::mutate(|p| p.current = current);
}

fn old_pot(total_reward: u128, uptime_threshold: u32) -> v3::RewardPotInfo<u128> {
    v3::RewardPotInfo { total_reward, uptime_threshold }
}

fn owner(seed: u8) -> AccountId {
    TestAccount::new([seed; 32]).account_id()
}

fn version() -> StorageVersion {
    NodeManager::on_chain_storage_version()
}

/// Runs the upgrade, wrapped in its try-runtime checks when that feature is enabled.
fn upgrade() {
    #[cfg(feature = "try-runtime")]
    let state = MainnetAlignmentUpgrade::<TestRuntime>::pre_upgrade().expect("pre_upgrade");

    MainnetAlignmentUpgrade::<TestRuntime>::on_runtime_upgrade();

    #[cfg(feature = "try-runtime")]
    MainnetAlignmentUpgrade::<TestRuntime>::post_upgrade(state).expect("post_upgrade");
}

#[test]
fn testnet_reward_pots_are_migrated_as_funded_and_payable() {
    let mut ext = ExtBuilder::build_default().with_genesis_config().as_externality();
    ext.execute_with(|| {
        Timestamp::set_timestamp(1_000_000);
        set_legacy_state(4, 3, 5);
        v3::RewardPot::<TestRuntime>::insert(3, old_pot(100, 11));
        v3::RewardPot::<TestRuntime>::insert(4, old_pot(250, 12));

        upgrade();

        assert_eq!(RewardPot::<TestRuntime>::get(3), Some(RewardPotInfo::new(100, 11, 0, true)));
        assert_eq!(RewardPot::<TestRuntime>::get(4), Some(RewardPotInfo::new(250, 12, 0, true)));
        // Periods snapshotted under the old rules can be paid straight away
        assert!(NodeManager::get_payable_reward_pot(3).is_ok());
        assert!(NodeManager::get_payable_reward_pot(4).is_ok());

        assert_eq!(OutstandingRewardToPay::<TestRuntime>::get(), 350);
        assert!(!v3::RewardAmount::<TestRuntime>::exists());
        assert_eq!(version(), StorageVersion::new(5));
    });
}

#[test]
fn testnet_owned_node_counts_are_removed() {
    let mut ext = ExtBuilder::build_default().with_genesis_config().as_externality();
    ext.execute_with(|| {
        StorageVersion::new(4).put::<NodeManager>();
        v4_testnet::OwnedNodesCount::<TestRuntime>::insert(owner(1), 3);
        v4_testnet::OwnedNodesCount::<TestRuntime>::insert(owner(2), 1);

        upgrade();

        assert!(v4_testnet::OwnedNodesCount::<TestRuntime>::iter_keys().next().is_none());
        assert_eq!(version(), StorageVersion::new(5));
    });
}

#[test]
fn testnet_layout_is_detected_from_reward_pots_alone() {
    let mut ext = ExtBuilder::build_default().with_genesis_config().as_externality();
    ext.execute_with(|| {
        StorageVersion::new(4).put::<NodeManager>();
        OldestUnpaidRewardPeriodIndex::<TestRuntime>::put(3);
        RewardPeriod::<TestRuntime>::mutate(|p| p.current = 4);
        v3::RewardPot::<TestRuntime>::insert(3, old_pot(100, 11));

        upgrade();

        assert_eq!(RewardPot::<TestRuntime>::get(3), Some(RewardPotInfo::new(100, 11, 0, true)));
        assert_eq!(OutstandingRewardToPay::<TestRuntime>::get(), 100);
        assert_eq!(version(), StorageVersion::new(5));
    });
}

#[test]
fn unpaid_periods_without_a_pot_are_backfilled_with_the_old_reward_amount() {
    let mut ext = ExtBuilder::build_default().with_genesis_config().as_externality();
    ext.execute_with(|| {
        set_legacy_state(4, 2, 5);
        v3::RewardPot::<TestRuntime>::insert(3, old_pot(100, 11));

        upgrade();

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
fn v3_state_is_migrated_like_testnet() {
    let mut ext = ExtBuilder::build_default().with_genesis_config().as_externality();
    ext.execute_with(|| {
        set_legacy_state(3, 3, 4);
        v3::RewardPot::<TestRuntime>::insert(3, old_pot(100, 11));

        upgrade();

        assert_eq!(RewardPot::<TestRuntime>::get(3), Some(RewardPotInfo::new(100, 11, 0, true)));
        assert_eq!(OutstandingRewardToPay::<TestRuntime>::get(), 100);
        assert!(!v3::RewardAmount::<TestRuntime>::exists());
        assert_eq!(version(), StorageVersion::new(5));
    });
}

#[test]
fn mainnet_state_only_gets_the_new_storage_version() {
    let mut ext = ExtBuilder::build_default().with_genesis_config().as_externality();
    ext.execute_with(|| {
        StorageVersion::new(4).put::<NodeManager>();
        OldestUnpaidRewardPeriodIndex::<TestRuntime>::put(3);
        RewardPeriod::<TestRuntime>::mutate(|p| p.current = 5);
        // An unfunded pot with an open window, and a funded one
        RewardPot::<TestRuntime>::insert(3, RewardPotInfo::new(250, 12, 1_000, true));
        RewardPot::<TestRuntime>::insert(4, RewardPotInfo::new(0, 12, 2_000, false));
        OutstandingRewardToPay::<TestRuntime>::put(250);

        upgrade();

        assert_eq!(
            RewardPot::<TestRuntime>::get(3),
            Some(RewardPotInfo::new(250, 12, 1_000, true))
        );
        assert_eq!(RewardPot::<TestRuntime>::get(4), Some(RewardPotInfo::new(0, 12, 2_000, false)));
        assert_eq!(OutstandingRewardToPay::<TestRuntime>::get(), 250);
        assert_eq!(version(), StorageVersion::new(5));
    });
}

#[test]
fn mainnet_pots_left_in_a_testnet_layout_are_not_converted() {
    let mut ext = ExtBuilder::build_default().with_genesis_config().as_externality();
    ext.execute_with(|| {
        set_legacy_state(4, 3, 5);
        v3::RewardPot::<TestRuntime>::insert(3, old_pot(100, 11));
        RewardPot::<TestRuntime>::insert(4, RewardPotInfo::new(0, 12, 2_000, false));

        upgrade();

        assert_eq!(RewardPot::<TestRuntime>::get(3), Some(RewardPotInfo::new(100, 11, 0, true)));
        assert_eq!(RewardPot::<TestRuntime>::get(4), Some(RewardPotInfo::new(0, 12, 2_000, false)));
        assert_eq!(OutstandingRewardToPay::<TestRuntime>::get(), 100);
    });
}

#[test]
fn migration_does_nothing_if_already_on_v5() {
    let mut ext = ExtBuilder::build_default().with_genesis_config().as_externality();
    ext.execute_with(|| {
        StorageVersion::new(5).put::<NodeManager>();
        v3::RewardAmount::<TestRuntime>::put(OLD_REWARD_AMOUNT);
        v3::RewardPot::<TestRuntime>::insert(1, old_pot(100, 11));

        upgrade();

        assert_eq!(v3::RewardPot::<TestRuntime>::get(1), Some(old_pot(100, 11)));
        assert_eq!(v3::RewardAmount::<TestRuntime>::get(), OLD_REWARD_AMOUNT);
        assert_eq!(OutstandingRewardToPay::<TestRuntime>::get(), 0);
    });
}
