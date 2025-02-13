//! # Node manager benchmarks
// Copyright 2025 Truth Network.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::{EventRecord, RawOrigin};
use prediction_market_primitives::math::fixed::FixedMulDiv;
use sp_runtime::SaturatedConversion;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len().saturating_sub(1 as usize)];
    assert_eq!(event, &system_event);
}

fn set_registrar<T: Config>(registrar: T::AccountId) {
    <NodeRegistrar<T>>::set(Some(registrar.clone()));
}

fn register_new_node<T: Config>(node: NodeId<T>, owner: T::AccountId) -> T::SignerId {
    let key = T::SignerId::generate_pair(None);
    <NodeRegistry<T>>::insert(node.clone(), NodeInfo::new(owner, key.clone()));

    key
}

fn create_heartbeat<T: Config>(node: NodeId<T>, reward_period_index: RewardPeriodIndex) {
    let uptime = <NodeUptime<T>>::get(reward_period_index, node.clone());
    let total_uptime = <TotalUptime<T>>::get(reward_period_index);
    if let Some(uptime) = uptime {
        <NodeUptime<T>>::insert(
            reward_period_index,
            node,
            UptimeInfo::<BlockNumberFor<T>>::new(
                uptime.count + 1,
                frame_system::Pallet::<T>::block_number(),
            ),
        );
    } else {
        let uptime_info =
            UptimeInfo::<BlockNumberFor<T>>::new(1u64, frame_system::Pallet::<T>::block_number());
        <NodeUptime<T>>::insert(reward_period_index, node, uptime_info);
    }

    <TotalUptime<T>>::insert(reward_period_index, total_uptime + 1u64);
}
}

benchmarks! {
    register_node {
        let registrar: T::AccountId = account("registrar", 0, 0);
        set_registrar::<T>(registrar.clone());

        let owner: T::AccountId = account("owner", 1, 1);
        let node: NodeId<T> = account("node", 2, 2);
        let signing_key: T::SignerId = account("signing_key", 3, 3);
    }: register_node(RawOrigin::Signed(registrar.clone()), node.clone(), owner.clone(), signing_key.clone())
    verify {
        assert!(<OwnedNodes<T>>::contains_key(owner.clone(), node.clone()));
        assert!(<NodeRegistry<T>>::contains_key(node.clone()));
        assert_last_event::<T>(Event::NodeRegistered {owner, node}.into());
    }
    set_admin_config_registrar {
        let registrar: T::AccountId = account("registrar", 0, 0);
        set_registrar::<T>(registrar.clone());
        let new_registrar: T::AccountId = account("new_registrar", 0, 0);
        let config = AdminConfig::NodeRegistrar(new_registrar.clone());

    }: set_admin_config(RawOrigin::Root, config.clone())
    verify {
        assert!(<NodeRegistrar<T>>::get() == Some(new_registrar));
    }
    set_admin_config_reward_period {
        let current_reward_period = <RewardPeriod<T>>::get().length;
        let new_reward_period = current_reward_period + 1u32;
        let config = AdminConfig::RewardPeriod(new_reward_period);

    }: set_admin_config(RawOrigin::Root, config.clone())
    verify {
        assert!(<RewardPeriod<T>>::get().length == new_reward_period);
    }

    set_admin_config_reward_heartbeat {
        let current_heartbeat = <HeartbeatPeriod<T>>::get();
        let new_heartbeat = current_heartbeat + 1u32;
        let config = AdminConfig::Heartbeat(new_heartbeat);

    }: set_admin_config(RawOrigin::Root, config.clone())
    verify {
        assert!(<HeartbeatPeriod<T>>::get() == new_heartbeat);
    }

    on_initialise_with_new_reward_period {
        let reward_period = <RewardPeriod<T>>::get();
        let block_number: BlockNumberFor<T> = (reward_period.first + BlockNumberFor::<T>::from(reward_period.length) + 1u32.into()).into();

    }: { Pallet::<T>::on_initialize(block_number) }
    verify {
        let new_reward_period = reward_period.current + 1u64;
        assert!(new_reward_period== <RewardPeriod<T>>::get().current);
        assert_last_event::<T>(Event::NewRewardPeriodStarted {
            reward_period_index: new_reward_period,
            reward_period_length: reward_period.length,
            previous_period_reward: RewardAmount::<T>::get()}.into());
    }

    on_initialise_no_reward_period {
        let reward_period = <RewardPeriod<T>>::get();
        let block_number: BlockNumberFor<T> = BlockNumberFor::<T>::from(reward_period.length) - 1u32.into();

    }: { Pallet::<T>::on_initialize(block_number) }
    verify {
        assert!(reward_period.current== <RewardPeriod<T>>::get().current);
    }

    offchain_submit_heartbeat {
        let reward_period = <RewardPeriod<T>>::get();
        let reward_period_index = reward_period.current;
        let node: NodeId<T> = account("node", 0, 0);
        let owner: T::AccountId = account("owner", 0, 0);
        let signing_key: T::SignerId = register_new_node::<T>(node.clone(), owner.clone());
        create_heartbeat::<T>(node.clone(), reward_period_index);

        // Move forward to the next heartbeat period
        <frame_system::Pallet<T>>::set_block_number(
            frame_system::Pallet::<T>::block_number() + <HeartbeatPeriod<T>>::get().into() + 1u32.into()
        );

        let heartbeat_count = 1u64;
        let signature = signing_key.sign(
            &(HEARTBEAT_CONTEXT, heartbeat_count, reward_period_index).encode()
        ).expect("Error signing");
    }: offchain_submit_heartbeat(RawOrigin::None, node.clone(), reward_period_index, heartbeat_count, signature)
    verify {
        let uptime_info = <NodeUptime<T>>::get(reward_period_index, &node).expect("No uptime info");
        assert!(uptime_info.count == heartbeat_count + 1);
        assert_last_event::<T>(Event::HeartbeatReceived {reward_period_index, node}.into());
    }

}

impl_benchmark_test_suite!(
    Pallet,
    crate::mock::ExtBuilder::build_default().with_genesis_config().as_externality(),
    crate::mock::TestRuntime,
);
