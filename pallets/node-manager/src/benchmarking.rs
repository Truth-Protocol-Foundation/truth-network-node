//! # Node manager benchmarks
// Copyright 2025 Truth Network.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::{EventRecord, RawOrigin};
use prediction_market_primitives::math::fixed::FixedMulDiv;
use sp_avn_common::Proof;
use sp_runtime::SaturatedConversion;

// Macro for comparing fixed point u128.
#[allow(unused_macros)]
macro_rules! assert_approx {
    ($left:expr, $right:expr, $precision:expr $(,)?) => {
        match (&$left, &$right, &$precision) {
            (left_val, right_val, precision_val) => {
                let diff = if *left_val > *right_val {
                    *left_val - *right_val
                } else {
                    *right_val - *left_val
                };
                if diff > $precision {
                    panic!("{:?} is not {:?}-close to {:?}", *left_val, *precision_val, *right_val);
                }
            },
        }
    };
}

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
    <NodeRegistry<T>>::insert(node.clone(), NodeInfo::new(owner.clone(), key.clone()));
    <OwnedNodes<T>>::insert(owner, node, ());
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

fn fund_reward_pot<T: Config>() {
    let reward_amount = RewardAmount::<T>::get() * 2000u32.into();
    let reward_pot_address = Pallet::<T>::compute_reward_account_id();
    T::Currency::make_free_balance_be(&reward_pot_address, reward_amount);
}

fn create_author<T: Config>() -> Author<T> {
    let account = account("dummy_validator", 0, 0);
    let key = <T as avn::Config>::AuthorityId::generate_pair(Some("//bob".as_bytes().to_vec()));
    Author::<T>::new(account, key)
}

fn create_nodes_and_hearbeat<T: Config>(
    owner: T::AccountId,
    reward_period_index: RewardPeriodIndex,
    node_to_create: u32,
) {
    for i in 1..=node_to_create {
        let node: NodeId<T> = account("node", i, i);
        let _ = register_new_node::<T>(node.clone(), owner.clone());
        create_heartbeat::<T>(node.clone(), reward_period_index);
    }
}

fn set_max_batch_size<T: Config>(batch_size: u32) {
    <MaxBatchSize<T>>::set(batch_size);
}

fn get_proof<T: Config>(
    relayer: &T::AccountId,
    signer: &T::AccountId,
    signature: sp_core::sr25519::Signature,
) -> Proof<T::Signature, T::AccountId> {
    return Proof { signer: signer.clone(), relayer: relayer.clone(), signature: signature.into() }
}

