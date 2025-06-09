#![cfg(test)]

use crate::{
    mock::{
        admin_account, alice, gas_fee_recipient, ExtBuilder, PalletConfig, RuntimeOrigin, System,
        TestRuntime,
    },
    BaseGasFee, Error, Event, GasFeeRecipientAccount,
};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_runtime::DispatchError;

use crate::AdminAccount;

mod set_base_gas_fee {
    use super::*;

    #[test]
    fn succeeds() {
        ExtBuilder::default().build().as_externality().execute_with(|| {
            let fee = 11111u128;
            assert!(<BaseGasFee<TestRuntime>>::get() != fee);

            assert_ok!(PalletConfig::set_base_gas_fee(RuntimeOrigin::signed(admin_account()), fee));

            System::assert_last_event(Event::BaseGasFeeSet { new_base_gas_fee: fee }.into());
        });
    }

    #[test]
    fn fee_cannot_be_zero() {
        ExtBuilder::default().build().as_externality().execute_with(|| {
            let fee = 0u128;
            assert_noop!(
                PalletConfig::set_base_gas_fee(RuntimeOrigin::signed(admin_account()), fee),
                Error::<TestRuntime>::BaseGasFeeZero
            );
        });
    }

    #[test]
    fn origin_is_checked_none() {
        ExtBuilder::default().build().as_externality().execute_with(|| {
            let fee = 1234567890u128;
            assert!(<BaseGasFee<TestRuntime>>::get() != fee);

            assert_noop!(
                PalletConfig::set_base_gas_fee(RawOrigin::None.into(), fee),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn origin_is_checked_signed() {
        ExtBuilder::default().build().as_externality().execute_with(|| {
            let fee = 1234567890u128;
            let bad_origin = alice();
            assert_noop!(
                PalletConfig::set_base_gas_fee(RuntimeOrigin::signed(bad_origin), fee),
                Error::<TestRuntime>::SenderNotAdmin
            );
        });
    }
}

mod set_gas_fee_recipient {
    use super::*;

    #[test]
    fn succeeds() {
        ExtBuilder::default().build().as_externality().execute_with(|| {
            let account = alice();
            assert!(<GasFeeRecipientAccount<TestRuntime>>::get() != Some(account));

            assert_ok!(PalletConfig::set_gas_fee_recipient(
                RuntimeOrigin::signed(admin_account()),
                account
            ));

            System::assert_last_event(Event::GasFeeRecipientSet { new_account: account }.into());
        });
    }

    #[test]
    fn origin_is_checked_none() {
        ExtBuilder::default().build().as_externality().execute_with(|| {
            let account = alice();

            assert_noop!(
                PalletConfig::set_gas_fee_recipient(RawOrigin::None.into(), account),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn origin_is_checked_signed() {
        ExtBuilder::default().build().as_externality().execute_with(|| {
            let account = gas_fee_recipient();

            let bad_origin = alice();
            assert_noop!(
                PalletConfig::set_gas_fee_recipient(RuntimeOrigin::signed(bad_origin), account),
                Error::<TestRuntime>::SenderNotAdmin
            );
        });
    }
}

mod set_admin_account {
    use super::*;

    #[test]
    fn succeeds() {
        ExtBuilder::default().build().as_externality().execute_with(|| {
            let account = alice();
            assert!(<AdminAccount<TestRuntime>>::get() != Some(account));

            assert_ok!(PalletConfig::set_admin_account(RuntimeOrigin::root(), account));

            System::assert_last_event(Event::AdminAccountSet { new_admin: account }.into());
        });
    }

    #[test]
    fn origin_is_checked_none() {
        ExtBuilder::default().build().as_externality().execute_with(|| {
            let account = alice();

            assert_noop!(
                PalletConfig::set_admin_account(RawOrigin::None.into(), account),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn origin_is_checked_signed() {
        ExtBuilder::default().build().as_externality().execute_with(|| {
            let account = gas_fee_recipient();

            let bad_origin = alice();
            assert_noop!(
                PalletConfig::set_admin_account(RuntimeOrigin::signed(bad_origin), account),
                DispatchError::BadOrigin
            );
        });
    }
}
