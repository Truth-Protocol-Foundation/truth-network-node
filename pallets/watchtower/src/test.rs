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
<<<<<<< HEAD
            let instance = SummarySource::EthereumBridge;

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
=======
            let instance = SummarySourceInstance::EthereumBridge;
            let signing_key =
                MockNodeManager::get_node_signing_key(&watchtower_account_1()).unwrap();
            let signature = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                signing_key,
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
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
<<<<<<< HEAD
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
=======
            let signing_key1 =
                MockNodeManager::get_node_signing_key(&watchtower_account_1()).unwrap();
            let signing_key2 =
                MockNodeManager::get_node_signing_key(&watchtower_account_2()).unwrap();
            let signature1 = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );
            let signature2 = create_test_signature(
                &watchtower_account_2(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                signing_key1,
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
                instance,
                root_id.clone(),
                true
            ));

<<<<<<< HEAD
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
=======
            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_2(),
                signing_key2,
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
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

<<<<<<< HEAD
            assert_consensus_reached_event_emitted(instance, &root_id, VotingStatus::Rejected);
=======
            assert_ok!(Watchtower::resolve_challenge(
                RuntimeOrigin::root(),
                instance,
                root_id.clone(),
                ChallengeResolution::SuccessfulChallenge
            ));

            assert_challenge_resolved_event_emitted(
                instance,
                &root_id,
                ChallengeResolution::SuccessfulChallenge,
            );
        });
}

#[test]
fn submit_watchtower_vote_unauthorized_fails() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let fake_signing_key = UintAuthorityId(999);
            let fake_signature = sp_runtime::testing::TestSignature(0, vec![]);

            assert_noop!(
                Watchtower::submit_watchtower_vote(
                    RuntimeOrigin::none(),
                    unauthorized_account(),
                    fake_signing_key,
                    instance,
                    root_id,
                    true,
                    fake_signature
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
            let instance = SummarySourceInstance::EthereumBridge;
            let signing_key =
                MockNodeManager::get_node_signing_key(&watchtower_account_1()).unwrap();
            let signature1 = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );
            let signature2 = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                signing_key.clone(),
                instance,
                root_id.clone(),
                true,
                signature1
            ));

            assert_noop!(
                Watchtower::submit_watchtower_vote(
                    RuntimeOrigin::none(),
                    watchtower_account_1(),
                    signing_key,
                    instance,
                    root_id,
                    true,
                    signature2
                ),
                Error::<TestRuntime>::AlreadyVoted
            );
        });
}

#[test]
fn submit_challenge_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let (incorrect_root, correct_root) = get_test_challenge_data();
            let signature = create_test_signature(
                &watchtower_account_1(),
                &(
                    crate::WATCHTOWER_CHALLENGE_CONTEXT,
                    &instance,
                    &root_id,
                    &incorrect_root,
                    &correct_root,
                ),
            );

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                instance,
                root_id.clone(),
                incorrect_root,
                correct_root,
                signature
            ));

            let challenge_key = (instance, root_id.clone());
            let challenge_info = Watchtower::challenges(&challenge_key).unwrap();
            assert_eq!(challenge_info.status, ChallengeStatus::Pending);
            assert_eq!(challenge_info.challengers.len(), 1);
            assert_eq!(challenge_info.challengers[0], watchtower_account_1());

            assert_challenge_submitted_event_emitted(
                &watchtower_account_1(),
                instance,
                &root_id,
                &incorrect_root,
                &correct_root,
                1,
            );
            assert_first_challenge_alert_event_emitted(instance, &root_id);
        });
}

