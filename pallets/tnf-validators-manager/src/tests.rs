//Copyright 2024 Aventus Network Services (UK) Ltd.

#![cfg(test)]

use crate::{mock::*, AVN, *};
use frame_support::{assert_noop, assert_ok, traits::Currency};
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_runtime::{testing::UintAuthorityId, traits::BadOrigin};
use substrate_test_utils::assert_eq_uvec;

fn register_validator(
    collator_id: &AccountId,
    collator_eth_public_key: &ecdsa::Public,
) -> DispatchResult {
    return TnfValidatorsManager::add_collator(
        RawOrigin::Root.into(),
        *collator_id,
        *collator_eth_public_key,
    )
}

fn set_session_keys(collator_id: &AccountId) {
    pallet_session::NextKeys::<TestRuntime>::insert::<AccountId, UintAuthorityId>(
        *collator_id,
        UintAuthorityId(10u64).into(),
    );
}

fn force_add_collator(
    collator_id: &AccountId,
    collator_eth_public_key: &ecdsa::Public,
) -> DispatchResult {
    set_session_keys(collator_id);
    assert_ok!(register_validator(collator_id, collator_eth_public_key));

    //Advance 2 session to add the collator to the session
    advance_session();
    advance_session();

    Ok(())
}

#[test]
fn test_register_existing_validator() {
    let mut ext = ExtBuilder::build_default().with_validators().as_externality();
    ext.execute_with(|| {
        let mock_data = MockData::setup_valid();
        TnfValidatorsManager::insert_to_validators(&mock_data.new_validator_id);

        let current_num_events = System::events().len();

        //Set the session keys of the new validator we are trying to register
        set_session_keys(&mock_data.new_validator_id);

        assert_noop!(
            register_validator(&mock_data.new_validator_id, &mock_data.collator_eth_public_key),
            Error::<TestRuntime>::ValidatorAlreadyExists
        );

        // No Event has been deposited
        assert_eq!(System::events().len(), current_num_events);
    });
}

#[test]
fn test_register_validator_with_no_validators() {
    let mut ext = ExtBuilder::build_default().as_externality();
    ext.execute_with(|| {
        let mock_data = MockData::setup_valid();
        let current_num_events = System::events().len();

        //Set the session keys of the new validator we are trying to register
        set_session_keys(&mock_data.new_validator_id);

        assert_noop!(
            register_validator(&mock_data.new_validator_id, &mock_data.collator_eth_public_key),
            Error::<TestRuntime>::NoValidators
        );

        // no Event has been deposited
        assert_eq!(System::events().len(), current_num_events);
    });
}

mod register_validator {
    use super::*;

    // TODO move MockData here and rename to Context

    fn run_preconditions(context: &MockData) {
        assert_eq!(0, ValidatorActions::<TestRuntime>::iter().count());
        let validator_account_ids =
            TnfValidatorsManager::validator_account_ids().expect("Should contain validators");
        assert_eq!(false, validator_account_ids.contains(&context.new_validator_id));
        assert_eq!(
            false,
            TnfValidatorsManager::get_ethereum_public_key_if_exists(&context.new_validator_id)
                .is_some()
        );
    }

    fn find_validator_activation_action(data: &MockData, status: ValidatorsActionStatus) -> bool {
        return ValidatorActions::<TestRuntime>::iter().any(|(account_id, _ingress, action_data)| {
            action_data.status == status &&
                action_data.action_type == ValidatorsActionType::Activation &&
                account_id == data.new_validator_id
        })
    }

    mod succeeds {
        use super::*;

        #[test]
        fn and_adds_validator() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = MockData::setup_valid();
                run_preconditions(&context);

                //set the session keys of the new validator we are trying to register
                set_session_keys(&context.new_validator_id);

