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
            assert_eq!(System::block_number(), 1);

            assert!(MockNodeManager::is_authorized_watchtower(&watchtower_account_1()));
            assert!(MockNodeManager::is_authorized_watchtower(&watchtower_account_2()));
            assert!(MockNodeManager::is_authorized_watchtower(&watchtower_account_3()));
            assert!(!MockNodeManager::is_authorized_watchtower(&unauthorized_account()));

            let watchtower_count = MockNodeManager::get_authorized_watchtowers_count();
            assert_eq!(watchtower_count, 3);

            assert!(MockNodeManager::get_node_signing_key(&watchtower_account_1()).is_some());
            assert!(MockNodeManager::get_node_signing_key(&watchtower_account_2()).is_some());
            assert!(MockNodeManager::get_node_signing_key(&watchtower_account_3()).is_some());
            assert!(MockNodeManager::get_node_signing_key(&unauthorized_account()).is_none());

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
            let root_hash = get_test_onchain_hash();

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                root_hash
            ));

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
            let root_hash = get_test_onchain_hash();

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                root_hash
            ));

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
            let root_hash = get_test_onchain_hash();

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                root_hash
            ));

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

            assert_consensus_reached_event_emitted(instance, &root_id, VotingStatus::PendingChallengeResolution);
            
            let challenge_key = (instance, root_id.clone());
            let challenge_info = Watchtower::challenges(&challenge_key);
            assert!(challenge_info.is_some(), "Challenge should be created automatically");
            
            let challenge = challenge_info.unwrap();
            assert_eq!(challenge.status, crate::ChallengeStatus::Pending);
            assert_eq!(challenge.original_consensus, Some(VotingStatus::Rejected));
        });
}

#[test]
fn voting_period_update_works() {
    ExtBuilder::build_default().as_externality().execute_with(|| {
        let old_period = Watchtower::get_voting_period();
        let new_period = 200u64;

        assert_ok!(Watchtower::set_voting_period(RuntimeOrigin::root(), new_period));

        assert_eq!(Watchtower::get_voting_period(), new_period);

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
            let root_hash = get_test_onchain_hash();

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                ethereum_instance,
                root_id.clone(),
                root_hash
            ));
            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                anchor_instance,
                root_id.clone(),
                root_hash
            ));

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
                VotingStatus::PendingChallengeResolution,
            );
            
            let challenge_key = (anchor_instance, root_id.clone());
            let challenge_info = Watchtower::challenges(&challenge_key);
            assert!(challenge_info.is_some(), "Challenge should be created automatically for anchor instance");
            
            let challenge = challenge_info.unwrap();
            assert_eq!(challenge.status, crate::ChallengeStatus::Pending);
            assert_eq!(challenge.original_consensus, Some(VotingStatus::Rejected));
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
            let root_hash = get_test_onchain_hash();

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                root_hash
            ));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true
            ));

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
            let root_hash = get_test_onchain_hash();

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                root_hash
            ));

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

            assert_noop!(
                Watchtower::vote(
                    RuntimeOrigin::signed(watchtower_account_3()),
                    instance,
                    root_id,
                    false
                ),
                Error::<TestRuntime>::VotingNotStarted
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
            let root_hash = get_test_onchain_hash();
            let voting_period = Watchtower::get_voting_period();

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                root_hash
            ));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true
            ));

            roll_forward(voting_period + 1);

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
            let root_hash = get_test_onchain_hash();

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                root_hash
            ));

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

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_3()),
                instance,
                root_id.clone(),
                true
            ));

            assert_consensus_reached_event_emitted(instance, &root_id, VotingStatus::Accepted);
        });
}

