#![cfg(test)]

use super::mock::*;
use crate::{
    Error, Event as WatchtowerEvent, NodeManagerInterface, SummarySource, VotingStartBlock,
};

use frame_support::{assert_noop, assert_ok};
use sp_avn_common::VotingStatus;
use sp_runtime::{testing::UintAuthorityId, RuntimeAppPublic};

#[test]
fn mock_setup_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            // Test that the mock setup is working
            assert_eq!(System::block_number(), 1);

            // Test that watchtowers are properly configured
            assert!(MockNodeManager::is_authorized_watchtower(&watchtower_account_1()));
            assert!(MockNodeManager::is_authorized_watchtower(&watchtower_account_2()));
            assert!(MockNodeManager::is_authorized_watchtower(&watchtower_account_3()));
            assert!(!MockNodeManager::is_authorized_watchtower(&unauthorized_account()));

            // Test that authorized watchtowers count works
            let watchtower_count = MockNodeManager::get_authorized_watchtowers_count();
            assert_eq!(watchtower_count, 3);

            // Test that signing keys are available
            assert!(MockNodeManager::get_node_signing_key(&watchtower_account_1()).is_some());
            assert!(MockNodeManager::get_node_signing_key(&watchtower_account_2()).is_some());
            assert!(MockNodeManager::get_node_signing_key(&watchtower_account_3()).is_some());
            assert!(MockNodeManager::get_node_signing_key(&unauthorized_account()).is_none());

            // Test that the new efficient node lookup works
            assert_eq!(MockNodeManager::get_node_from_local_signing_keys(), None);

            UintAuthorityId::set_all_keys(vec![
                UintAuthorityId(1),
                UintAuthorityId(2),
                UintAuthorityId(3),
            ]);
            let lookup_result = MockNodeManager::get_node_from_local_signing_keys();
            assert!(lookup_result.is_some(), "Should find a node when keystore is set up");

            if let Some((node, _signing_key)) = lookup_result {
                assert!(MockNodeManager::is_authorized_watchtower(&node));
            }
        });
}

#[test]
fn vote_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true
            ));

            assert_watchtower_vote_event_emitted(&watchtower_account_1(), instance, &root_id, true);

            assert!(Watchtower::is_voting_active(instance, root_id.clone()));
        });
}

#[test]
fn voting_consensus_acceptance_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;

            // Submit votes from 2 watchtowers (2/3 = majority for acceptance)
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true
            ));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
                instance,
                root_id.clone(),
                true
            ));

            assert_consensus_reached_event_emitted(instance, &root_id, VotingStatus::Accepted);

            assert!(!Watchtower::is_voting_active(instance, root_id));
        });
}

#[test]
fn voting_consensus_rejection_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;

            // Submit votes from 2 watchtowers (2/3 = majority for rejection)
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                false
            ));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
                instance,
                root_id.clone(),
                false
            ));

            assert_consensus_reached_event_emitted(instance, &root_id, VotingStatus::Rejected);
        });
}

#[test]
fn voting_period_update_works() {
    ExtBuilder::build_default().as_externality().execute_with(|| {
        let old_period = Watchtower::get_voting_period();
        let new_period = 200u64;

        // Only root can update voting period
        assert_ok!(Watchtower::set_voting_period(RuntimeOrigin::root(), new_period));

        assert_eq!(Watchtower::get_voting_period(), new_period);

        // Check that event was emitted
        let events = System::events();
        assert!(events.iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::Watchtower(WatchtowerEvent::VotingPeriodUpdated {
                    old_period: old,
                    new_period: new
                }) if old == old_period && new == new_period
            )
        }));
    });
}

#[test]
fn multiple_summary_instances_work_independently() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let ethereum_instance = SummarySource::EthereumBridge;
            let anchor_instance = SummarySource::AnchorStorage;

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                ethereum_instance,
                root_id.clone(),
                true
            ));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
                ethereum_instance,
                root_id.clone(),
                true
            ));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                anchor_instance,
                root_id.clone(),
                false
            ));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
                anchor_instance,
                root_id.clone(),
                false
            ));

            assert_consensus_reached_event_emitted(
                ethereum_instance,
                &root_id,
                VotingStatus::Accepted,
            );

            assert_consensus_reached_event_emitted(
                anchor_instance,
                &root_id,
                VotingStatus::Rejected,
            );
        });
}

#[test]
fn vote_unauthorized_fails() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;

            // Try to submit a vote from an unauthorized account
            assert_noop!(
                Watchtower::vote(
                    RuntimeOrigin::signed(unauthorized_account()),
                    instance,
                    root_id,
                    true
                ),
                Error::<TestRuntime>::NotAuthorizedWatchtower
            );
        });
}

