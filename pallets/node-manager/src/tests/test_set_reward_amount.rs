//Copyright 2025 Truth Network.

#![cfg(test)]

use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use prediction_market_primitives::test_helper::TestAccount;
use sp_runtime::DispatchError;

fn set_registrar() -> AccountId {
    let registrar = TestAccount::new([1u8; 32]).account_id();
    <NodeRegistrar<TestRuntime>>::set(Some(registrar));
    registrar
}

fn fund_pot(amount: u128) {
    Balances::make_free_balance_be(&NodeManager::compute_reward_account_id(), amount);
}

/// Roll forward until the current reward period ends and return its index
fn end_current_period() -> RewardPeriodIndex {
    let reward_period = <RewardPeriod<TestRuntime>>::get();
    let end = reward_period.first + reward_period.length as u64;
    roll_forward(end.saturating_sub(System::block_number()).max(1));
    assert_eq!(<RewardPeriod<TestRuntime>>::get().current, reward_period.current + 1);
    reward_period.current
}

fn add_uptime(period: RewardPeriodIndex, node: AccountId, count: u64) {
    <NodeUptime<TestRuntime>>::insert(
        period,
        node,
        UptimeInfo { count, last_reported: System::block_number() },
    );
    <TotalUptime<TestRuntime>>::mutate(period, |total| *total = total.saturating_add(count));
}

fn setup() -> sp_io::TestExternalities {
    let mut ext = ExtBuilder::build_default().with_genesis_config().as_externality();
    ext.execute_with(|| Timestamp::set_timestamp(1_000_000));
    ext
}

#[test]
fn a_period_ends_unfunded() {
    setup().execute_with(|| {
        let period = end_current_period();

        let pot = <RewardPot<TestRuntime>>::get(period).unwrap();
        assert!(!pot.funded);
        assert_eq!(pot.total_reward, 0);
        assert_eq!(pot.reward_end_time, NodeManager::time_now_sec());
        assert_eq!(<OutstandingRewardToPay<TestRuntime>>::get(), 0);
    });
}

#[test]
fn rollover_does_not_wait_for_funding() {
    setup().execute_with(|| {
        let first = end_current_period();
        let second = end_current_period();

        assert_eq!(second, first + 1);
        assert!(!<RewardPot<TestRuntime>>::get(first).unwrap().funded);
        assert!(!<RewardPot<TestRuntime>>::get(second).unwrap().funded);
    });
}

#[test]
fn root_can_set_the_amount_of_an_ended_period() {
    setup().execute_with(|| {
        fund_pot(REWARD_AMOUNT);
        let period = end_current_period();

        assert_ok!(NodeManager::set_reward_amount(RawOrigin::Root.into(), period, REWARD_AMOUNT));

        let pot = <RewardPot<TestRuntime>>::get(period).unwrap();
        assert!(pot.funded);
        assert_eq!(pot.total_reward, REWARD_AMOUNT);
        assert_eq!(<OutstandingRewardToPay<TestRuntime>>::get(), REWARD_AMOUNT);
        System::assert_last_event(Event::RewardAmountSet { period, amount: REWARD_AMOUNT }.into());
    });
}

#[test]
fn registrar_can_set_the_amount_of_an_ended_period() {
    setup().execute_with(|| {
        let registrar = set_registrar();
        fund_pot(REWARD_AMOUNT);
        let period = end_current_period();

        assert_ok!(NodeManager::set_reward_amount(
            RuntimeOrigin::signed(registrar),
            period,
            REWARD_AMOUNT
        ));
        assert!(<RewardPot<TestRuntime>>::get(period).unwrap().funded);
    });
}

#[test]
fn zero_amount_can_be_set() {
    setup().execute_with(|| {
        let period = end_current_period();

        assert_ok!(NodeManager::set_reward_amount(RawOrigin::Root.into(), period, 0));

        let pot = <RewardPot<TestRuntime>>::get(period).unwrap();
        assert!(pot.funded);
        assert_eq!(pot.total_reward, 0);
    });
}

