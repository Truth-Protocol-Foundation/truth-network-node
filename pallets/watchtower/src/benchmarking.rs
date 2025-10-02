// Copyright 2025 Truth Network.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::{EventRecord, RawOrigin};
use sp_avn_common::Proof;
use sp_core::crypto::DEV_PHRASE;
use sp_runtime::{traits::Hash, SaturatedConversion};

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len().saturating_sub(1 as usize)];
    assert_eq!(event, &system_event);
}

fn create_proposal_request<T: Config>(
    external_ref_id: u32,
    created_at: u32,
    is_internal: bool,
) -> ProposalRequest {
    let external_ref: T::Hash = T::Hashing::hash_of(&external_ref_id);
    let source: ProposalSource;

    if is_internal {
        source = ProposalSource::Internal(ProposalType::Governance);
    } else {
        source = ProposalSource::External;
    };

    ProposalRequest {
        title: "Dummy Proposal".as_bytes().to_vec(),
        external_ref: H256::from_slice(&external_ref.as_ref()),
        threshold: Perbill::from_percent(50),
        payload: RawPayload::Uri(external_ref_id.encode()),
        source,
        decision_rule: DecisionRule::SimpleMajority,
        created_at,
        vote_duration: Some(MinVotingPeriod::<T>::get().saturated_into::<u32>() + 1u32),
    }
}
benchmarks! {
    submit_external_proposal {
        let signer: T::AccountId = account("signer", 0, 0);
        let proposal_request = create_proposal_request::<T>(1, 1u32, false);
        let external_ref = proposal_request.external_ref;
    }: submit_external_proposal(RawOrigin::Signed(signer), proposal_request)
    verify {
        assert!(ExternalRef::<T>::contains_key(external_ref));

        let proposal_id = ExternalRef::<T>::get(external_ref);
        assert!(ProposalStatus::<T>::get(proposal_id) == ProposalStatusEnum::Active);
        assert_last_event::<T>(
            Event::ProposalSubmitted { proposal_id, external_ref, status: ProposalStatusEnum::Active }.into()
        );
    }
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