#[test]
fn double_voting_fails() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;

            // First vote should succeed
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true
            ));

            // Second vote from same watchtower should fail
            assert_noop!(
                Watchtower::vote(
                    RuntimeOrigin::signed(watchtower_account_1()),
                    instance,
                    root_id,
                    false
                ),
                Error::<TestRuntime>::AlreadyVoted
            );
        });
}

#[test]
fn voting_after_consensus_fails() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;

            // Reach consensus with 2 votes
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true
            ));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
                instance,
                root_id.clone(),
                true
            ));

            // Verify consensus reached
            assert_consensus_reached_event_emitted(instance, &root_id, VotingStatus::Accepted);

            // Third vote should fail as consensus already reached
            assert_noop!(
                Watchtower::vote(
                    RuntimeOrigin::signed(watchtower_account_3()),
                    instance,
                    root_id,
                    false
                ),
                Error::<TestRuntime>::ConsensusAlreadyReached
            );
        });
}

#[test]
fn voting_period_expiry_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;
            let voting_period = Watchtower::get_voting_period();

            // Submit initial vote
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true
            ));

            // Advance blocks past voting period
            roll_forward(voting_period + 1);

            // Try to vote after period expiry should fail
            assert_noop!(
                Watchtower::vote(
                    RuntimeOrigin::signed(watchtower_account_2()),
                    instance,
                    root_id,
                    true
                ),
                Error::<TestRuntime>::VotingPeriodExpired
            );
        });
}

#[test]
fn split_vote_no_consensus() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;

            // Submit split votes (no consensus possible with current setup)
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true
            ));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
                instance,
                root_id.clone(),
                false
            ));

            // Third vote determines consensus
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_3()),
                instance,
                root_id.clone(),
                true
            ));

            // Should reach consensus on acceptance (2 true vs 1 false)
            assert_consensus_reached_event_emitted(instance, &root_id, VotingStatus::Accepted);
        });
}

#[test]
fn invalid_voting_period_update_fails() {
    ExtBuilder::build_default().as_externality().execute_with(|| {
        // Try to set voting period below minimum (10 blocks)
        assert_noop!(
            Watchtower::set_voting_period(RuntimeOrigin::root(), 5u64),
            Error::<TestRuntime>::InvalidVotingPeriod
        );
    });
}

#[test]
fn non_root_voting_period_update_fails() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            // Try to set voting period from non-root origin
            assert_noop!(
                Watchtower::set_voting_period(
                    RuntimeOrigin::signed(watchtower_account_1()),
                    200u64
                ),
                sp_runtime::DispatchError::BadOrigin
            );
        });
}

#[test]
fn ocw_signature_validation_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .for_offchain_worker()
        .build_and_execute_with_state(|ext, _pool_state, _offchain_state| {
            ext.execute_with(|| {
                let root_id = get_test_root_id();
                let instance = SummarySource::EthereumBridge;
                let vote_is_valid = true;

                // Get signing key for watchtower 1
                let signing_key =
                    MockNodeManager::get_node_signing_key(&watchtower_account_1()).unwrap();

                // Create test data and signature
                let data = (crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, vote_is_valid);
                let signature = signing_key.sign(&data.encode()).unwrap();

                // Validate signature
                assert!(Watchtower::offchain_signature_is_valid(&data, &signing_key, &signature));

                // Test with invalid signature
                let wrong_data =
                    (crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, !vote_is_valid);
                assert!(!Watchtower::offchain_signature_is_valid(
                    &wrong_data,
                    &signing_key,
                    &signature
                ));
            });
        });
}

#[test]
fn ocw_response_validation_works() {
    ExtBuilder::build_default().as_externality().execute_with(|| {
        // Test valid hex response
        let valid_response =
            b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_vec();
        let result = Watchtower::validate_response(valid_response);
        assert!(result.is_ok());

        // Test invalid length response
        let invalid_length_response = b"0123456789abcdef".to_vec();
        let result = Watchtower::validate_response(invalid_length_response);
        assert!(result.is_err());

        // Test invalid hex response
        let invalid_hex_response =
            b"gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg".to_vec();
        let result = Watchtower::validate_response(invalid_hex_response);
        assert!(result.is_err());

        // Test non-UTF8 response
        let non_utf8_response = vec![0xFF; 64];
        let result = Watchtower::validate_response(non_utf8_response);
        assert!(result.is_err());
    });
}

