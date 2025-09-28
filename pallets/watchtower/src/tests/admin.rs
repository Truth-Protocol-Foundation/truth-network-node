//Copyright 2025 Truth Network.

#![cfg(test)]

use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_runtime::DispatchError;

#[test]
fn origin_is_checked_none() {
    let mut ext = ExtBuilder::build_default().as_externality();
    ext.execute_with(|| {
        let current_period = MinVotingPeriod::<TestRuntime>::get();
        let new_period = current_period + 1;

        let config = AdminConfig::MinVotingPeriod(new_period);
        assert_noop!(
            Watchtower::set_admin_config(RawOrigin::None.into(), config,),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn origin_is_checked_signed() {
    let mut ext = ExtBuilder::build_default().as_externality();
    ext.execute_with(|| {
        let current_period = MinVotingPeriod::<TestRuntime>::get();
        let new_period = current_period + 1;

        let config = AdminConfig::MinVotingPeriod(new_period);
        let bad_signer = TestAccount::new([99u8; 32]).account_id();
        assert_noop!(
            Watchtower::set_admin_config(RuntimeOrigin::signed(bad_signer.clone()), config,),
            DispatchError::BadOrigin
        );
    });
}

mod min_voting_period {
    use super::*;

    #[test]
    fn min_voting_period_can_be_set() {
        let mut ext = ExtBuilder::build_default().as_externality();
        ext.execute_with(|| {
            let current_period = MinVotingPeriod::<TestRuntime>::get();
            let new_period = current_period + 1;

            let config = AdminConfig::MinVotingPeriod(new_period);
            assert_ok!(Watchtower::set_admin_config(RawOrigin::Root.into(), config,));
            System::assert_last_event(Event::MinVotingPeriodSet { new_period }.into());
        });
    }
}