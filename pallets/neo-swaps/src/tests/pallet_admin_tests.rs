//Copyright 2025 Truth Network.

#![cfg(test)]

use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_runtime::DispatchError;

mod set_additional_swap_fee {
    use super::*;

    #[test]
    fn succeeds() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let fee = 123_456;
            assert!(<AdditionalSwapFee<Runtime>>::get() != Some(fee));
            assert_ok!(NeoSwaps::set_additional_swap_fee(
                RuntimeOrigin::signed(market_admin()),
                fee
            ));

            System::assert_last_event(Event::AdditionalSwapFeeSet { new_fee: fee }.into());
        });
    }

    #[test]
    fn origin_is_checked_none() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let fee = 123_456;
            assert!(<AdditionalSwapFee<Runtime>>::get() != Some(fee));
            assert_noop!(
                NeoSwaps::set_additional_swap_fee(RawOrigin::None.into(), fee),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn origin_is_checked_signed() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let fee = 123_456;
            assert!(<AdditionalSwapFee<Runtime>>::get() != Some(fee));

            let bad_origin = alice();
            assert_noop!(
                NeoSwaps::set_additional_swap_fee(RuntimeOrigin::signed(bad_origin), fee),
                Error::<Runtime>::SenderNotMarketAdmin
            );
        });
    }
}

mod set_early_exit_fee_account {
    use super::*;

    #[test]
    fn succeeds() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let account = charlie();
            assert!(<EarlyExitFeeAccount<Runtime>>::get() != Some(account));

            assert_ok!(NeoSwaps::set_early_exit_fee_account(
                RuntimeOrigin::signed(market_admin()),
                account
            ));

            System::assert_last_event(
                Event::EarlyExitFeeAccountSet { new_account: account }.into(),
            );
        });
    }

    #[test]
    fn origin_is_checked_none() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let account = charlie();
            assert!(<EarlyExitFeeAccount<Runtime>>::get() != Some(account));
            assert_noop!(
                NeoSwaps::set_early_exit_fee_account(RawOrigin::None.into(), account),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn origin_is_checked_signed() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let account = charlie();
            assert!(<EarlyExitFeeAccount<Runtime>>::get() != Some(account));

            let bad_origin = alice();
            assert_noop!(
                NeoSwaps::set_early_exit_fee_account(RuntimeOrigin::signed(bad_origin), account),
                Error::<Runtime>::SenderNotMarketAdmin
            );
        });
    }
}
