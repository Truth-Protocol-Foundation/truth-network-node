//Copyright 2025 Truth Network.

#![cfg(test)]

use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;

#[derive(Clone)]
struct Context {
    registrar: AccountId,
    owner: AccountId,
    node_id: AccountId,
    signing_key: <mock::TestRuntime as pallet::Config>::SignerId,
}

impl Default for Context {
    fn default() -> Self {
        let registrar = TestAccount::new([1u8; 32]).account_id();
        let node_id = TestAccount::new([202u8; 32]).account_id();
        let signing_key_id = 987;

        setup_registrar(&registrar);
        set_ocw_node_id(node_id.clone());
        UintAuthorityId::set_all_keys(vec![UintAuthorityId(signing_key_id)]);

        Context {
            node_id,
            registrar,
            owner: TestAccount::new([101u8; 32]).account_id(),
            signing_key: UintAuthorityId(signing_key_id),
        }
    }
}

fn set_ocw_node_id(node_id: AccountId) {
    let storage = StorageValueRef::persistent(REGISTERED_NODE_KEY);
    storage
        .mutate(|r: Result<Option<AccountId>, StorageRetrievalError>| match r {
            Ok(Some(_)) => Ok(node_id),
            Ok(None) => Ok(node_id),
            _ => Err(()),
        })
        .unwrap();
}

fn remove_ocw_node_id() {
    let mut storage = StorageValueRef::persistent(REGISTERED_NODE_KEY);
    storage.clear();
}

fn setup_registrar(registrar: &AccountId) {
    <NodeRegistrar<TestRuntime>>::set(Some(registrar.clone()));
}

fn register_node(context: &Context) {
    assert_ok!(NodeManager::register_node(
        RuntimeOrigin::signed(context.registrar.clone()),
        context.node_id.clone(),
        context.owner.clone(),
        context.signing_key.clone(),
    ));
}

fn pop_tx_from_mempool(pool_state: Arc<RwLock<PoolState>>) -> Extrinsic {
    let tx = pool_state.write().transactions.pop().unwrap();
    Extrinsic::decode(&mut &*tx).unwrap()
}

mod given_a_reward_period {
    use super::*;

    #[test]
    fn heartbeat_submission_succeeds() {
        let (mut ext, pool_state, _offchain_state) = ExtBuilder::build_default()
            .with_genesis_config()
            .for_offchain_worker()
            .as_externality_with_state();
        ext.execute_with(|| {
            let context = Context::default();
            register_node(&context);

            NodeManager::offchain_worker(System::block_number());

            let tx = pop_tx_from_mempool(pool_state);
            assert_ok!(tx.call.clone().dispatch(frame_system::RawOrigin::None.into()));

            // Check if the transaction from the mempool is what we expected
            assert!(matches!(
                tx.call,
                RuntimeCall::NodeManager(crate::Call::offchain_submit_heartbeat {
                    node: _,
                    reward_period_index: _,
                    heartbeat_count: _,
                    signature: _,
                })
            ));

            // Ensure the tx has executed successfully
            let reward_period = <RewardPeriod<TestRuntime>>::get().current;
            let uptime_info =
                <NodeUptime<TestRuntime>>::get(reward_period, &context.node_id).unwrap();

            assert_eq!(uptime_info.count, 1);
            assert_eq!(uptime_info.last_reported, System::block_number());
            assert_eq!(<TotalUptime<TestRuntime>>::get(&reward_period), 1);
            System::assert_last_event(
                Event::HeartbeatReceived {
                    reward_period_index: reward_period,
                    node: context.node_id,
                }
                .into(),
            );
        });
    }

    #[test]
    fn heartbeat_submission_succeeds_without_node_id() {
        let (mut ext, pool_state, _offchain_state) = ExtBuilder::build_default()
            .with_genesis_config()
            .for_offchain_worker()
            .as_externality_with_state();
        ext.execute_with(|| {
            let context = Context::default();
            register_node(&context);

            // Remove the node id from the ocw database
            remove_ocw_node_id();

            NodeManager::offchain_worker(System::block_number());
            let tx = pop_tx_from_mempool(pool_state);

            // Check if the transaction from the mempool is what we expected
            assert!(matches!(
                tx.call,
                RuntimeCall::NodeManager(crate::Call::offchain_submit_heartbeat {
                    node: _,
                    reward_period_index: _,
                    heartbeat_count: _,
                    signature: _,
                })
            ));
        });
    }