#[test]
fn voting_status_query_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;

            // Check status before voting starts
            let status = Watchtower::get_voting_status(instance, root_id.clone());
            assert!(status.is_none());

            // Submit a vote
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true
            ));

            // Check status after voting starts
            let status = Watchtower::get_voting_status(instance, root_id.clone());
            assert!(status.is_some());

            let (start_block, deadline, yes_votes, no_votes) = status.unwrap();
            assert_eq!(start_block, 1); // Started at block 1
            assert_eq!(deadline, 1 + Watchtower::get_voting_period());
            assert_eq!(yes_votes, 1);
            assert_eq!(no_votes, 0);
        });
}

#[test]
fn vote_counters_work_correctly() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;

            let (yes_votes, no_votes) = Watchtower::vote_counters(instance, root_id.clone());
            assert_eq!(yes_votes, 0);
            assert_eq!(no_votes, 0);

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true
            ));

            let (yes_votes, no_votes) = Watchtower::vote_counters(instance, root_id.clone());
            assert_eq!(yes_votes, 1);
            assert_eq!(no_votes, 0);

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
                instance,
                root_id.clone(),
                false
            ));

            let (yes_votes, no_votes) = Watchtower::vote_counters(instance, root_id.clone());
            assert_eq!(yes_votes, 1);
            assert_eq!(no_votes, 1);

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_3()),
                instance,
                root_id.clone(),
                true
            ));

            let (yes_votes, no_votes) = Watchtower::vote_counters(instance, root_id.clone());
            assert_eq!(yes_votes, 0);
            assert_eq!(no_votes, 0);
        });
}

#[test]
fn exact_consensus_threshold_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;

            // With 3 watchtowers, need 2 for consensus (⌈(2*3)/3⌉ = ⌈8/3⌉ = 3, but actually uses
            // 2/3) First vote - no consensus yet
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true
            ));

            // Verify no consensus yet
            let events = System::events();
            assert!(!events.iter().any(|record| {
                matches!(
                    record.event,
                    RuntimeEvent::Watchtower(WatchtowerEvent::WatchtowerConsensusReached { .. })
                )
            }));

            // Second vote - should trigger consensus
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
                instance,
                root_id.clone(),
                true
            ));

            // Now consensus should be reached
            assert_consensus_reached_event_emitted(instance, &root_id, VotingStatus::Accepted);
        });
}

#[test]
fn voting_deadline_boundary_test() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;
            let voting_period = Watchtower::get_voting_period();

            // Submit initial vote at block 1
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true
            ));

            // Roll to exactly the deadline (start_block + voting_period)
            roll_forward(voting_period);

            // Vote should still be possible at the deadline block - use split vote to avoid
            // consensus
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
                instance,
                root_id.clone(),
                false // Different vote to prevent consensus
            ));

            // Roll one more block past deadline
            roll_one_block();

            // Now voting should fail
            assert_noop!(
                Watchtower::vote(
                    RuntimeOrigin::signed(watchtower_account_3()),
                    instance,
                    root_id,
                    true
                ),
                Error::<TestRuntime>::VotingPeriodExpired
            );
        });
}

#[test]
fn lazy_cleanup_on_access_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;
            let voting_period = Watchtower::get_voting_period();

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true
            ));

            assert!(Watchtower::is_voting_active(instance, root_id.clone()));
            let status = Watchtower::get_voting_status(instance, root_id.clone());
            assert!(status.is_some());

            roll_forward(voting_period + 1);

            assert!(VotingStartBlock::<TestRuntime>::contains_key((instance, root_id.clone())));

            assert!(!Watchtower::is_voting_active(instance, root_id.clone()));

            assert!(
                !VotingStartBlock::<TestRuntime>::contains_key((instance, root_id.clone())),
                "Expired session should be cleaned up"
            );

            let root_id_2 =
                RootId { range: RootRange { from_block: 20, to_block: 30 }, ingress_counter: 1 };

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id_2.clone(),
                true
            ));

            roll_forward(voting_period + 1);

            let status = Watchtower::get_voting_status(instance, root_id_2.clone());
            assert!(status.is_none(), "Should return None for expired session");
            assert!(
                !VotingStartBlock::<TestRuntime>::contains_key((instance, root_id_2)),
                "Should be cleaned up"
            );
        });
}

#[test]
fn different_root_ids_independent_voting() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id_1 = get_test_root_id();
            let mut root_id_2 = get_test_root_id();
            root_id_2.ingress_counter = 1; // Make it different

            let instance = SummarySource::EthereumBridge;

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id_1.clone(),
                true
            ));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id_2.clone(),
                false
            ));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
                instance,
                root_id_1.clone(),
                true
            ));

            assert_consensus_reached_event_emitted(instance, &root_id_1, VotingStatus::Accepted);

            assert!(Watchtower::is_voting_active(instance, root_id_2.clone()));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
                instance,
                root_id_2,
                false
            ));
        });
}
