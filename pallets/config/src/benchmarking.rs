//! # Pallet config benchmarks
// Copyright 2025 Truth Network.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_system::{EventRecord, RawOrigin};

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len().saturating_sub(1 as usize)];
    assert_eq!(event, &system_event);
}

fn set_admin<T: Config>(admin_account: T::AccountId) {
    AdminAccount::<T>::put(admin_account);
}

benchmarks! {
    set_admin_account {
        let admin_account: T::AccountId = account("WhitelistedAcc", 0, 0);
    }: _(RawOrigin::Root, admin_account.clone())
    verify {
        assert_eq!(AdminAccount::<T>::get(), Some(admin_account.clone()));
        assert_last_event::<T>(
            Event::AdminAccountSet { new_admin: admin_account }
        .into());
    }

    set_base_gas_fee {
        let admin: T::AccountId = whitelisted_caller();
        set_admin::<T>(admin.clone());
        let fee = 112233u128;
    }: _(RawOrigin::Signed(admin), fee.clone())
    verify {
        assert_eq!(BaseGasFee::<T>::get(), fee.clone());
        assert_last_event::<T>(
            Event::BaseGasFeeSet { new_base_gas_fee: fee }
        .into());
    }

    set_gas_fee_recipient {
        let admin: T::AccountId = whitelisted_caller();
        set_admin::<T>(admin.clone());
        let whitelisted_account: T::AccountId = account("WhitelistedAcc", 0, 0);
    }: _(RawOrigin::Signed(admin), whitelisted_account.clone())
    verify {
        assert_eq!(GasFeeRecipientAccount::<T>::get(), Some(whitelisted_account.clone()));
        assert_last_event::<T>(
            Event::GasFeeRecipientSet { new_account: whitelisted_account }
        .into());
    }
}

impl_benchmark_test_suite!(
    Pallet,
    crate::mock::ExtBuilder::default().build().as_externality(),
    crate::mock::TestRuntime,
);
