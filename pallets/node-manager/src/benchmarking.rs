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
}