#[test]
fn funded_amount_can_be_updated_within_the_window() {
    setup().execute_with(|| {
        fund_pot(REWARD_AMOUNT * 2);
        let period = end_current_period();

        assert_ok!(NodeManager::set_reward_amount(RawOrigin::Root.into(), period, REWARD_AMOUNT));
        advance_time_secs(REWARD_UPDATE_WINDOW_SECS - 1);
        assert_ok!(NodeManager::set_reward_amount(
            RawOrigin::Root.into(),
            period,
            REWARD_AMOUNT * 2
        ));

        assert_eq!(<RewardPot<TestRuntime>>::get(period).unwrap().total_reward, REWARD_AMOUNT * 2);
        // The previous amount is replaced, not added to
        assert_eq!(<OutstandingRewardToPay<TestRuntime>>::get(), REWARD_AMOUNT * 2);
    });
}

#[test]
fn amounts_of_other_periods_count_towards_the_pot_balance() {
    setup().execute_with(|| {
        fund_pot(REWARD_AMOUNT * 2);
        let first = end_current_period();
        let second = end_current_period();

        assert_ok!(NodeManager::set_reward_amount(RawOrigin::Root.into(), first, REWARD_AMOUNT));
        assert_noop!(
            NodeManager::set_reward_amount(RawOrigin::Root.into(), second, REWARD_AMOUNT + 1),
            Error::<TestRuntime>::InsufficientPotBalance
        );
        assert_ok!(NodeManager::set_reward_amount(RawOrigin::Root.into(), second, REWARD_AMOUNT));
        assert_eq!(<OutstandingRewardToPay<TestRuntime>>::get(), REWARD_AMOUNT * 2);
    });
}

#[test]
fn an_unfunded_period_can_be_set_after_the_window() {
    setup().execute_with(|| {
        fund_pot(REWARD_AMOUNT);
        let period = end_current_period();
        advance_time_secs(REWARD_UPDATE_WINDOW_SECS * 10);

        assert_ok!(NodeManager::set_reward_amount(RawOrigin::Root.into(), period, REWARD_AMOUNT));
        assert!(<RewardPot<TestRuntime>>::get(period).unwrap().funded);
    });
}

mod payment {
    use super::*;

    fn pay(period: RewardPeriodIndex) -> DispatchResultWithPostInfo {
        let author = mock::AVN::active_validators()[0].clone();
        let signature = UintAuthorityId(1).sign(&("DummyProof").encode()).expect("Error signing");
        NodeManager::offchain_pay_nodes(RawOrigin::None.into(), period, author, signature)
    }

    fn setup_with_authors() -> sp_io::TestExternalities {
        let mut ext = ExtBuilder::build_default()
            .with_genesis_config()
            .with_authors()
            .as_externality();
        ext.execute_with(|| Timestamp::set_timestamp(1_000_000));
        ext
    }

    #[test]
    fn is_not_possible_before_the_amount_is_set() {
        setup_with_authors().execute_with(|| {
            fund_pot(REWARD_AMOUNT);
            let node = TestAccount::new([7u8; 32]).account_id();
            add_uptime(<RewardPeriod<TestRuntime>>::get().current, node, 1);
            let period = end_current_period();
            advance_time_secs(REWARD_UPDATE_WINDOW_SECS * 10);

            assert_noop!(pay(period), Error::<TestRuntime>::RewardPeriodNotFunded);
            assert!(!NodeManager::can_trigger_payment().unwrap());
        });
    }

    #[test]
    fn is_not_possible_while_the_update_window_is_open() {
        setup_with_authors().execute_with(|| {
            fund_pot(REWARD_AMOUNT);
            let node = TestAccount::new([7u8; 32]).account_id();
            add_uptime(<RewardPeriod<TestRuntime>>::get().current, node, 1);
            let period = end_current_period();

            assert_ok!(NodeManager::set_reward_amount(
                RawOrigin::Root.into(),
                period,
                REWARD_AMOUNT
            ));
            advance_time_secs(REWARD_UPDATE_WINDOW_SECS - 1);

            assert_noop!(pay(period), Error::<TestRuntime>::RewardUpdateWindowOpen);
            assert!(!NodeManager::can_trigger_payment().unwrap());

            // Once the window closes the period can be paid
            advance_time_secs(1);
            assert!(NodeManager::can_trigger_payment().unwrap());
        });
    }