#[test]
fn challenge_threshold_acceptance_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let (incorrect_root, correct_root) = get_test_challenge_data();

            // Submit challenges from 3 watchtowers (default threshold)
            for (i, account) in
                [watchtower_account_1(), watchtower_account_2(), watchtower_account_3()]
                    .iter()
                    .enumerate()
            {
                let signature = create_test_signature(
                    account,
                    &(
                        crate::WATCHTOWER_CHALLENGE_CONTEXT,
                        &instance,
                        &root_id,
                        &incorrect_root,
                        &correct_root,
                    ),
                );

                assert_ok!(Watchtower::submit_challenge(
                    RuntimeOrigin::none(),
                    account.clone(),
                    instance,
                    root_id.clone(),
                    incorrect_root,
                    correct_root,
                    signature
                ));

                // Check challenge count
                assert_challenge_submitted_event_emitted(
                    account,
                    instance,
                    &root_id,
                    &incorrect_root,
                    &correct_root,
                    (i + 1) as u32,
                );
            }

            // Should be accepted after 3 challenges (default threshold)
            let challenge_key = (instance, root_id.clone());
            let challenge_info = Watchtower::challenges(&challenge_key).unwrap();
            assert_eq!(challenge_info.status, ChallengeStatus::Accepted);

            assert_challenge_accepted_event_emitted(instance, &root_id);
        });
}

#[test]
fn challenge_unauthorized_fails() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let (incorrect_root, correct_root) = get_test_challenge_data();
            let fake_signature = sp_runtime::testing::TestSignature(0, vec![]);

            assert_noop!(
                Watchtower::submit_challenge(
                    RuntimeOrigin::none(),
                    unauthorized_account(),
                    instance,
                    root_id,
                    incorrect_root,
                    correct_root,
                    fake_signature
                ),
                Error::<TestRuntime>::NotAuthorizedWatchtower
            );
        });
}

#[test]
fn double_challenge_fails() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let (incorrect_root, correct_root) = get_test_challenge_data();
            let signature1 = create_test_signature(
                &watchtower_account_1(),
                &(
                    crate::WATCHTOWER_CHALLENGE_CONTEXT,
                    &instance,
                    &root_id,
                    &incorrect_root,
                    &correct_root,
                ),
            );
            let signature2 = create_test_signature(
                &watchtower_account_1(),
                &(
                    crate::WATCHTOWER_CHALLENGE_CONTEXT,
                    &instance,
                    &root_id,
                    &incorrect_root,
                    &correct_root,
                ),
            );

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                instance,
                root_id.clone(),
                incorrect_root,
                correct_root,
                signature1
            ));

            assert_noop!(
                Watchtower::submit_challenge(
                    RuntimeOrigin::none(),
                    watchtower_account_1(),
                    instance,
                    root_id,
                    incorrect_root,
                    correct_root,
                    signature2
                ),
                Error::<TestRuntime>::AlreadyChallenged
            );
        });
}

#[test]
fn resolve_challenge_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let (incorrect_root, correct_root) = get_test_challenge_data();

            // Submit a challenge first
            let signature = create_test_signature(
                &watchtower_account_1(),
                &(
                    crate::WATCHTOWER_CHALLENGE_CONTEXT,
                    &instance,
                    &root_id,
                    &incorrect_root,
                    &correct_root,
                ),
            );

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                instance,
                root_id.clone(),
                incorrect_root,
                correct_root,
                signature
            ));

            assert_ok!(Watchtower::resolve_challenge(
                RuntimeOrigin::root(),
                instance,
                root_id.clone(),
                ChallengeResolution::BadChallenge
            ));

            assert_eq!(Watchtower::failed_challenge_count(&watchtower_account_1()), 1);

            let challenge_key = (instance, root_id.clone());
            assert!(Watchtower::challenges(&challenge_key).is_none());

            assert_challenge_resolved_event_emitted(
                instance,
                &root_id,
                ChallengeResolution::BadChallenge,
            );
        });
}

#[test]
fn resolve_challenge_unauthorized_fails() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;

            assert_noop!(
                Watchtower::resolve_challenge(
                    RuntimeOrigin::signed(watchtower_account_1()),
                    instance,
                    root_id,
                    ChallengeResolution::BadChallenge
                ),
                sp_runtime::DispatchError::BadOrigin
            );
        });
}