fn enable_rewards<T: Config>() {
    <RewardEnabled<T>>::set(true);
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

    set_admin_config_reward_batch_size {
        let current_batch_size = <MaxBatchSize<T>>::get();
        let new_batch_size = current_batch_size + 1u32;
        let config = AdminConfig::BatchSize(new_batch_size);

    }: set_admin_config(RawOrigin::Root, config.clone())
    verify {
        assert!(<MaxBatchSize<T>>::get() == new_batch_size);
    }

    set_admin_config_reward_heartbeat {
        let current_heartbeat = <HeartbeatPeriod<T>>::get();
        let new_heartbeat = current_heartbeat + 1u32;
        let config = AdminConfig::Heartbeat(new_heartbeat);

    }: set_admin_config(RawOrigin::Root, config.clone())
    verify {
        assert!(<HeartbeatPeriod<T>>::get() == new_heartbeat);
    }

    set_admin_config_reward_amount {
        let current_amount = <RewardAmount<T>>::get();
        let new_amount = current_amount + 1u32.into();
        let config = AdminConfig::RewardAmount(new_amount);

    }: set_admin_config(RawOrigin::Root, config.clone())
    verify {
        assert!(<RewardAmount<T>>::get() == new_amount);
    }

    set_admin_config_reward_enabled {
        let current_flag = <RewardEnabled<T>>::get();
        let new_flag = !current_flag;
        let config = AdminConfig::RewardToggle(new_flag);

    }: set_admin_config(RawOrigin::Root, config.clone())
    verify {
        assert!(<RewardEnabled<T>>::get() == new_flag);
    }

    on_initialise_with_new_reward_period {
        let reward_period = <RewardPeriod<T>>::get();
        let block_number: BlockNumberFor<T> = (reward_period.first + BlockNumberFor::<T>::from(reward_period.length) + 1u32.into()).into();
        enable_rewards::<T>();
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
        enable_rewards::<T>();
    }: { Pallet::<T>::on_initialize(block_number) }
    verify {
        assert!(reward_period.current== <RewardPeriod<T>>::get().current);
    }

    offchain_submit_heartbeat {
        enable_rewards::<T>();

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

    offchain_pay_nodes {
        let registered_nodes = 1001;

        // This should affect the performance of the extrinsic.
        let b in 1 .. 1000;

        enable_rewards::<T>();
        fund_reward_pot::<T>();
        set_max_batch_size::<T>(b);

        let reward_period = <RewardPeriod<T>>::get();
        let reward_period_index = reward_period.current;
        let owner: T::AccountId = account("owner", 0, 0);
        let author = create_author::<T>();

        create_nodes_and_hearbeat::<T>(owner.clone(), reward_period_index, registered_nodes);

        // Move forward to the next reward period
        <frame_system::Pallet<T>>::set_block_number((reward_period.length + 1).into());
        let current_block_number = frame_system::Pallet::<T>::block_number();
        <frame_system::Pallet<T>>::set_block_number(current_block_number + reward_period.length.into());
        Pallet::<T>::on_initialize(current_block_number);
        let signature = author.key.sign(
            &(PAYOUT_REWARD_CONTEXT, reward_period_index).encode()
        ).expect("Error signing");
    }: offchain_pay_nodes(RawOrigin::None, reward_period_index, author ,signature)
    verify {
        let max_batch_size = MaxBatchSize::<T>::get();
        let expected_balance = max_batch_size.min(registered_nodes).saturated_into::<BalanceOf<T>>().
            bmul_bdiv(RewardAmount::<T>::get(), registered_nodes.saturated_into::<BalanceOf<T>>())
            .unwrap();
        assert_approx!(T::Currency::free_balance(&owner.clone()), expected_balance, 1_000u32.saturated_into::<BalanceOf<T>>());
    }

    #[extra]
    pay_nodes_constant_batch_size {
        /* Prove that the read/write is constant time with respect to the batch size.
           Even if the number of registered nodes (n) increases. You should see something like:

             Median Slopes Analysis
             ========
             -- Extrinsic Time --

             Model:
             Time ~=    514.2
                + n    0.554 µs

             Reads = 30 + (0 * n)
             Writes = 13 + (0 * n)
             Recorded proof Size = 2601 + (12 * n)

        */

        // This should NOT affect the performance of the extrinsic. The execution time should be constant.
        let n in 1 .. 100;

        enable_rewards::<T>();
        fund_reward_pot::<T>();

        let reward_period = <RewardPeriod<T>>::get();
        let reward_period_index = reward_period.current;
        let owner: T::AccountId = account("owner", 0, 0);
        let author = create_author::<T>();

        create_nodes_and_hearbeat::<T>(owner.clone(), reward_period_index, n);

        // Move forward to the next reward period
        <frame_system::Pallet<T>>::set_block_number((reward_period.length + 1).into());
        let current_block_number = frame_system::Pallet::<T>::block_number();
        <frame_system::Pallet<T>>::set_block_number(current_block_number + reward_period.length.into());
        Pallet::<T>::on_initialize(current_block_number);
        let signature = author.key.sign(
            &(PAYOUT_REWARD_CONTEXT, reward_period_index).encode()
        ).expect("Error signing");
    }: offchain_pay_nodes(RawOrigin::None, reward_period_index, author ,signature)
    verify {
        let max_batch_size = MaxBatchSize::<T>::get();
        let expected_balance = max_batch_size.min(n).saturated_into::<BalanceOf<T>>().
            bmul_bdiv(RewardAmount::<T>::get(), n.saturated_into::<BalanceOf<T>>())
            .unwrap();
        assert_approx!(T::Currency::free_balance(&owner.clone()), expected_balance, 1_000u32.saturated_into::<BalanceOf<T>>());
    }

    signed_register_node {
        let registrar_key = crate::sr25519::app_sr25519::Public::generate_pair(None);
        let registrar: T::AccountId =
            T::AccountId::decode(&mut Encode::encode(&registrar_key).as_slice()).expect("valid account id");
        set_registrar::<T>(registrar.clone());

        let relayer: T::AccountId = account("relayer", 11, 11);
        let owner: T::AccountId = account("owner", 1, 1);
        let node: NodeId<T> = account("node", 2, 2);
        let signing_key: T::SignerId = account("signing_key", 3, 3);
        let now = frame_system::Pallet::<T>::block_number();

        let signed_payload = encode_signed_register_node_params::<T>(
            &relayer.clone(),
            &node,
            &owner,
            &signing_key,
            &now.clone(),
        );

        let signature = registrar_key.sign(&signed_payload).ok_or("Error signing proof")?;
        let proof = get_proof::<T>(&relayer.clone(), &registrar, signature.into());
    }: signed_register_node(RawOrigin::Signed(registrar.clone()), proof.clone(), node.clone(), owner.clone(), signing_key.clone(), now)
    verify {
        assert!(<OwnedNodes<T>>::contains_key(owner.clone(), node.clone()));
        assert!(<NodeRegistry<T>>::contains_key(node.clone()));
        assert_last_event::<T>(Event::NodeRegistered{owner, node}.into());
    }

    transfer_ownership {
        let owner: T::AccountId = account("owner", 1, 1);
        let node_id: NodeId<T> = account("node", 2, 2);
        let new_owner: T::AccountId = account("new_owner", 3, 3);
        let _: T::SignerId = register_new_node::<T>(node_id.clone(), owner.clone());
    } : transfer_ownership(RawOrigin::Signed(owner.clone()), node_id.clone(), new_owner.clone())
    verify {
        assert!(<OwnedNodes<T>>::contains_key(new_owner.clone(), node_id.clone()));
        assert!(!<OwnedNodes<T>>::contains_key(owner.clone(), node_id.clone()));
        assert_last_event::<T>(Event::NodeOwnershipTransferred{node_id, old_owner: owner, new_owner}.into());
    }

    signed_transfer_ownership {
        let relayer: T::AccountId = account("relayer", 11, 11);
        let now = frame_system::Pallet::<T>::block_number();

        let owner_key = crate::sr25519::app_sr25519::Public::generate_pair(None);
        let owner: T::AccountId =
            T::AccountId::decode(&mut Encode::encode(&owner_key).as_slice()).expect("valid account id");

        let node_id: NodeId<T> = account("node", 2, 2);
        let new_owner: T::AccountId = account("new_owner", 3, 3);
        let _: T::SignerId = register_new_node::<T>(node_id.clone(), owner.clone());

        let signed_payload = encode_signed_transfer_ownership_params::<T>(
            &relayer.clone(),
            &node_id,
            &new_owner,
            &now.clone(),
        );

        let signature = owner_key.sign(&signed_payload).ok_or("Error signing proof")?;
        let proof = get_proof::<T>(&relayer.clone(), &owner, signature.into());
    }: signed_transfer_ownership(RawOrigin::Signed(owner.clone()), proof.clone(), node_id.clone(), new_owner.clone(), now)
    verify {
        assert_eq!(false, <OwnedNodes<T>>::contains_key(owner.clone(), node_id.clone()));
        assert_eq!(true, <OwnedNodes<T>>::contains_key(new_owner.clone(), node_id.clone()));
        assert!(<NodeRegistry<T>>::contains_key(node_id.clone()));
        assert_last_event::<T>(Event::NodeOwnershipTransferred{old_owner: owner, new_owner, node_id}.into());
    }
}

impl_benchmark_test_suite!(
    Pallet,
    crate::mock::ExtBuilder::build_default().with_genesis_config().as_externality(),
    crate::mock::TestRuntime,
);
