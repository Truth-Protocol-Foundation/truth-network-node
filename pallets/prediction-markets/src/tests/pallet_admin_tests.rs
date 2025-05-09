//Copyright 2025 Truth Network.

#![cfg(test)]

use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_runtime::DispatchError;

mod whitelist_market_creator {
    use super::*;

    #[test]
    fn succeeds() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let creator = charlie();
            assert!(!<WhitelistedMarketCreators<Runtime>>::contains_key(&creator));

            assert_ok!(PredictionMarkets::whitelist_market_creator(
                RuntimeOrigin::signed(market_admin()),
                creator
            ));

            System::assert_last_event(
                Event::MarketCreatorAdded { whitelisted_account: creator }.into(),
            );
        });
    }

    #[test]
    fn origin_is_checked_none() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let creator = charlie();
            assert!(!<WhitelistedMarketCreators<Runtime>>::contains_key(&creator));
            assert_noop!(
                PredictionMarkets::whitelist_market_creator(RawOrigin::None.into(), creator),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn origin_is_checked_signed() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let creator = charlie();
            assert!(!<WhitelistedMarketCreators<Runtime>>::contains_key(&creator));

            let bad_origin = alice();
            assert_noop!(
                PredictionMarkets::whitelist_market_creator(
                    RuntimeOrigin::signed(bad_origin),
                    creator
                ),
                Error::<Runtime>::SenderNotMarketAdmin
            );
        });
    }
}

mod remove_market_creator {
    use super::*;

    #[test]
    fn succeeds() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let creator = charlie();
            assert_ok!(PredictionMarkets::whitelist_market_creator(
                RuntimeOrigin::signed(market_admin()),
                creator
            ));

            assert_ok!(PredictionMarkets::remove_market_creator(
                RuntimeOrigin::signed(market_admin()),
                creator
            ));

            System::assert_last_event(
                Event::MarketCreatorRemoved { removed_account: creator }.into(),
            );
        });
    }

    #[test]
    fn origin_is_checked_none() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let creator = charlie();
            assert_ok!(PredictionMarkets::whitelist_market_creator(
                RuntimeOrigin::signed(market_admin()),
                creator
            ));
            assert_noop!(
                PredictionMarkets::remove_market_creator(RawOrigin::None.into(), creator),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn origin_is_checked_signed() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let creator = charlie();
            assert_ok!(PredictionMarkets::whitelist_market_creator(
                RuntimeOrigin::signed(market_admin()),
                creator
            ));

            let bad_origin = alice();
            assert_noop!(
                PredictionMarkets::remove_market_creator(
                    RuntimeOrigin::signed(bad_origin),
                    creator
                ),
                Error::<Runtime>::SenderNotMarketAdmin
            );
        });
    }
}

mod set_winnings_fee_account {
    use super::*;

    #[test]
    fn succeeds() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let account = charlie();
            assert!(<WinningsFeeAccount<Runtime>>::get().is_none());

            assert_ok!(PredictionMarkets::set_winnings_fee_account(
                RuntimeOrigin::signed(market_admin()),
                account
            ));

            System::assert_last_event(Event::WinningsFeeAccountSet { new_account: account }.into());
        });
    }

    #[test]
    fn origin_is_checked_none() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let account = charlie();
            assert!(<WinningsFeeAccount<Runtime>>::get().is_none());
            assert_noop!(
                PredictionMarkets::set_winnings_fee_account(RawOrigin::None.into(), account),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn origin_is_checked_signed() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let account = charlie();
            assert!(<WinningsFeeAccount<Runtime>>::get().is_none());

            let bad_origin = alice();
            assert_noop!(
                PredictionMarkets::set_winnings_fee_account(
                    RuntimeOrigin::signed(bad_origin),
                    account
                ),
                Error::<Runtime>::SenderNotMarketAdmin
            );
        });
    }
}

mod set_additional_swap_fee_account {
    use super::*;

    #[test]
    fn succeeds() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let account = charlie();
            assert!(<AdditionalSwapFeeAccount<Runtime>>::get().is_none());

            assert_ok!(PredictionMarkets::set_additional_swap_fee_account(
                RuntimeOrigin::signed(market_admin()),
                account
            ));

            System::assert_last_event(
                Event::AdditionalSwapFeeAccountSet { new_account: account }.into(),
            );
        });
    }

    #[test]
    fn origin_is_checked_none() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let account = charlie();
            assert!(<AdditionalSwapFeeAccount<Runtime>>::get().is_none());
            assert_noop!(
                PredictionMarkets::set_additional_swap_fee_account(RawOrigin::None.into(), account),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn origin_is_checked_signed() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let account = charlie();
            assert!(<AdditionalSwapFeeAccount<Runtime>>::get().is_none());

            let bad_origin = alice();
            assert_noop!(
                PredictionMarkets::set_additional_swap_fee_account(
                    RuntimeOrigin::signed(bad_origin),
                    account
                ),
                Error::<Runtime>::SenderNotMarketAdmin
            );
        });
    }
}