#[test]
fn invalid_challenge_resolution_no_punishment() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let (incorrect_root, correct_root) = get_test_challenge_data();

            let signature = create_test_signature(
                &watchtower_account_1(),
                &(
                    crate::WATCHTOWER_CHALLENGE_CONTEXT,
                    &instance,
                    &root_id,
                    &incorrect_root,
                    &correct_root,
                ),
            );

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                instance,
                root_id.clone(),
                incorrect_root,
                correct_root,
                signature
            ));

            assert_ok!(Watchtower::resolve_challenge(
                RuntimeOrigin::root(),
                instance,
                root_id.clone(),
                ChallengeResolution::InvalidChallenge
            ));

            assert_eq!(Watchtower::failed_challenge_count(&watchtower_account_1()), 0);
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
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
<<<<<<< HEAD
            // Try to set voting period from non-root origin
            assert_noop!(
                Watchtower::set_voting_period(
                    RuntimeOrigin::signed(watchtower_account_1()),
                    200u64
=======
            let root_id = get_test_root_id();
            let ethereum_instance = SummarySourceInstance::EthereumBridge;
            let anchor_instance = SummarySourceInstance::AnchorStorage;

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                ethereum_instance,
                root_id.clone(),
                get_test_onchain_hash()
            ));

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                anchor_instance,
                root_id.clone(),
                get_test_onchain_hash()
            ));

            let signing_key1 =
                MockNodeManager::get_node_signing_key(&watchtower_account_1()).unwrap();
            let signing_key2 =
                MockNodeManager::get_node_signing_key(&watchtower_account_2()).unwrap();
            let eth_sig1 = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &ethereum_instance, &root_id, true),
            );
            let eth_sig2 = create_test_signature(
                &watchtower_account_2(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &ethereum_instance, &root_id, true),
            );

            let (incorrect_root, correct_root) = get_test_challenge_data();
            let anchor_challenge1 = create_test_signature(
                &watchtower_account_1(),
                &(
                    crate::WATCHTOWER_CHALLENGE_CONTEXT,
                    &anchor_instance,
                    &root_id,
                    &incorrect_root,
                    &correct_root,
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
                ),
                sp_runtime::DispatchError::BadOrigin
            );
<<<<<<< HEAD
=======

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                signing_key1,
                ethereum_instance,
                root_id.clone(),
                true,
                eth_sig1
            ));

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_2(),
                signing_key2,
                ethereum_instance,
                root_id.clone(),
                true,
                eth_sig2
            ));

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                anchor_instance,
                root_id.clone(),
                incorrect_root,
                correct_root,
                anchor_challenge1
            ));

            assert_consensus_reached_event_emitted(
                ethereum_instance,
                &root_id,
                WatchtowerSummaryStatus::Accepted,
            );

            let anchor_challenge_key = (anchor_instance, root_id.clone());
            let anchor_challenge = Watchtower::challenges(&anchor_challenge_key).unwrap();
            assert_eq!(anchor_challenge.status, ChallengeStatus::Pending);

            assert_ok!(Watchtower::resolve_challenge(
                RuntimeOrigin::root(),
                anchor_instance,
                root_id.clone(),
                ChallengeResolution::SuccessfulChallenge
            ));

            assert_challenge_resolved_event_emitted(
                anchor_instance,
                &root_id,
                ChallengeResolution::SuccessfulChallenge,
            );
        });
}

#[test]
fn challenges_and_votes_work_independently() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let (incorrect_root, correct_root) = get_test_challenge_data();

            let signing_key =
                MockNodeManager::get_node_signing_key(&watchtower_account_1()).unwrap();
            let vote_signature = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                signing_key,
                instance,
                root_id.clone(),
                true,
                vote_signature
            ));

            let challenge_signature = create_test_signature(
                &watchtower_account_2(),
                &(
                    crate::WATCHTOWER_CHALLENGE_CONTEXT,
                    &instance,
                    &root_id,
                    &incorrect_root,
                    &correct_root,
                ),
            );

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                watchtower_account_2(),
                instance,
                root_id.clone(),
                incorrect_root,
                correct_root,
                challenge_signature
            ));

            assert!(Watchtower::is_voting_active(instance, root_id.clone()));

            let challenge_key = (instance, root_id);
            let challenge_info = Watchtower::challenges(&challenge_key).unwrap();
            assert_eq!(challenge_info.status, ChallengeStatus::Pending);
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
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
            let signing_key =
                MockNodeManager::get_node_signing_key(&watchtower_account_1()).unwrap();
            let signature = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                signing_key,
                instance,
                root_id.clone(),
                true,
                signature
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