                // Result OK
                assert_ok!(register_validator(
                    &context.new_validator_id,
                    &context.collator_eth_public_key
                ));
                // Upon completion validator has been added ValidatorAccountIds storage
                assert!(TnfValidatorsManager::validator_account_ids()
                    .unwrap()
                    .iter()
                    .any(|a| a == &context.new_validator_id));
                // ValidatorRegistered Event has been deposited
                assert_eq!(
                    true,
                    System::events().iter().any(|a| a.event ==
                        mock::RuntimeEvent::TnfValidatorsManager(
                            crate::Event::<TestRuntime>::ValidatorRegistered {
                                validator_id: context.new_validator_id,
                                eth_key: context.collator_eth_public_key.clone()
                            }
                        ))
                );
                // ValidatorActivationStarted Event has not been deposited yet
                assert_eq!(
                    false,
                    System::events().iter().any(|a| a.event ==
                        mock::RuntimeEvent::TnfValidatorsManager(
                            crate::Event::<TestRuntime>::ValidatorActivationStarted {
                                validator_id: context.new_validator_id
                            }
                        ))
                );
                // But the activation action has been triggered
                assert_eq!(
                    true,
                    find_validator_activation_action(
                        &context,
                        ValidatorsActionStatus::AwaitingConfirmation
                    )
                );
            });
        }

        #[test]
        fn activation_dispatches_after_two_sessions() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = MockData::setup_valid();
                run_preconditions(&context);

                //Set the session keys of the new validator we are trying to register
                set_session_keys(&context.new_validator_id);

                assert_ok!(register_validator(
                    &context.new_validator_id,
                    &context.collator_eth_public_key
                ));

                // It takes 2 session for validators to be updated
                advance_session();
                advance_session();

                // The activation action has been sent
                assert_eq!(
                    true,
                    find_validator_activation_action(&context, ValidatorsActionStatus::Confirmed)
                );
                // ValidatorActivationStarted Event has been deposited
                assert_eq!(
                    true,
                    System::events().iter().any(|a| a.event ==
                        mock::RuntimeEvent::TnfValidatorsManager(
                            crate::Event::<TestRuntime>::ValidatorActivationStarted {
                                validator_id: context.new_validator_id
                            }
                        ))
                );
            });
        }
    }
}

// Change these tests to accomodate the use of votes
#[allow(non_fmt_panics)]
mod remove_validator_public {
    use super::*;

    // Tests for pub fn remove_validator(origin) -> DispatchResult {...}
    #[test]
    fn valid_case() {
        let mut ext = ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(|| {
            let context = MockData::setup_valid();
            assert_ok!(force_add_collator(
                &context.new_validator_id,
                &context.collator_eth_public_key
            ));

            //Prove this is an existing validator
            assert_eq_uvec!(
                Session::validators(),
                vec![
                    validator_id_1(),
                    validator_id_2(),
                    validator_id_3(),
                    validator_id_4(),
                    validator_id_5(),
                    context.new_validator_id
                ]
            );

            //Validator exists in the AVN
            assert_eq!(AVN::<TestRuntime>::is_validator(&context.new_validator_id), true);

            //Remove the validator
            assert_ok!(TnfValidatorsManager::remove_validator(
                RawOrigin::Root.into(),
                context.new_validator_id
            ));

            //Event emitted as expected
            assert!(System::events().iter().any(|a| a.event ==
                mock::RuntimeEvent::TnfValidatorsManager(
                    crate::Event::<TestRuntime>::ValidatorDeregistered {
                        validator_id: context.new_validator_id
                    }
                )));

            //Validator removed from validators manager
            assert_eq!(
                TnfValidatorsManager::validator_account_ids()
                    .unwrap()
                    .iter()
                    .position(|&x| x == context.new_validator_id),
                None
            );

            //Validator is still in the session. Will be removed after 1 era.
            assert_eq_uvec!(
                Session::validators(),
                vec![
                    validator_id_1(),
                    validator_id_2(),
                    validator_id_3(),
                    validator_id_4(),
                    validator_id_5(),
                    context.new_validator_id
                ]
            );

            // Advance 2 sessions
            advance_session();
            advance_session();

            // Validator has been removed from the session
            assert_eq_uvec!(
                Session::validators(),
                vec![
                    validator_id_1(),
                    validator_id_2(),
                    validator_id_3(),
                    validator_id_4(),
                    validator_id_5()
                ]
            );

            //Validator is also removed from the AVN
            assert_eq!(AVN::<TestRuntime>::is_validator(&context.new_validator_id), false);
        });
    }

    #[test]
    fn fails_when_regular_sender_submits_transaction() {
        let mut ext = ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(|| {
            let context = MockData::setup_valid();
            assert_ok!(force_add_collator(
                &context.new_validator_id,
                &context.collator_eth_public_key
            ));

            let num_events = System::events().len();
            assert_noop!(
                TnfValidatorsManager::remove_validator(
                    RuntimeOrigin::signed(validator_id_3()),
                    validator_id_3()
                ),
                BadOrigin
            );
            assert_eq!(System::events().len(), num_events);
        });
    }

    #[test]
    fn unsigned_sender() {
        let mut ext = ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(|| {
            let context = MockData::setup_valid();
            assert_ok!(force_add_collator(
                &context.new_validator_id,
                &context.collator_eth_public_key
            ));

            let num_events = System::events().len();
            assert_noop!(
                TnfValidatorsManager::remove_validator(
                    RawOrigin::None.into(),
                    context.new_validator_id
                ),
                BadOrigin
            );
            assert_eq!(System::events().len(), num_events);
        });
    }

