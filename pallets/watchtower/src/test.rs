#![cfg(test)]

use super::mock::*;
use crate::{
    ChallengeResolution, ChallengeStatus, Error, Event as WatchtowerEvent, NodeManagerInterface,
    SummarySourceInstance, WatchtowerSummaryStatus,
};
use frame_support::{assert_noop, assert_ok};
use sp_core::H256;
use sp_runtime::{testing::UintAuthorityId, RuntimeAppPublic};

fn get_test_challenge_data() -> (H256, H256) {
    let incorrect_root = H256::from([1u8; 32]);
    let correct_root = H256::from([2u8; 32]);
    (incorrect_root, correct_root)
}

fn create_test_signature(
    account: &AccountId,
    data: &impl parity_scale_codec::Encode,
) -> <UintAuthorityId as sp_runtime::RuntimeAppPublic>::Signature {
    let signing_key = MockNodeManager::get_node_signing_key(account).unwrap();
    signing_key.sign(&data.encode()).unwrap()
}

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

            // Test that authorized watchtowers list works
            let watchtowers = MockNodeManager::get_authorized_watchtowers().unwrap();
            assert_eq!(watchtowers.len(), 3);

            // Test that signing keys are available
            assert!(MockNodeManager::get_node_signing_key(&watchtower_account_1()).is_some());
            assert!(MockNodeManager::get_node_signing_key(&watchtower_account_2()).is_some());
            assert!(MockNodeManager::get_node_signing_key(&watchtower_account_3()).is_some());
            assert!(MockNodeManager::get_node_signing_key(&unauthorized_account()).is_none());
        });
}

#[test]
fn submit_watchtower_vote_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let signature = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                instance,
                root_id.clone(),
                true,
                signature
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
            let instance = SummarySourceInstance::EthereumBridge;

            // Submit votes from 2 watchtowers (2/3 = majority for acceptance)
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
                instance,
                root_id.clone(),
                true,
                signature1
            ));

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_2(),
                instance,
                root_id.clone(),
                true,
                signature2
            ));

            assert_consensus_reached_event_emitted(
                instance,
                &root_id,
                WatchtowerSummaryStatus::Accepted,
            );

            assert!(!Watchtower::is_voting_active(instance, root_id));
        });
}

#[test]
fn voting_consensus_rejection_only_through_admin() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let (incorrect_root, correct_root) = get_test_challenge_data();

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                get_test_onchain_hash()
            ));

            let challenge_signature = create_test_signature(
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
                challenge_signature
            ));

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
            let fake_signature = sp_runtime::testing::TestSignature(0, vec![]);

            assert_noop!(
                Watchtower::submit_watchtower_vote(
                    RuntimeOrigin::none(),
                    unauthorized_account(),
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
                instance,
                root_id.clone(),
                true,
                signature1
            ));

            assert_noop!(
                Watchtower::submit_watchtower_vote(
                    RuntimeOrigin::none(),
                    watchtower_account_1(),
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
fn invalid_voting_period_update_fails() {
    ExtBuilder::build_default().as_externality().execute_with(|| {
        // Try to set period too low (below minimum of 10)
        assert_noop!(
            Watchtower::set_voting_period(RuntimeOrigin::root(), 5u64),
            Error::<TestRuntime>::InvalidVotingPeriod
        );
    });
}

#[test]
fn non_root_voting_period_update_fails() {
    ExtBuilder::build_default().as_externality().execute_with(|| {
        assert_noop!(
            Watchtower::set_voting_period(RuntimeOrigin::signed(watchtower_account_1()), 200u64),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn multiple_summary_instances_work_independently() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
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
                ),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                ethereum_instance,
                root_id.clone(),
                true,
                eth_sig1
            ));

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_2(),
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

            let vote_signature = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
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
        });
}

#[test]
fn ocw_signature_validation_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let account = watchtower_account_1();
            let signing_key = MockNodeManager::get_node_signing_key(&account).unwrap();
            let data = b"test data";

            let signature = signing_key.sign(&data.encode()).unwrap();

            assert!(Watchtower::offchain_signature_is_valid(data, &signing_key, &signature));

            let wrong_data = b"wrong data";
            assert!(!Watchtower::offchain_signature_is_valid(wrong_data, &signing_key, &signature));
        });
}

#[test]
fn voting_status_query_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;

            assert!(Watchtower::get_voting_status(instance, root_id.clone()).is_none());

            let signature = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                instance,
                root_id.clone(),
                true,
                signature
            ));

            let status = Watchtower::get_voting_status(instance, root_id).unwrap();
            assert_eq!(status.2, 1); // vote count should be 1
        });
}