<<<<<<< HEAD
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
=======
            let signing_key1 =
                MockNodeManager::get_node_signing_key(&watchtower_account_1()).unwrap();
            let signing_key2 =
                MockNodeManager::get_node_signing_key(&watchtower_account_2()).unwrap();
            let signing_key3 =
                MockNodeManager::get_node_signing_key(&watchtower_account_3()).unwrap();
            let signature1 = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );
            let signature2 = create_test_signature(
                &watchtower_account_2(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, false),
            );
            let signature3 = create_test_signature(
                &watchtower_account_3(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                signing_key1,
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
                instance,
                root_id.clone(),
                true,
                signature1
            ));

            let (yes_votes, no_votes) = Watchtower::vote_counters(instance, root_id.clone());
            assert_eq!(yes_votes, 1);
            assert_eq!(no_votes, 0);

<<<<<<< HEAD
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
=======
            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_2(),
                signing_key2,
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
                instance,
                root_id.clone(),
                false,
                signature2
            ));

            let (yes_votes, no_votes) = Watchtower::vote_counters(instance, root_id.clone());
            assert_eq!(yes_votes, 1);
            assert_eq!(no_votes, 1);

<<<<<<< HEAD
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_3()),
=======
            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_3(),
                signing_key3,
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
                instance,
                root_id.clone(),
                true,
                signature3
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
<<<<<<< HEAD
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
=======
            let signing_key1 =
                MockNodeManager::get_node_signing_key(&watchtower_account_1()).unwrap();
            let signing_key2 =
                MockNodeManager::get_node_signing_key(&watchtower_account_2()).unwrap();
            let signature1 = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );
            let signature2 = create_test_signature(
                &watchtower_account_2(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                signing_key1,
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
                instance,
                root_id.clone(),
                true,
                signature1
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
<<<<<<< HEAD
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
=======
            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_2(),
                signing_key2,
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
                instance,
                root_id.clone(),
                true,
                signature2
            ));

            // Now consensus should be reached
<<<<<<< HEAD
            assert_consensus_reached_event_emitted(instance, &root_id, VotingStatus::Accepted);
=======
            assert_consensus_reached_event_emitted(
                instance,
                &root_id,
                WatchtowerSummaryStatus::Accepted,
            );
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
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

<<<<<<< HEAD
            // Submit initial vote at block 1
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
=======
            assert_ok!(Watchtower::set_voting_period(RuntimeOrigin::root(), 10u64));

            let signing_key1 =
                MockNodeManager::get_node_signing_key(&watchtower_account_1()).unwrap();
            let signing_key2 =
                MockNodeManager::get_node_signing_key(&watchtower_account_2()).unwrap();
            let signature1 = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                signing_key1,
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
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
<<<<<<< HEAD
                Watchtower::vote(
                    RuntimeOrigin::signed(watchtower_account_3()),
=======
                Watchtower::submit_watchtower_vote(
                    RuntimeOrigin::none(),
                    watchtower_account_2(),
                    signing_key2,
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
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

<<<<<<< HEAD
            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
=======
            assert_ok!(Watchtower::set_voting_period(RuntimeOrigin::root(), 10u64));

            let signing_key =
                MockNodeManager::get_node_signing_key(&watchtower_account_1()).unwrap();
            let signature = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                signing_key,
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
                instance,
                root_id.clone(),
                true
            ));

            assert!(Watchtower::is_voting_active(instance, root_id.clone()));
            let status = Watchtower::get_voting_status(instance, root_id.clone());
            assert!(status.is_some());