#[test]
fn invalid_voting_period_update_fails() {
    ExtBuilder::build_default().as_externality().execute_with(|| {
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

                let signing_key =
                    MockNodeManager::get_node_signing_key(&watchtower_account_1()).unwrap();

                let data = (crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, vote_is_valid);
                let signature = signing_key.sign(&data.encode()).unwrap();

                assert!(Watchtower::offchain_signature_is_valid(&data, &signing_key, &signature));

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
        let valid_response =
            b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_vec();
        let result = Watchtower::validate_response(valid_response);
        assert!(result.is_ok());

        let invalid_length_response = b"0123456789abcdef".to_vec();
        let result = Watchtower::validate_response(invalid_length_response);
        assert!(result.is_err());

        let invalid_hex_response =
            b"gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg".to_vec();
        let result = Watchtower::validate_response(invalid_hex_response);
        assert!(result.is_err());

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
            let root_hash = get_test_onchain_hash();

            let status = Watchtower::get_voting_status(instance, root_id.clone());
            assert!(status.is_none());

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                root_hash
            ));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true,
            ));

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
            let root_hash = get_test_onchain_hash();

            let (yes_votes, no_votes) = Watchtower::vote_counters(instance, root_id.clone());
            assert_eq!(yes_votes, 0);
            assert_eq!(no_votes, 0);

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                root_hash
            ));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true,
            ));

            let (yes_votes, no_votes) = Watchtower::vote_counters(instance, root_id.clone());
            assert_eq!(yes_votes, 1);
            assert_eq!(no_votes, 0);

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
                instance,
                root_id.clone(),
                false,
            ));

            let (yes_votes, no_votes) = Watchtower::vote_counters(instance, root_id.clone());
            assert_eq!(yes_votes, 1);
            assert_eq!(no_votes, 1);

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_3()),
                instance,
                root_id.clone(),
                true,
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
            let root_hash = get_test_onchain_hash();

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                root_hash
            ));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true,
            ));

            let events = System::events();
            assert!(!events.iter().any(|record| {
                matches!(
                    record.event,
                    RuntimeEvent::Watchtower(WatchtowerEvent::WatchtowerConsensusReached { .. })
                )
            }));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
                instance,
                root_id.clone(),
                true,
            ));

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
            let root_hash = get_test_onchain_hash();
            let voting_period = Watchtower::get_voting_period();

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                root_hash
            ));

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_1()),
                instance,
                root_id.clone(),
                true
            ));

            roll_forward(voting_period);

            assert_ok!(Watchtower::vote(
                RuntimeOrigin::signed(watchtower_account_2()),
                instance,
                root_id.clone(),
                false
            ));

            roll_one_block();

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
            let root_hash = get_test_onchain_hash();
            let voting_period = Watchtower::get_voting_period();

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                root_hash
            ));

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

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id_2.clone(),
                root_hash
            ));

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
            root_id_2.ingress_counter = 1;

            let instance = SummarySource::EthereumBridge;
            let root_hash = get_test_onchain_hash();

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id_1.clone(),
                root_hash
            ));
            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id_2.clone(),
                root_hash
            ));

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

#[test]
fn challenge_submission_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;
            let incorrect_root_hash = get_test_onchain_hash();
            let correct_root_hash = sp_core::H256::from([2u8; 32]);
            let challenger = watchtower_account_1();
            let signing_key = MockNodeManager::get_node_signing_key(&challenger).unwrap();

            let data = (
                crate::WATCHTOWER_CHALLENGE_CONTEXT,
                &instance,
                &root_id,
                &incorrect_root_hash,
                &correct_root_hash,
            );
            let signature = signing_key.sign(&data.encode()).unwrap();

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                challenger.clone(),
                instance,
                root_id.clone(),
                incorrect_root_hash,
                correct_root_hash,
                signature
            ));

            let challenge_key = (instance, root_id.clone());
            let challenge_info = Watchtower::challenges(&challenge_key);
            assert!(challenge_info.is_some(), "Challenge should be stored");

            let challenge = challenge_info.unwrap();
            assert_eq!(challenge.challengers.len(), 1);
            assert_eq!(challenge.challengers[0], challenger);
            assert_eq!(challenge.status, crate::ChallengeStatus::Pending);
            assert_eq!(challenge.incorrect_root_id, incorrect_root_hash);
            assert_eq!(challenge.correct_root_hash, correct_root_hash);

            let events = System::events();
            assert!(events.iter().any(|record| {
                matches!(
                    record.event,
                    RuntimeEvent::Watchtower(crate::Event::ChallengeSubmitted { .. })
                )
            }));

            assert!(events.iter().any(|record| {
                matches!(
                    record.event,
                    RuntimeEvent::Watchtower(crate::Event::FirstChallengeAlert { .. })
                )
            }));
        });
}

#[test]
fn challenge_threshold_acceptance_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;
            let incorrect_root_hash = get_test_onchain_hash();
            let correct_root_hash = sp_core::H256::from([2u8; 32]);

            let challenger1 = watchtower_account_1();
            let signing_key1 = MockNodeManager::get_node_signing_key(&challenger1).unwrap();
            let data = (
                crate::WATCHTOWER_CHALLENGE_CONTEXT,
                &instance,
                &root_id,
                &incorrect_root_hash,
                &correct_root_hash,
            );
            let signature1 = signing_key1.sign(&data.encode()).unwrap();

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                challenger1.clone(),
                instance,
                root_id.clone(),
                incorrect_root_hash,
                correct_root_hash,
                signature1
            ));

            let challenge_key = (instance, root_id.clone());
            let challenge = Watchtower::challenges(&challenge_key).unwrap();
            assert_eq!(challenge.status, crate::ChallengeStatus::Pending);

            let challenger2 = watchtower_account_2();
            let signing_key2 = MockNodeManager::get_node_signing_key(&challenger2).unwrap();
            let signature2 = signing_key2.sign(&data.encode()).unwrap();

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                challenger2.clone(),
                instance,
                root_id.clone(),
                incorrect_root_hash,
                correct_root_hash,
                signature2
            ));

            let challenge = Watchtower::challenges(&challenge_key).unwrap();
            assert_eq!(challenge.status, crate::ChallengeStatus::Accepted);
            assert_eq!(challenge.challengers.len(), 2);

            let events = System::events();
            assert!(events.iter().any(|record| {
                matches!(
                    record.event,
                    RuntimeEvent::Watchtower(crate::Event::ChallengeAccepted { .. })
                )
            }));
        });
}

