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
    account, benchmarks, impl_benchmark_test_suite, whitelist_account, whitelisted_caller,
};
use frame_system::{Pallet as System, RawOrigin};
use sp_core::H256;
use sp_runtime::RuntimeAppPublic;

// Helper function to create a RootId for testing
fn create_test_root_id<T: Config>(index: u32) -> RootId<BlockNumberFor<T>> {
    use sp_avn_common::{RootId, RootRange};
    RootId {
        range: RootRange {
            from_block: (100u32 * index).into(),
            to_block: (100u32 * index + 50u32).into(),
        },
        ingress_counter: (1u64 + index as u64),
    }
}

fn create_test_challenge_data() -> (WatchtowerOnChainHash, WatchtowerOnChainHash) {
    let incorrect_root = H256::from([1u8; 32]);
    let correct_root = H256::from([2u8; 32]);
    (incorrect_root, correct_root)
}

fn create_fake_signature<T: Config>() -> <T::SignerId as RuntimeAppPublic>::Signature {
    let dummy_data = b"benchmark_signature";
    let dummy_key = T::SignerId::generate_pair(None);
    dummy_key.sign(&dummy_data.encode()).expect("Signature generation should work")
}

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

    set_challenge_resolution_admin {
        let new_admin: T::AccountId = whitelisted_caller();
        whitelist_account!(new_admin);
    }: _(RawOrigin::Root, Some(new_admin.clone()))
    verify {
        assert_eq!(ChallengeResolutionAdmin::<T>::get(), Some(new_admin));
        assert_events_emitted::<T>();
    }

    vote {
        let voter: T::AccountId = whitelisted_caller();
        whitelist_account!(voter);

        let summary_instance = SummarySource::EthereumBridge;
        let root_id = create_test_root_id::<T>(0);
        let vote_is_valid = true;
        let signature = create_fake_signature::<T>();

        let consensus_key = (summary_instance, root_id.clone());
        VotingStartBlock::<T>::insert(&consensus_key, System::<T>::block_number());

    }: _(RawOrigin::None, voter.clone(), summary_instance, root_id.clone(), vote_is_valid, signature)
    verify {
        // Verify the vote was recorded in counters
        let (yes_votes, no_votes) = VoteCounters::<T>::get(summary_instance, root_id.clone());
        assert!(yes_votes > 0 || no_votes > 0, "Vote should be recorded in counters");

        // Verify voter history was recorded
        let consensus_key = (summary_instance, root_id.clone());
        assert!(VoterHistory::<T>::contains_key(&consensus_key, &voter),
               "Voter history should be recorded");

        assert!(VotingStartBlock::<T>::contains_key((summary_instance, root_id.clone())),
               "Voting period should exist");

        assert_events_emitted::<T>();
    }

    submit_challenge {
        let challenger: T::AccountId = whitelisted_caller();
        whitelist_account!(challenger);

        let summary_instance = SummarySourceInstance::EthereumBridge;
        let root_id = create_test_root_id::<T>(1);
        let (incorrect_root_id, correct_root_hash) = create_test_challenge_data();
        let signature = create_fake_signature::<T>();

    }: _(RawOrigin::None, challenger.clone(), summary_instance, root_id.clone(), incorrect_root_id, correct_root_hash, signature)
    verify {
        let challenge_key = (summary_instance, root_id.clone());
        let challenge_info = Challenges::<T>::get(&challenge_key);
        assert!(challenge_info.is_some(), "Challenge should be stored");

        let challenge = challenge_info.unwrap();
        assert_eq!(challenge.challengers.len(), 1, "Should have one challenger");
        assert_eq!(challenge.challengers[0], challenger, "Challenger should match");
        assert_eq!(challenge.status, ChallengeStatus::Pending, "Challenge should be pending");

        assert_events_emitted::<T>();
    }

    resolve_challenge {
        let challenger: T::AccountId = whitelisted_caller();
        let summary_instance = SummarySourceInstance::EthereumBridge;
        let root_id = create_test_root_id::<T>(2);
        let (incorrect_root_id, correct_root_hash) = create_test_challenge_data();

        let challenge_info = ChallengeInfo {
            incorrect_root_id,
            correct_root_hash,
            challengers: vec![challenger.clone()].try_into().expect("Should fit in bounds"),
            status: ChallengeStatus::Pending,
            created_block: 1u32,
            first_challenge_alert_sent: false,
            original_consensus: Some(WatchtowerSummaryStatus::Accepted),
        };
        let challenge_key = (summary_instance, root_id.clone());
        Challenges::<T>::insert(&challenge_key, challenge_info);

        let resolution = ChallengeResolution::BadChallenge;

    }: _(RawOrigin::Root, summary_instance, root_id.clone(), resolution)
    verify {
        assert!(Challenges::<T>::get(&challenge_key).is_none(), "Challenge should be removed");

        assert_eq!(FailedChallengeCount::<T>::get(&challenger), 1, "Failed challenge count should be incremented");

        assert_events_emitted::<T>();
    }

    submit_multiple_votes_for_consensus {
        let v in 1 .. T::MaxWatchtowers::get();

        let summary_instance = SummarySourceInstance::EthereumBridge;
        let root_id = create_test_root_id::<T>(3);
        let signature = create_fake_signature::<T>();

        let consensus_key = (summary_instance, root_id.clone());
        VotingStartBlock::<T>::insert(&consensus_key, System::<T>::block_number());

        for i in 1..v {
            let voter: T::AccountId = account("voter", i, 0);
            let _ = IndividualWatchtowerVotes::<T>::try_mutate(
                summary_instance,
                root_id.clone(),
                |votes| -> Result<(), &'static str> {
                    votes.try_push((voter, true)).map_err(|_| "Too many votes")?;
                    Ok(())
                }
            );
        }

        let final_voter: T::AccountId = whitelisted_caller();
        whitelist_account!(final_voter);

    }: submit_watchtower_vote(RawOrigin::None, final_voter.clone(), summary_instance, root_id.clone(), true, signature)
    verify {
        let votes = IndividualWatchtowerVotes::<T>::get(summary_instance, root_id.clone());
        assert_eq!(votes.len() as u32, v, "Should have v votes");
        assert_events_emitted::<T>();
    }

    submit_multiple_challenges {
        let c in 1 .. T::MaxWatchtowers::get();

        let summary_instance = SummarySourceInstance::EthereumBridge;
        let root_id = create_test_root_id::<T>(4);
        let (incorrect_root_id, correct_root_hash) = create_test_challenge_data();
        let signature = create_fake_signature::<T>();

        let mut challengers = Vec::new();
        for i in 1..c {
            challengers.push(account("challenger", i, 0));
        }

        let challenge_info = ChallengeInfo {
            incorrect_root_id,
            correct_root_hash,
            challengers: challengers.try_into().expect("Should fit in bounds"),
            status: ChallengeStatus::Pending,
            created_block: 1u32,
            first_challenge_alert_sent: false,
            original_consensus: None,
        };
        let challenge_key = (summary_instance, root_id.clone());
        Challenges::<T>::insert(&challenge_key, challenge_info);

        let final_challenger: T::AccountId = whitelisted_caller();
        whitelist_account!(final_challenger);

    }: submit_challenge(RawOrigin::None, final_challenger.clone(), summary_instance, root_id.clone(), incorrect_root_id, correct_root_hash, signature)
    verify {
        let challenge = Challenges::<T>::get(&challenge_key).expect("Challenge should exist");
        assert_eq!(challenge.challengers.len() as u32, c, "Should have c challengers");
        assert_events_emitted::<T>();
    }

    ocw_vote {
        // Setup: Get a valid watchtower account and signing key
        let node: T::AccountId = whitelisted_caller();
        whitelist_account!(node);

        let summary_instance = SummarySource::EthereumBridge;
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