<<<<<<< HEAD
            roll_forward(voting_period + 1);

            assert!(VotingStartBlock::<TestRuntime>::contains_key((instance, root_id.clone())));
=======
            // Check that voting is active (no direct way to check individual votes)
            let (yes_votes, no_votes) = Watchtower::vote_counters(instance, root_id.clone());
            assert_eq!(yes_votes, 1);
            assert_eq!(no_votes, 0);

            roll_forward(15);

            assert_ok!(Watchtower::cleanup_expired_votes(instance, root_id.clone()));

            // Check that vote counters are reset after cleanup
            let (yes_votes_after, no_votes_after) =
                Watchtower::vote_counters(instance, root_id.clone());
            assert_eq!(yes_votes_after, 0);
            assert_eq!(no_votes_after, 0);
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)

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

<<<<<<< HEAD
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
=======
            assert_noop!(
                Watchtower::cleanup_expired_votes(instance, root_id),
                Error::<TestRuntime>::VotingNotStarted
            );
        });
}

// #[test]
// fn consensus_attempts_after_expiration_fail() {
//     ExtBuilder::build_default()
//         .with_watchtowers()
//         .as_externality()
//         .execute_with(|| {
//             let root_id = get_test_root_id();
//             let instance = SummarySourceInstance::EthereumBridge;

//             assert_ok!(Watchtower::set_voting_period(RuntimeOrigin::root(), 10u64));

//             let signing_key =
// MockNodeManager::get_node_signing_key(&watchtower_account_1()).unwrap();             let
// signature1 = create_test_signature(                 &watchtower_account_1(),
//                 &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
//             );

//             assert_ok!(Watchtower::submit_watchtower_vote(
//                 RuntimeOrigin::none(),
//                 watchtower_account_1(),
//                 signing_key,
//                 instance,
//                 root_id.clone(),
//                 true,
//                 signature1
//             ));

//             roll_forward(15);

//             let result = Watchtower::try_reach_consensus(instance, root_id.clone());

//             assert_noop!(result, Error::<TestRuntime>::VotingPeriodExpired);

//             // Check that vote counters are cleared after period expiration
//             let (yes_votes, no_votes) = Watchtower::vote_counters(instance, root_id.clone());
//             assert_eq!(yes_votes, 0);
//             assert_eq!(no_votes, 0);

//             let consensus_key = (instance, root_id);
//             assert!(Watchtower::voting_start_block(&consensus_key).is_none());
//         });
// }

// === NOTIFICATION SYSTEM TESTS ===

#[test]
fn duplicate_notifications_are_handled_gracefully() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let root_hash = get_test_onchain_hash();

            // Send first notification
            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                root_hash
            ));

            // Verify voting was started
            let consensus_key = (instance, root_id.clone());
            assert!(Watchtower::voting_start_block(&consensus_key).is_some());
            assert!(Watchtower::pending_validation_root_hash(&consensus_key).is_some());

            // Send duplicate notification - should be ignored
            let different_hash = H256::from([2u8; 32]);
            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                different_hash
            ));

            // Verify original state is unchanged
            assert_eq!(
                Watchtower::pending_validation_root_hash(&consensus_key).unwrap(),
                root_hash
            );
        });
}

#[test]
fn zero_hash_handling_rejects_notification() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let zero_hash = H256::zero();

            // Zero hash should be rejected with an error
            assert_noop!(
                Watchtower::notify_summary_ready_for_validation(
                    instance,
                    root_id.clone(),
                    zero_hash
                ),
                Error::<TestRuntime>::InvalidVerificationSubmission
            );

            // Verify notification was NOT processed
            let consensus_key = (instance, root_id);
            assert!(Watchtower::voting_start_block(&consensus_key).is_none());
            assert!(Watchtower::pending_validation_root_hash(&consensus_key).is_none());
        });
}