#[test]
fn challenge_resolution_bad_challenge_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;
            let incorrect_root_hash = get_test_onchain_hash();
            let correct_root_hash = sp_core::H256::from([2u8; 32]);
            let challenger = watchtower_account_1();

            let challenge_info = crate::ChallengeInfo {
                incorrect_root_id: incorrect_root_hash,
                correct_root_hash,
                challengers: vec![challenger.clone()].try_into().unwrap(),
                status: crate::ChallengeStatus::Accepted,
                created_block: 1u32,
                first_challenge_alert_sent: true,
                original_consensus: Some(VotingStatus::Accepted),
            };
            let challenge_key = (instance, root_id.clone());
            crate::Challenges::<TestRuntime>::insert(&challenge_key, challenge_info);

            assert_eq!(Watchtower::failed_challenge_count(&challenger), 0);

            assert_ok!(Watchtower::resolve_challenge(
                RuntimeOrigin::root(),
                instance,
                root_id.clone(),
                crate::ChallengeResolution::BadChallenge
            ));

            assert!(Watchtower::challenges(&challenge_key).is_none());

            assert_eq!(Watchtower::failed_challenge_count(&challenger), 1);

            let events = System::events();
            assert!(events.iter().any(|record| {
                matches!(
                    record.event,
                    RuntimeEvent::Watchtower(crate::Event::ChallengeResolved {
                        resolution: crate::ChallengeResolution::BadChallenge,
                        ..
                    })
                )
            }));
        });
}

#[test]
fn challenge_resolution_successful_challenge_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;
            let incorrect_root_hash = get_test_onchain_hash();
            let correct_root_hash = sp_core::H256::from([2u8; 32]);
            let challenger = watchtower_account_1();

            let challenge_info = crate::ChallengeInfo {
                incorrect_root_id: incorrect_root_hash,
                correct_root_hash,
                challengers: vec![challenger.clone()].try_into().unwrap(),
                status: crate::ChallengeStatus::Accepted,
                created_block: 1u32,
                first_challenge_alert_sent: true,
                original_consensus: Some(VotingStatus::Accepted),
            };
            let challenge_key = (instance, root_id.clone());
            crate::Challenges::<TestRuntime>::insert(&challenge_key, challenge_info);

            assert_eq!(Watchtower::failed_challenge_count(&challenger), 0);

            assert_ok!(Watchtower::resolve_challenge(
                RuntimeOrigin::root(),
                instance,
                root_id.clone(),
                crate::ChallengeResolution::SuccessfulChallenge
            ));

            assert!(Watchtower::challenges(&challenge_key).is_none());

            assert_eq!(Watchtower::failed_challenge_count(&challenger), 0);

            let events = System::events();
            assert!(events.iter().any(|record| {
                matches!(
                    record.event,
                    RuntimeEvent::Watchtower(crate::Event::ChallengeResolved {
                        resolution: crate::ChallengeResolution::SuccessfulChallenge,
                        ..
                    })
                )
            }));
        });
}

#[test]
fn challenge_resolution_invalid_challenge_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;
            let incorrect_root_hash = get_test_onchain_hash();
            let correct_root_hash = sp_core::H256::from([2u8; 32]);
            let challenger = watchtower_account_1();

            let challenge_info = crate::ChallengeInfo {
                incorrect_root_id: incorrect_root_hash,
                correct_root_hash,
                challengers: vec![challenger.clone()].try_into().unwrap(),
                status: crate::ChallengeStatus::Accepted,
                created_block: 1u32,
                first_challenge_alert_sent: true,
                original_consensus: Some(VotingStatus::Accepted),
            };
            let challenge_key = (instance, root_id.clone());
            crate::Challenges::<TestRuntime>::insert(&challenge_key, challenge_info);

            assert_ok!(Watchtower::resolve_challenge(
                RuntimeOrigin::root(),
                instance,
                root_id.clone(),
                crate::ChallengeResolution::InvalidChallenge
            ));

            assert!(Watchtower::challenges(&challenge_key).is_none());

            assert_eq!(Watchtower::failed_challenge_count(&challenger), 0);

            let events = System::events();
            assert!(events.iter().any(|record| {
                matches!(
                    record.event,
                    RuntimeEvent::Watchtower(crate::Event::ChallengeResolved {
                        resolution: crate::ChallengeResolution::InvalidChallenge,
                        ..
                    })
                )
            }));
        });
}