    #[test]
    fn heartbeat_lock_prevents_duplicate_submissions() {
        let (mut ext, pool_state, _offchain_state) = ExtBuilder::build_default()
            .with_genesis_config()
            .for_offchain_worker()
            .as_externality_with_state();
        ext.execute_with(|| {
            let context = Context::default();
            register_node(&context);

            NodeManager::offchain_worker(System::block_number());
            assert_eq!(false, pool_state.read().transactions.is_empty());

            // Remove the tx from mempool
            let _ = pop_tx_from_mempool(pool_state.clone());

            // Call OCW again, but this time it should not submit a new transaction
            NodeManager::offchain_worker(System::block_number());
            assert_eq!(true, pool_state.read().transactions.is_empty());
        });
    }

    #[test]
    fn heartbeat_lock_released_automatically() {
        let (mut ext, pool_state, _offchain_state) = ExtBuilder::build_default()
            .with_genesis_config()
            .for_offchain_worker()
            .as_externality_with_state();
        ext.execute_with(|| {
            let context = Context::default();
            register_node(&context);

            NodeManager::offchain_worker(System::block_number());
            assert_eq!(false, pool_state.read().transactions.is_empty());

            // Remove the tx from mempool
            let _ = pop_tx_from_mempool(pool_state.clone());

            // Move forward to release ocw lock
            System::set_block_number(System::block_number() + 6u64);

            // Call OCW again, but this time it should not submit a new transaction
            NodeManager::offchain_worker(System::block_number());
            // Mempool is not empty
            assert_eq!(false, pool_state.read().transactions.is_empty());
        });
    }

    #[test]
    fn mutiple_heartbeat_submission_succeeds() {
        let (mut ext, pool_state, _offchain_state) = ExtBuilder::build_default()
            .with_genesis_config()
            .for_offchain_worker()
            .as_externality_with_state();
        ext.execute_with(|| {
            let context = Context::default();
            register_node(&context);

            NodeManager::offchain_worker(System::block_number());
            let tx = pop_tx_from_mempool(pool_state.clone());
            assert_ok!(tx.call.clone().dispatch(frame_system::RawOrigin::None.into()));

            // Ensure the tx has executed successfully
            let reward_period = <RewardPeriod<TestRuntime>>::get().current;

            // Move forward
            System::set_block_number(
                System::block_number() + <HeartbeatPeriod<TestRuntime>>::get() as u64 + 1u64,
            );

            // Call OCW and send transactions
            NodeManager::offchain_worker(System::block_number());
            let tx = pop_tx_from_mempool(pool_state);
            assert_ok!(tx.call.clone().dispatch(frame_system::RawOrigin::None.into()));

            let uptime_info =
                <NodeUptime<TestRuntime>>::get(reward_period, &context.node_id).unwrap();
            assert_eq!(uptime_info.count, 2);
            assert_eq!(uptime_info.last_reported, System::block_number());
            assert_eq!(<TotalUptime<TestRuntime>>::get(&reward_period), 2);
            System::assert_last_event(
                Event::HeartbeatReceived {
                    reward_period_index: reward_period,
                    node: context.node_id,
                }
                .into(),
            );
        });
    }

    #[test]
    fn heartbeat_period_is_respected() {
        let (mut ext, pool_state, _offchain_state) = ExtBuilder::build_default()
            .with_genesis_config()
            .for_offchain_worker()
            .as_externality_with_state();
        ext.execute_with(|| {
            let context = Context::default();
            register_node(&context);

            let reward_period_length = <RewardPeriod<TestRuntime>>::get().length as u64;

            NodeManager::offchain_worker(System::block_number());
            assert_eq!(false, pool_state.read().transactions.is_empty());
            let _ = pop_tx_from_mempool(pool_state.clone());

            roll_forward(1u64);
            NodeManager::offchain_worker(System::block_number());
            // No transaction because we are still in the same heartbeat period
            assert_eq!(true, pool_state.read().transactions.is_empty());

            roll_forward(1u64);
            NodeManager::offchain_worker(System::block_number());
            // No transaction because we are still in the same heartbeat period
            assert_eq!(true, pool_state.read().transactions.is_empty());

            roll_forward(1u64);
            NodeManager::offchain_worker(System::block_number());
            // No transaction because we are still in the same heartbeat period
            assert_eq!(true, pool_state.read().transactions.is_empty());

            roll_forward((reward_period_length - System::block_number()) + 1);
            NodeManager::offchain_worker(System::block_number());
            assert_eq!(false, pool_state.read().transactions.is_empty());
        });
    }
}

