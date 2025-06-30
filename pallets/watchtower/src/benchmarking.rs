#![cfg(feature = "runtime-benchmarks")]

//! Watchtower Pallet Benchmarks
//!
//! These benchmarks require the runtime to be configured with appropriate mock implementations.
//! The benchmarking runtime should use the same mock implementations as defined in mock.rs:
//! - MockNodeManager for T::NodeManager
//! - MockVoteStatusNotifier for T::VoteStatusNotifier
//!
//! Additionally, the benchmarking runtime should ensure that whitelisted accounts
//! are properly configured as authorized watchtowers in the NodeManager mock.

use super::*;
use crate::Pallet as Watchtower;
use frame_benchmarking::{
    benchmarks, impl_benchmark_test_suite, whitelist_account, whitelisted_caller,
};
use frame_system::{Pallet as System, RawOrigin};

// Helper function to create a RootId for testing
fn create_test_root_id<T: Config>(index: u32) -> WatchtowerRootId<BlockNumberFor<T>> {
    use sp_avn_common::{RootId, RootRange};
    RootId {
        range: RootRange {
            from_block: (100u32 * index).into(),
            to_block: (100u32 * index + 50u32).into(),
        },
        ingress_counter: (1u64 + index as u64),
    }
}

// Helper to assert events were emitted
fn assert_events_emitted<T: Config>() {
    let events = System::<T>::events();
    assert!(!events.is_empty(), "No events recorded");
}

benchmarks! {
    where_clause {
        where T: pallet_avn::Config,
    }

    set_voting_period {
        let new_period: BlockNumberFor<T> = 200u32.into();
    }: _(RawOrigin::Root, new_period)
    verify {
        assert_eq!(Watchtower::<T>::get_voting_period(), new_period);
        assert_events_emitted::<T>();
    }

    submit_watchtower_vote {
        // Setup: Get a valid watchtower account
        let voter: T::AccountId = whitelisted_caller();
        whitelist_account!(voter);

        let summary_instance = SummarySourceInstance::EthereumBridge;
        let root_id = create_test_root_id::<T>(0);
        let vote_is_valid = true;

        // Note: This benchmark assumes the runtime configures the NodeManager mock
        // to recognize the whitelisted_caller as an authorized watchtower

    }: _(RawOrigin::Signed(voter.clone()), summary_instance, root_id.clone(), vote_is_valid)
    verify {
        // Verify the vote was recorded in counters
        let (yes_votes, no_votes) = VoteCounters::<T>::get(summary_instance, root_id.clone());
        assert!(yes_votes > 0 || no_votes > 0, "Vote should be recorded in counters");

        // Verify voter history was recorded
        let consensus_key = (summary_instance, root_id.clone());
        assert!(VoterHistory::<T>::contains_key(&consensus_key, &voter),
               "Voter history should be recorded");

        // Verify voting period was initialized
        assert!(VotingStartBlock::<T>::contains_key((summary_instance, root_id.clone())),
               "Voting period should be initialized");

        // Check that events were emitted
        assert_events_emitted::<T>();
    }

    offchain_submit_watchtower_vote {
        // Setup: Get a valid watchtower account and signing key
        let node: T::AccountId = whitelisted_caller();
        whitelist_account!(node);

        let summary_instance = SummarySourceInstance::EthereumBridge;
        let root_id = create_test_root_id::<T>(1);
        let vote_is_valid = true;

        // Get the signing key for the node (mock should provide this)
        let signing_key = T::SignerId::generate_pair(None);

        // Create a dummy signature (in real usage this would be properly signed)
        let data = (
            crate::WATCHTOWER_OCW_CONTEXT,
            &summary_instance,
            &root_id,
            vote_is_valid,
        );
        let signature = signing_key.sign(&data.encode()).unwrap();

    }: _(RawOrigin::None, node.clone(), signing_key, summary_instance, root_id.clone(), vote_is_valid, signature)
    verify {
        // Verify the vote was recorded in counters
        let (yes_votes, no_votes) = VoteCounters::<T>::get(summary_instance, root_id.clone());
        assert!(yes_votes > 0 || no_votes > 0, "Vote should be recorded in counters");

        // Verify voter history was recorded
        let consensus_key = (summary_instance, root_id.clone());
        assert!(VoterHistory::<T>::contains_key(&consensus_key, &node),
               "Voter history should be recorded");

        // Check that events were emitted
        assert_events_emitted::<T>();
    }
}

impl_benchmark_test_suite!(
    Pallet,
    crate::mock::ExtBuilder::build_default().for_benchmarks().as_externality(),
    crate::mock::TestRuntime,
);