#[test]
fn automatic_challenge_on_negative_consensus_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;
            let root_hash = get_test_onchain_hash();

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                root_hash
            ));

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

            let challenge_key = (instance, root_id.clone());
            let challenge_info = Watchtower::challenges(&challenge_key);
            assert!(challenge_info.is_some(), "Automatic challenge should be created");

            let challenge = challenge_info.unwrap();
            assert_eq!(challenge.status, crate::ChallengeStatus::Pending);
            assert_eq!(challenge.original_consensus, Some(VotingStatus::Rejected));
            assert_eq!(challenge.incorrect_root_id, root_hash);
            assert_eq!(challenge.challengers.len(), 0);

            let events = System::events();
            assert!(events.iter().any(|record| {
                matches!(
                    record.event,
                    RuntimeEvent::Watchtower(crate::Event::ChallengesPresentedToAdmin { .. })
                )
            }));

            assert!(events.iter().any(|record| {
                matches!(
                    record.event,
                    RuntimeEvent::Watchtower(crate::Event::WatchtowerConsensusReached {
                        consensus_result: VotingStatus::PendingChallengeResolution,
                        ..
                    })
                )
            }));
        });
}

#[test]
fn duplicate_challenge_submission_fails() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;
            let incorrect_root_hash = get_test_onchain_hash();
            let correct_root_hash = sp_core::H256::from([2u8; 32]);
            let challenger = watchtower_account_1();
            let signing_key = MockNodeManager::get_node_signing_key(&challenger).unwrap();

            let data = (
                crate::WATCHTOWER_CHALLENGE_CONTEXT,
                &instance,
                &root_id,
                &incorrect_root_hash,
                &correct_root_hash,
            );
            let signature = signing_key.sign(&data.encode()).unwrap();

            assert_ok!(Watchtower::submit_challenge(
                RuntimeOrigin::none(),
                challenger.clone(),
                instance,
                root_id.clone(),
                incorrect_root_hash,
                correct_root_hash,
                signature.clone()
            ));

            assert_noop!(
                Watchtower::submit_challenge(
                    RuntimeOrigin::none(),
                    challenger,
                    instance,
                    root_id,
                    incorrect_root_hash,
                    correct_root_hash,
                    signature
                ),
                Error::<TestRuntime>::AlreadyChallenged
            );
        });
}

#[test]
fn unauthorized_challenge_submission_fails() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySource::EthereumBridge;
            let incorrect_root_hash = get_test_onchain_hash();
            let correct_root_hash = sp_core::H256::from([2u8; 32]);
            let unauthorized_challenger = unauthorized_account();

            let signing_key = sp_runtime::testing::UintAuthorityId(999);
            let data = (
                crate::WATCHTOWER_CHALLENGE_CONTEXT,
                &instance,
                &root_id,
                &incorrect_root_hash,
                &correct_root_hash,
            );
            let signature = signing_key.sign(&data.encode()).unwrap();

            assert_noop!(
                Watchtower::submit_challenge(
                    RuntimeOrigin::none(),
                    unauthorized_challenger,
                    instance,
                    root_id,
                    incorrect_root_hash,
                    correct_root_hash,
                    signature
                ),
                Error::<TestRuntime>::NotAuthorizedWatchtower
            );
        });
}

#[test]
fn challenge_resolution_admin_management_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let admin = watchtower_account_1();

            assert_eq!(Watchtower::challenge_resolution_admin(), None);

            assert_ok!(Watchtower::set_challenge_resolution_admin(
                RuntimeOrigin::root(),
                Some(admin.clone())
            ));

            assert_eq!(Watchtower::challenge_resolution_admin(), Some(admin.clone()));

            assert_ok!(Watchtower::set_challenge_resolution_admin(
                RuntimeOrigin::root(),
                None
            ));

            assert_eq!(Watchtower::challenge_resolution_admin(), None);

            let events = System::events();
            assert!(events.iter().any(|record| {
                matches!(
                    record.event,
                    RuntimeEvent::Watchtower(crate::Event::ChallengeResolutionAdminUpdated { .. })
                )
            }));
        });
}