    #[test]
    fn a_zero_reward_period_completes_without_paying() {
        setup_with_authors().execute_with(|| {
            let owner = TestAccount::new([209u8; 32]).account_id();
            let registrar = set_registrar();
            let node = TestAccount::new([7u8; 32]).account_id();
            assert_ok!(NodeManager::register_node(
                RuntimeOrigin::signed(registrar),
                node,
                owner,
                UintAuthorityId(7),
            ));
            add_uptime(<RewardPeriod<TestRuntime>>::get().current, node, 1);
            let period = end_current_period();

            fund_reward_period(period, 0);
            assert_ok!(pay(period));

            assert_eq!(Balances::free_balance(&owner), 0);
            assert!(<RewardPot<TestRuntime>>::get(period).is_none());
            assert!(<NodeUptime<TestRuntime>>::iter_prefix(period).next().is_none());
            assert_eq!(<OldestUnpaidRewardPeriodIndex<TestRuntime>>::get(), period + 1);
        });
    }

    #[test]
    fn completing_a_period_releases_its_outstanding_amount() {
        setup_with_authors().execute_with(|| {
            fund_pot(REWARD_AMOUNT);
            let period = end_current_period();

            // No uptime, so the whole amount is released
            fund_reward_period(period, REWARD_AMOUNT);
            assert_eq!(<OutstandingRewardToPay<TestRuntime>>::get(), REWARD_AMOUNT);
            assert_ok!(pay(period));

            assert_eq!(<OutstandingRewardToPay<TestRuntime>>::get(), 0);
            assert_eq!(
                Balances::free_balance(&NodeManager::compute_reward_account_id()),
                REWARD_AMOUNT
            );
            assert!(<RewardPot<TestRuntime>>::get(period).is_none());
        });
    }
}

mod fails_to_be_set_when {
    use super::*;

    #[test]
    fn the_period_has_not_ended_yet() {
        setup().execute_with(|| {
            fund_pot(REWARD_AMOUNT);
            let current = <RewardPeriod<TestRuntime>>::get().current;
            assert_noop!(
                NodeManager::set_reward_amount(RawOrigin::Root.into(), current, REWARD_AMOUNT),
                Error::<TestRuntime>::RewardPotNotFound
            );
        });
    }

    #[test]
    fn the_period_is_funded_and_its_update_window_has_closed() {
        setup().execute_with(|| {
            fund_pot(REWARD_AMOUNT * 2);
            let period = end_current_period();
            fund_reward_period(period, REWARD_AMOUNT);

            assert_noop!(
                NodeManager::set_reward_amount(RawOrigin::Root.into(), period, REWARD_AMOUNT * 2),
                Error::<TestRuntime>::RewardUpdateWindowClosed
            );
        });
    }

    #[test]
    fn amount_exceeds_the_per_period_cap() {
        setup().execute_with(|| {
            let max = MaxRewardPerPeriod::get();
            fund_pot(max * 2);
            let period = end_current_period();

            assert_noop!(
                NodeManager::set_reward_amount(RawOrigin::Root.into(), period, max + 1),
                Error::<TestRuntime>::RewardExceedsMax
            );
            assert_ok!(NodeManager::set_reward_amount(RawOrigin::Root.into(), period, max));
        });
    }

    #[test]
    fn the_pot_does_not_have_enough_balance() {
        setup().execute_with(|| {
            fund_pot(REWARD_AMOUNT - 1);
            let period = end_current_period();

            assert_noop!(
                NodeManager::set_reward_amount(RawOrigin::Root.into(), period, REWARD_AMOUNT),
                Error::<TestRuntime>::InsufficientPotBalance
            );
        });
    }

    #[test]
    fn origin_is_an_unauthorised_signed_account() {
        setup().execute_with(|| {
            set_registrar();
            fund_pot(REWARD_AMOUNT);
            let period = end_current_period();
            let other = TestAccount::new([2u8; 32]).account_id();

            assert_noop!(
                NodeManager::set_reward_amount(RuntimeOrigin::signed(other), period, REWARD_AMOUNT),
                Error::<TestRuntime>::OriginNotRegistrar
            );
        });
    }

    #[test]
    fn origin_is_signed_and_no_registrar_is_set() {
        setup().execute_with(|| {
            fund_pot(REWARD_AMOUNT);
            let period = end_current_period();
            let other = TestAccount::new([2u8; 32]).account_id();

            assert_noop!(
                NodeManager::set_reward_amount(RuntimeOrigin::signed(other), period, REWARD_AMOUNT),
                Error::<TestRuntime>::RegistrarNotSet
            );
        });
    }

    #[test]
    fn origin_is_none() {
        setup().execute_with(|| {
            let period = end_current_period();
            assert_noop!(
                NodeManager::set_reward_amount(RawOrigin::None.into(), period, REWARD_AMOUNT),
                DispatchError::BadOrigin
            );
        });
    }
}
