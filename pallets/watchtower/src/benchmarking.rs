// Copyright 2025 Truth Network.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::{EventRecord, RawOrigin};
use sp_avn_common::Proof;
use sp_core::crypto::DEV_PHRASE;
use sp_runtime::{traits::Hash, SaturatedConversion};
benchmarks! {

    set_admin_config_voting {
        let new_period: BlockNumberFor<T> = 36u32.into();
        let config = AdminConfig::MinVotingPeriod(new_period);
    }: set_admin_config(RawOrigin::Root, config)
    verify {
        assert!(<MinVotingPeriod<T>>::get() == new_period);
    }

}
impl_benchmark_test_suite!(
    Pallet,
    crate::mock::ExtBuilder::build_default().as_externality(),
    crate::mock::TestRuntime,
);