    #[test]
    fn non_validator() {
        let mut ext = ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(|| {
            //Ensure we have enough candidates
            let context = MockData::setup_valid();
            assert_ok!(force_add_collator(
                &context.new_validator_id,
                &context.collator_eth_public_key
            ));

            let original_validators = TnfValidatorsManager::validator_account_ids();
            let num_events = System::events().len();

            // Caller of remove function has to emit event if removal is successful.
            assert_eq!(System::events().len(), num_events);
            assert_eq!(TnfValidatorsManager::validator_account_ids(), original_validators);
        });
    }
}

#[test]
fn test_initial_validators_populated_from_genesis_config() {
    let mut ext = ExtBuilder::build_default().with_validators().as_externality();
    ext.execute_with(|| {
        assert_eq!(
            TnfValidatorsManager::validator_account_ids().unwrap(),
            genesis_config_initial_validators().to_vec()
        );
    });
}

mod add_validator {
    use super::*;

    struct AddValidatorContext {
        collator: AccountId,
        collator_eth_public_key: ecdsa::Public,
    }

    impl Default for AddValidatorContext {
        fn default() -> Self {
            let collator = TestAccount::new([0u8; 32]).account_id();
            Balances::make_free_balance_be(&collator, 100000);

            AddValidatorContext {
                collator,
                collator_eth_public_key: ecdsa::Public::from_raw(hex!(
                    "02407b0d9f41148bbe3b6c7d4a62585ae66cc32a707441197fa5453abfebd31d57"
                )),
            }
        }
    }

    #[test]
    fn succeeds_with_good_parameters() {
        let mut ext = ExtBuilder::build_default().with_validators().as_externality();
        ext.execute_with(|| {
            let context = &AddValidatorContext::default();

            set_session_keys(&context.collator);
            assert_ok!(register_validator(&context.collator, &context.collator_eth_public_key));

            assert_eq!(
                true,
                TnfValidatorsManager::validator_account_ids()
                    .unwrap()
                    .contains(&context.collator)
            );
            assert_eq!(
                TnfValidatorsManager::get_validator_by_eth_public_key(
                    context.collator_eth_public_key.clone()
                )
                .unwrap(),
                context.collator
            );
        });
    }

    mod fails_when {
        use super::*;

        #[test]
        fn extrinsic_is_unsigned() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &AddValidatorContext::default();

                set_session_keys(&context.collator);
                assert_noop!(
                    TnfValidatorsManager::add_collator(
                        RawOrigin::None.into(),
                        context.collator,
                        context.collator_eth_public_key,
                    ),
                    BadOrigin
                );
            });
        }

        #[test]
        fn no_validators() {
            let mut ext = ExtBuilder::build_default().as_externality();
            ext.execute_with(|| {
                // This test is simulating "no validators" by not using validators when building the
                // test extension
                let context = &AddValidatorContext::default();

                set_session_keys(&context.collator);
                assert_noop!(
                    register_validator(&context.collator, &context.collator_eth_public_key),
                    Error::<TestRuntime>::NoValidators
                );
            });
        }

        #[test]
        fn validator_eth_key_already_exists() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &AddValidatorContext::default();

                set_session_keys(&context.collator);
                <EthereumPublicKeys<TestRuntime>>::insert(
                    context.collator_eth_public_key.clone(),
                    context.collator,
                );

                assert_noop!(
                    register_validator(&context.collator, &context.collator_eth_public_key),
                    Error::<TestRuntime>::ValidatorEthKeyAlreadyExists
                );
            });
        }

        #[test]
        fn validator_already_exists() {
            let mut ext = ExtBuilder::build_default().with_validators().as_externality();
            ext.execute_with(|| {
                let context = &AddValidatorContext::default();

                set_session_keys(&context.collator);
                assert_ok!(<ValidatorAccountIds::<TestRuntime>>::try_append(&context.collator));

                assert_noop!(
                    register_validator(&context.collator, &context.collator_eth_public_key),
                    Error::<TestRuntime>::ValidatorAlreadyExists
                );
            });
        }

        #[test]
        fn maximum_collators_is_reached() {
            let mut ext = ExtBuilder::build_default().with_maximum_validators().as_externality();
            ext.execute_with(|| {
                let context = &AddValidatorContext::default();

                set_session_keys(&context.collator);
                assert_noop!(
                    register_validator(&context.collator, &context.collator_eth_public_key),
                    Error::<TestRuntime>::MaximumValidatorsReached
                );
            });
        }
    }
}
