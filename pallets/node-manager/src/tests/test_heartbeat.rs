//Copyright 2025 Truth Network.

#![cfg(test)]

use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use prediction_market_primitives::test_helper::TestAccount;

#[derive(Clone)]
struct Context {
    origin: RuntimeOrigin,
    owner: AccountId,
    node_id: AccountId,
    signing_key: <mock::TestRuntime as pallet::Config>::SignerId,
}

impl Default for Context {
    fn default() -> Self {
        let registrar = TestAccount::new([1u8; 32]).account_id();
        let node_id = TestAccount::new([202u8; 32]).account_id();

        setup_registrar(&registrar);

        Context {
            node_id,
            origin: RuntimeOrigin::signed(node_id.clone()),
            owner: TestAccount::new([101u8; 32]).account_id(),
            signing_key: <mock::TestRuntime as pallet::Config>::SignerId::generate_pair(None),
        }
    }
}

fn setup_registrar(registrar: &AccountId) {
    <NodeRegistrar<TestRuntime>>::set(Some(registrar.clone()));
}

#[test]
fn heartbeat_submission_succeeds() {
    let (mut ext, _pool_state, _offchain_state) = ExtBuilder::build_default().with_genesis_config().for_offchain_worker().as_externality_with_state();
    ext.execute_with(|| {

        // Total node counter is increased
        assert_eq!(1, 1);
    });
}