#[test]
fn notification_after_consensus_reached_is_ignored() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let root_hash = get_test_onchain_hash();

            // Reach consensus first
            let signing_key1 =
                MockNodeManager::get_node_signing_key(&watchtower_account_1()).unwrap();
            let signing_key2 =
                MockNodeManager::get_node_signing_key(&watchtower_account_2()).unwrap();
            let signature1 = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );
            let signature2 = create_test_signature(
                &watchtower_account_2(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                signing_key1,
                instance,
                root_id.clone(),
                true,
                signature1
            ));

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_2(),
                signing_key2,
                instance,
                root_id.clone(),
                true,
                signature2
            ));

            // Verify consensus was reached
            let consensus_key = (instance, root_id.clone());
            assert!(Watchtower::consensus_reached_flag(&consensus_key));

            // Try to send notification after consensus - should be ignored
            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance, root_id, root_hash
            ));

            // Should not affect the already reached consensus
            assert!(Watchtower::consensus_reached_flag(&consensus_key));
        });
}

#[test]
fn notification_integration_with_voting_start() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let root_hash = get_test_onchain_hash();
            let start_block = System::block_number();

            // Send notification
            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                root_hash
            ));

            let consensus_key = (instance, root_id.clone());

            // Verify voting start block was set correctly
            assert_eq!(Watchtower::voting_start_block(&consensus_key).unwrap(), start_block);

            // Verify pending validation hash was set
            assert_eq!(
                Watchtower::pending_validation_root_hash(&consensus_key).unwrap(),
                root_hash
            );

            // Verify voting is active
            assert!(Watchtower::is_voting_active(instance, root_id));
        });
}

// === CHALLENGE SYSTEM EDGE CASES ===

#[test]
fn multiple_challengers_for_same_root_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let (incorrect_root, correct_root) = get_test_challenge_data();

            // Submit challenges from multiple watchtowers
            let challengers =
                [watchtower_account_1(), watchtower_account_2(), watchtower_account_3()];

            for (i, challenger) in challengers.iter().enumerate() {
                let signature = create_test_signature(
                    challenger,
                    &(
                        crate::WATCHTOWER_CHALLENGE_CONTEXT,
                        &instance,
                        &root_id,
                        &incorrect_root,
                        &correct_root,
                    ),
                );

                assert_ok!(Watchtower::submit_challenge(
                    RuntimeOrigin::none(),
                    challenger.clone(),
                    instance,
                    root_id.clone(),
                    incorrect_root,
                    correct_root,
                    signature
                ));

                // Verify challenge count increases
                let challenge_key = (instance, root_id.clone());
                let challenge_info = Watchtower::challenges(&challenge_key).unwrap();
                assert_eq!(challenge_info.challengers.len(), i + 1);
                assert!(challenge_info.challengers.contains(challenger));

                // First challenge should trigger alert
                if i == 0 {
                    assert!(challenge_info.first_challenge_alert_sent);
                }
            }

            // After 3 challenges (default threshold), status should be Accepted
            let challenge_key = (instance, root_id);
            let final_challenge_info = Watchtower::challenges(&challenge_key).unwrap();
            assert_eq!(final_challenge_info.status, ChallengeStatus::Accepted);
            assert_eq!(final_challenge_info.challengers.len(), 3);
        });
}