#[test]
fn votes_submitted_after_voting_period_expires_fail() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;

            assert_ok!(Watchtower::set_voting_period(RuntimeOrigin::root(), 10u64));

            let signature1 = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                instance,
                root_id.clone(),
                true,
                signature1
            ));

            roll_forward(15);

            let signature2 = create_test_signature(
                &watchtower_account_2(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_noop!(
                Watchtower::submit_watchtower_vote(
                    RuntimeOrigin::none(),
                    watchtower_account_2(),
                    instance,
                    root_id,
                    true,
                    signature2
                ),
                Error::<TestRuntime>::VotingPeriodExpired
            );
        });
}

#[test]
fn cleanup_expired_votes_functionality_works() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;

            assert_ok!(Watchtower::set_voting_period(RuntimeOrigin::root(), 10u64));

            let signature = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                instance,
                root_id.clone(),
                true,
                signature
            ));

            assert!(Watchtower::is_voting_active(instance, root_id.clone()));

            let votes = Watchtower::individual_votes(instance, &root_id);
            assert_eq!(votes.len(), 1);

            roll_forward(15);

            assert_ok!(Watchtower::cleanup_expired_votes(instance, root_id.clone()));

            let votes_after = Watchtower::individual_votes(instance, &root_id);
            assert_eq!(votes_after.len(), 0);

            assert!(!Watchtower::is_voting_active(instance, root_id.clone()));

            let consensus_key = (instance, root_id);
            assert!(Watchtower::voting_start_block(&consensus_key).is_none());
        });
}

#[test]
fn cleanup_expired_votes_fails_when_voting_not_started() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;

            assert_noop!(
                Watchtower::cleanup_expired_votes(instance, root_id),
                Error::<TestRuntime>::VotingNotStarted
            );
        });
}

#[test]
fn consensus_attempts_after_expiration_fail() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;

            assert_ok!(Watchtower::set_voting_period(RuntimeOrigin::root(), 10u64));

            let signature1 = create_test_signature(
                &watchtower_account_1(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_1(),
                instance,
                root_id.clone(),
                true,
                signature1
            ));

            roll_forward(15);

            let result = Watchtower::try_reach_consensus(instance, root_id.clone());

            assert_noop!(result, Error::<TestRuntime>::VotingPeriodExpired);

            let votes = Watchtower::individual_votes(instance, &root_id);
            assert_eq!(votes.len(), 0);

            let consensus_key = (instance, root_id);
            assert!(Watchtower::voting_start_block(&consensus_key).is_none());
        });
}

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
                sp_runtime::DispatchError::Other("InvalidRootHash")
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
                instance,
                root_id.clone(),
                true,
                signature1
            ));

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_2(),
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
        });
}

#[test]
fn consensus_reached_with_challenges_notifies_admin() {
    ExtBuilder::build_default()
        .with_watchtowers()
        .as_externality()
        .execute_with(|| {
            let root_id = get_test_root_id();
            let instance = SummarySourceInstance::EthereumBridge;
            let (incorrect_root, correct_root) = get_test_challenge_data();

            assert_ok!(Watchtower::notify_summary_ready_for_validation(
                instance,
                root_id.clone(),
                get_test_onchain_hash()
            ));

            let challenge_signature = create_test_signature(
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
                challenge_signature
            ));

            let vote_signature1 = create_test_signature(
                &watchtower_account_2(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );
            let vote_signature2 = create_test_signature(
                &watchtower_account_3(),
                &(crate::WATCHTOWER_OCW_CONTEXT, &instance, &root_id, true),
            );

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_2(),
                instance,
                root_id.clone(),
                true,
                vote_signature1
            ));

            assert_ok!(Watchtower::submit_watchtower_vote(
                RuntimeOrigin::none(),
                watchtower_account_3(),
                instance,
                root_id.clone(),
                true,
                vote_signature2
            ));

            let events = System::events();
            assert!(events.iter().any(|record| {
                matches!(
                    record.event,
                    RuntimeEvent::Watchtower(crate::Event::WatchtowerConsensusReached {
                        summary_instance: i,
                        root_id: ref r,
                        consensus_result: crate::WatchtowerSummaryStatus::Accepted
                    }) if i == instance && r == &root_id
                )
            }));

            assert!(events.iter().any(|record| {
                matches!(
                    record.event,
                    RuntimeEvent::Watchtower(crate::Event::ChallengesPresentedToAdmin {
                        summary_instance: i,
                        root_id: ref r,
                        challenge_count: 1,
                        trigger: crate::ChallengeAdminTrigger::ConsensusReached
                    }) if i == instance && r == &root_id
                )
            }));
        });
}