mod across_multiple_reward_periods {
    use super::*;

    fn submit_multiple_heartbeats(n: u64, pool_state: Arc<RwLock<PoolState>>) {
        for _ in 0..n {
            NodeManager::offchain_worker(System::block_number());
            let tx = pop_tx_from_mempool(pool_state.clone());
            assert_ok!(tx.call.clone().dispatch(frame_system::RawOrigin::None.into()));

            // Move forward
            System::set_block_number(
                System::block_number() + <HeartbeatPeriod<TestRuntime>>::get() as u64 + 1u64,
            );
        }
    }

    #[test]
    fn mutiple_heartbeat_submissions_succeed() {
        let (mut ext, pool_state, _offchain_state) = ExtBuilder::build_default()
            .with_genesis_config()
            .for_offchain_worker()
            .as_externality_with_state();
        ext.execute_with(|| {
            let context = Context::default();
            register_node(&context);

            let reward_period = <RewardPeriod<TestRuntime>>::get();
            let old_reward_period = reward_period.current as u64;
            let reward_period_length = reward_period.length as u64;

            let old_heartbeat_count = 4u64;
            submit_multiple_heartbeats(old_heartbeat_count, pool_state.clone());

            if System::block_number() < reward_period_length {
                roll_forward((reward_period_length - System::block_number()) + 1);
            } else {
                roll_forward(1u64);
            }
            assert_eq!(<RewardPeriod<TestRuntime>>::get().current, old_reward_period + 1);

            let new_heartbeat_count = old_heartbeat_count + 1;
            submit_multiple_heartbeats(new_heartbeat_count, pool_state.clone());

            // Ensure the tx has executed successfully
            let new_reward_period = <RewardPeriod<TestRuntime>>::get().current;

            let uptime_info =
                <NodeUptime<TestRuntime>>::get(old_reward_period, &context.node_id).unwrap();
            assert_eq!(uptime_info.count, old_heartbeat_count);
            assert_eq!(<TotalUptime<TestRuntime>>::get(&old_reward_period), old_heartbeat_count);

            let uptime_info =
                <NodeUptime<TestRuntime>>::get(new_reward_period, &context.node_id).unwrap();
            assert_eq!(uptime_info.count, new_heartbeat_count);
            assert_eq!(<TotalUptime<TestRuntime>>::get(&new_reward_period), new_heartbeat_count);

            System::assert_last_event(
                Event::HeartbeatReceived {
                    reward_period_index: new_reward_period,
                    node: context.node_id,
                }
                .into(),
            );
        });
    }
}

mod fails_when {
    use super::*;

    #[test]
    fn duplicate_heartbeats_submitted() {
        let (mut ext, pool_state, _offchain_state) = ExtBuilder::build_default()
            .with_genesis_config()
            .for_offchain_worker()
            .as_externality_with_state();
        ext.execute_with(|| {
            let context = Context::default();
            register_node(&context);

            let reward_period = <RewardPeriod<TestRuntime>>::get().current;
            NodeManager::offchain_worker(System::block_number());
            let tx = pop_tx_from_mempool(pool_state.clone());
            assert_ok!(tx.call.clone().dispatch(frame_system::RawOrigin::None.into()));

            let signature =
                context.signing_key.sign(&("DummyProof").encode()).expect("Error signing");
            assert_noop!(
                NodeManager::offchain_submit_heartbeat(
                    RawOrigin::None.into(),
                    context.node_id,
                    reward_period,
                    1u64,
                    signature
                ),
                Error::<TestRuntime>::DuplicateHeartbeat
            );
        });
    }

    #[test]
    fn wrong_period_used() {
        let (mut ext, _pool_state, _offchain_state) = ExtBuilder::build_default()
            .with_genesis_config()
            .for_offchain_worker()
            .as_externality_with_state();
        ext.execute_with(|| {
            let context = Context::default();
            register_node(&context);
            let reward_period = <RewardPeriod<TestRuntime>>::get().current;
            let signature =
                context.signing_key.sign(&("DummyProof").encode()).expect("Error signing");

            let bad_reward_period = reward_period + 1;
            assert_noop!(
                NodeManager::offchain_submit_heartbeat(
                    RawOrigin::None.into(),
                    context.node_id,
                    bad_reward_period,
                    1u64,
                    signature
                ),
                Error::<TestRuntime>::InvalidHeartbeat
            );
        });
    }