#[test]
fn failed_challenge_count_management_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let (incorrect_root, correct_root) = get_test_challenge_data();

            // Initial failed challenge count should be 0
            assert_eq!(Watchtower::failed_challenge_count(&watchtower_account_1()), 0);
            assert_eq!(Watchtower::failed_challenge_count(&watchtower_account_2()), 0);

            // Submit challenges from two watchtowers
            let signature1 = create_test_signature(
                &watchtower_account_1(),
                &(
                    crate::WATCHTOWER_CHALLENGE_CONTEXT,
                    &instance,
                    &root_id,
                    &incorrect_root,
                    &correct_root,
                ),
            );
            let signature2 = create_test_signature(
                &watchtower_account_2(),
                &(
                    crate::WATCHTOWER_CHALLENGE_CONTEXT,
                    &instance,
                    &root_id,
                    &incorrect_root,
                    &correct_root,
                ),
            );

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                instance,
                root_id.clone(),
                incorrect_root,
                correct_root,
                signature1
            ));

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                watchtower_account_2(),
                instance,
                root_id.clone(),
                incorrect_root,
                correct_root,
                signature2
            ));

            // Verify total challenge counts were incremented
            assert_eq!(Watchtower::total_challenge_count(&watchtower_account_1()), 1);
            assert_eq!(Watchtower::total_challenge_count(&watchtower_account_2()), 1);

            // Resolve as bad challenge
            assert_ok!(Watchtower::resolve_challenge(
                RuntimeOrigin::root(),
                instance,
                root_id.clone(),
                ChallengeResolution::BadChallenge
            ));

            // Verify failed challenge counts were incremented for both challengers
            assert_eq!(Watchtower::failed_challenge_count(&watchtower_account_1()), 1);
            assert_eq!(Watchtower::failed_challenge_count(&watchtower_account_2()), 1);

            // Test reset functionality (via ChallengeRewardInterface)
            use crate::ChallengeRewardInterface;
            Watchtower::reset_failed_challenge_count(&watchtower_account_1());
            assert_eq!(Watchtower::failed_challenge_count(&watchtower_account_1()), 0);
            assert_eq!(Watchtower::failed_challenge_count(&watchtower_account_2()), 1); // Should remain unchanged
        });
}

#[test]
fn challenge_resolution_with_invalid_challenge_no_punishment() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let (incorrect_root, correct_root) = get_test_challenge_data();

            // Submit challenge
            let signature = create_test_signature(
                &watchtower_account_1(),
                &(
                    crate::WATCHTOWER_CHALLENGE_CONTEXT,
                    &instance,
                    &root_id,
                    &incorrect_root,
                    &correct_root,
                ),
            );

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                instance,
                root_id.clone(),
                incorrect_root,
                correct_root,
                signature
            ));

            // Verify total challenge count was incremented
            assert_eq!(Watchtower::total_challenge_count(&watchtower_account_1()), 1);
            assert_eq!(Watchtower::failed_challenge_count(&watchtower_account_1()), 0);

            // Resolve as invalid challenge (good faith, but incorrect)
            assert_ok!(Watchtower::resolve_challenge(
                RuntimeOrigin::root(),
                instance,
                root_id,
                ChallengeResolution::InvalidChallenge
            ));

            // Failed challenge count should NOT be incremented for invalid challenges
            assert_eq!(Watchtower::failed_challenge_count(&watchtower_account_1()), 0);
            // But total challenge count remains
            assert_eq!(Watchtower::total_challenge_count(&watchtower_account_1()), 1);
        });
}

#[test]
fn challenge_already_resolved_prevents_double_resolution() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let (incorrect_root, correct_root) = get_test_challenge_data();

            // Submit challenge
            let signature = create_test_signature(
                &watchtower_account_1(),
                &(
                    crate::WATCHTOWER_CHALLENGE_CONTEXT,
                    &instance,
                    &root_id,
                    &incorrect_root,
                    &correct_root,
                ),
            );

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                instance,
                root_id.clone(),
                incorrect_root,
                correct_root,
                signature
            ));

            // Resolve challenge first time
            assert_ok!(Watchtower::resolve_challenge(
                RuntimeOrigin::root(),
                instance,
                root_id.clone(),
                ChallengeResolution::BadChallenge
            ));

            // Try to resolve again - should fail because challenge was removed
            assert_noop!(
                Watchtower::resolve_challenge(
                    RuntimeOrigin::root(),
                    instance,
                    root_id,
                    ChallengeResolution::InvalidChallenge
                ),
                Error::<TestRuntime>::ChallengeNotFound
            );
        });
}

#[test]
fn challenge_count_tracking_across_multiple_challenges() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let instance = SummarySourceInstance::EthereumBridge;
            let challenger = watchtower_account_1();

            // Submit multiple challenges for different roots
            for i in 1..=3 {
                let root_id = sp_avn_common::RootId {
                    range: sp_avn_common::RootRange { from_block: i, to_block: i + 10 },
                    ingress_counter: 0,
                };
                let (incorrect_root, correct_root) =
                    (H256::from([i as u8; 32]), H256::from([(i + 100) as u8; 32]));

                let signature = create_test_signature(
                    &challenger,
                    &(
                        crate::WATCHTOWER_CHALLENGE_CONTEXT,
                        &instance,
                        &root_id,
                        &incorrect_root,
                        &correct_root,
                    ),
                );

                assert_ok!(Watchtower::submit_challenge(
                    RuntimeOrigin::none(),
                    challenger.clone(),
                    instance,
                    root_id,
                    incorrect_root,
                    correct_root,
                    signature
                ));

                // Verify total challenge count increases with each challenge
                assert_eq!(Watchtower::total_challenge_count(&challenger), i as u32);
            }

            // Failed challenge count should still be 0 (no resolutions yet)
            assert_eq!(Watchtower::failed_challenge_count(&challenger), 0);
        });
}

#[test]
fn set_challenge_resolution_admin_works() {
    ExtBuilder::build_default().as_externality().execute_with(|| {
        let new_admin = watchtower_account_1();

        assert!(Watchtower::challenge_resolution_admin().is_none());

        assert_ok!(Watchtower::set_challenge_resolution_admin(
            RuntimeOrigin::root(),
            Some(new_admin.clone())
        ));

        assert_eq!(Watchtower::challenge_resolution_admin(), Some(new_admin.clone()));

        let events = System::events();
        assert!(events.iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::Watchtower(crate::Event::ChallengeResolutionAdminUpdated {
                    old_admin: None,
                    new_admin: Some(ref admin)
                }) if admin == &new_admin
            )
        }));

        // Remove admin
        assert_ok!(Watchtower::set_challenge_resolution_admin(RuntimeOrigin::root(), None));

        assert!(Watchtower::challenge_resolution_admin().is_none());
    });
}

#[test]
fn challenge_resolution_with_admin_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let admin = watchtower_account_3(); // Not the challenger
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let (incorrect_root, correct_root) = get_test_challenge_data();

            assert_ok!(Watchtower::set_challenge_resolution_admin(
                RuntimeOrigin::root(),
                Some(admin.clone())
            ));

            let signature = create_test_signature(
                &watchtower_account_1(),
                &(
                    crate::WATCHTOWER_CHALLENGE_CONTEXT,
                    &instance,
                    &root_id,
                    &incorrect_root,
                    &correct_root,
                ),
            );

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                instance,
                root_id.clone(),
                incorrect_root,
                correct_root,
                signature
            ));

            assert_ok!(Watchtower::resolve_challenge(
                RuntimeOrigin::signed(admin),
                instance,
                root_id.clone(),
                ChallengeResolution::InvalidChallenge
            ));

            let challenge_key = (instance, root_id);
            assert!(Watchtower::challenges(&challenge_key).is_none());
        });
}

#[test]
fn challenge_resolution_admin_unauthorized_fails() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let admin = watchtower_account_3();
            let non_admin = watchtower_account_1();
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let (incorrect_root, correct_root) = get_test_challenge_data();

            assert_ok!(Watchtower::set_challenge_resolution_admin(
                RuntimeOrigin::root(),
                Some(admin)
            ));

            let signature = create_test_signature(
                &non_admin,
                &(
                    crate::WATCHTOWER_CHALLENGE_CONTEXT,
                    &instance,
                    &root_id,
                    &incorrect_root,
                    &correct_root,
                ),
            );

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                non_admin.clone(),
                instance,
                root_id.clone(),
                incorrect_root,
                correct_root,
                signature
            ));

            assert_noop!(
                Watchtower::resolve_challenge(
                    RuntimeOrigin::signed(non_admin),
                    instance,
                    root_id,
                    ChallengeResolution::InvalidChallenge
                ),
                Error::<TestRuntime>::InvalidChallengeResolutionAdmin
            );
>>>>>>> b180ed8 (fix: rebased on latest version of the watchtower branch and refactored the logic)
        });
}