    #[test]
    fn submitter_is_not_registered() {
        let (mut ext, _pool_state, _offchain_state) = ExtBuilder::build_default()
            .with_genesis_config()
            .for_offchain_worker()
            .as_externality_with_state();
        ext.execute_with(|| {
            let context = Context::default();
            register_node(&context);
            let reward_period = <RewardPeriod<TestRuntime>>::get().current;
            let signature =
                context.signing_key.sign(&("DummyProof").encode()).expect("Error signing");

            let bad_node = TestAccount::new([31u8; 32]).account_id();
            assert_noop!(
                NodeManager::offchain_submit_heartbeat(
                    RawOrigin::None.into(),
                    bad_node,
                    reward_period,
                    1u64,
                    signature
                ),
                Error::<TestRuntime>::NodeNotRegistered
            );
        });
    }

    #[test]
    fn wrong_uptime_count_is_used() {
        let (mut ext, _pool_state, _offchain_state) = ExtBuilder::build_default()
            .with_genesis_config()
            .for_offchain_worker()
            .as_externality_with_state();
        ext.execute_with(|| {
            let context = Context::default();
            register_node(&context);
            let reward_period = <RewardPeriod<TestRuntime>>::get().current;
            let signature =
                context.signing_key.sign(&("DummyProof").encode()).expect("Error signing");

            let bad_uptime_count = 99u64;
            assert_noop!(
                NodeManager::offchain_submit_heartbeat(
                    RawOrigin::None.into(),
                    context.node_id,
                    reward_period,
                    bad_uptime_count,
                    signature
                ),
                Error::<TestRuntime>::InvalidHeartbeat
            );
        });
    }

    #[test]
    fn keystore_not_populated() {
        let (mut ext, pool_state, _offchain_state) = ExtBuilder::build_default()
            .with_genesis_config()
            .for_offchain_worker()
            .as_externality_with_state();
        ext.execute_with(|| {
            let mut context = Context::default();
            let key_id = 987;
            context.signing_key = UintAuthorityId(key_id);
            register_node(&context);

            // Clear keystore keys
            let keys: Vec<UintAuthorityId> = vec![];
            UintAuthorityId::set_all_keys(keys);

            NodeManager::offchain_worker(System::block_number());
            assert_eq!(true, pool_state.read().transactions.is_empty());

            // Prove that it can work if the keystore was populated
            UintAuthorityId::set_all_keys(vec![UintAuthorityId(key_id)]);
            NodeManager::offchain_worker(System::block_number());
            assert_eq!(false, pool_state.read().transactions.is_empty());
        });
    }

    #[test]
    fn rewards_are_disabled() {
        let (mut ext, _ool_state, _offchain_state) = ExtBuilder::build_default()
            .with_genesis_config()
            .for_offchain_worker()
            .as_externality_with_state();
        ext.execute_with(|| {
            let mut context = Context::default();
            //Disable rewards
            RewardEnabled::<TestRuntime>::put(false);

            let key_id = 987;
            context.signing_key = UintAuthorityId(key_id);
            register_node(&context);

            let call = crate::Call::offchain_submit_heartbeat {
                node: context.node_id,
                reward_period_index: 1u64,
                heartbeat_count: 1u64,
                signature: context
                    .signing_key
                    .sign(&("DummyProof").encode())
                    .expect("Error signing"),
            };

            assert_noop!(
                <NodeManager as ValidateUnsigned>::validate_unsigned(
                    TransactionSource::Local,
                    &call
                ),
                InvalidTransaction::Custom(ERROR_CODE_REWARD_DISABLED)
            );
        });
    }

    #[test]
    fn unsigned_calls_are_not_local() {
        let (mut ext, _pool_state, _offchain_state) = ExtBuilder::build_default()
            .with_genesis_config()
            .for_offchain_worker()
            .as_externality_with_state();
        ext.execute_with(|| {
            let mut context = Context::default();
            let key_id = 987;
            context.signing_key = UintAuthorityId(key_id);
            register_node(&context);

            let call = crate::Call::offchain_submit_heartbeat {
                node: context.node_id,
                reward_period_index: 1u64,
                heartbeat_count: 1u64,
                signature: context
                    .signing_key
                    .sign(&("DummyProof").encode())
                    .expect("Error signing"),
            };

            assert_noop!(
                <NodeManager as ValidateUnsigned>::validate_unsigned(
                    TransactionSource::External,
                    &call
                ),
                InvalidTransaction::Call
            );
        });
    }
}
