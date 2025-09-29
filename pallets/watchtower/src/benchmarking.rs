// Copyright 2025 Truth Network.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::{EventRecord, RawOrigin};
use sp_avn_common::Proof;
use sp_runtime::{traits::Hash, SaturatedConversion};

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len().saturating_sub(1 as usize)];
    assert_eq!(event, &system_event);
}

fn create_proposal<T: Config>(
    external_ref_id: u32,
    created_at: BlockNumberFor<T>,
    end_at: Option<BlockNumberFor<T>>,
    is_internal: bool,
) -> Proposal<T> {
    let external_ref: T::Hash = T::Hashing::hash_of(&external_ref_id);
    let inner_payload = BoundedVec::try_from(external_ref_id.encode()).unwrap();
    let source: ProposalSource;
    let proposer: Option<T::AccountId>;

    if is_internal {
        source = ProposalSource::Internal(ProposalType::Governance);
        proposer = None;
    } else {
        source = ProposalSource::External;
        proposer = Some(account("proposer", 0, 0));
    };

    Proposal {
        title: BoundedVec::try_from("Bench proposal".as_bytes().to_vec()).unwrap(),
        external_ref: H256::from_slice(&external_ref.as_ref()),
        threshold: Perbill::from_percent(50),
        payload: Payload::Inline(inner_payload),
        source,
        proposer,
        decision_rule: DecisionRule::SimpleMajority,
        created_at,
        vote_duration: if let Some(end) = end_at {
            end.saturating_sub(created_at).saturated_into::<u32>()
        } else {
            MinVotingPeriod::<T>::get().saturated_into::<u32>()
        },
        end_at,
    }
}

fn set_active_proposal<T: Config>(proposal_id: H256, created_at: u32, length: u32) -> Proposal<T> {
    let created_at: BlockNumberFor<T> = created_at.into();
    let active_proposal =
        create_proposal::<T>(1, created_at, Some(created_at + length.into()), true);
    Proposals::<T>::insert(proposal_id, &active_proposal);
    ActiveInternalProposal::<T>::put(proposal_id);
    ProposalStatus::<T>::insert(proposal_id, ProposalStatusEnum::Active);
    active_proposal
}

fn queue_proposal<T: Config>(proposal_id: H256, created_at: u32) -> Proposal<T> {
    let created_at: BlockNumberFor<T> = created_at.into();
    let queued_proposal = create_proposal::<T>(2, created_at, None, true);
    Proposals::<T>::insert(proposal_id, &queued_proposal);
    Pallet::<T>::enqueue(proposal_id).unwrap();
    ProposalStatus::<T>::insert(proposal_id, ProposalStatusEnum::Queued);
    queued_proposal
}

fn get_proof<T: Config>(
    relayer: &T::AccountId,
    signer: &T::AccountId,
    signature: sp_core::sr25519::Signature,
) -> Proof<T::Signature, T::AccountId> {
    return Proof { signer: signer.clone(), relayer: relayer.clone(), signature: signature.into() }
}

benchmarks! {
    finalise_proposal {
        let signer: T::AccountId = account("signer", 0, 0);
        let proposal_id = H256::repeat_byte(3);
        let queued_proposal_id = H256::repeat_byte(7);
        <frame_system::Pallet<T>>::set_block_number(100u32.into());
        let _ = set_active_proposal::<T>(proposal_id, 5u32, 50u32);
        let _ = queue_proposal::<T>(queued_proposal_id, 100u32);
    }: finalise_proposal(RawOrigin::Signed(signer), proposal_id)
    verify {
        assert!(ProposalStatus::<T>::get(proposal_id) == ProposalStatusEnum::Expired);
        assert!(ProposalStatus::<T>::get(queued_proposal_id) == ProposalStatusEnum::Active);
        assert!(ActiveInternalProposal::<T>::get() == Some(queued_proposal_id));
    }

    set_admin_config_voting {
        let new_period: BlockNumberFor<T> = 36u32.into();
        let config = AdminConfig::MinVotingPeriod(new_period);
    }: set_admin_config(RawOrigin::Root, config)
    verify {
        assert!(<MinVotingPeriod<T>>::get() == new_period);
    }

    active_proposal_expiry_status {
        <frame_system::Pallet<T>>::set_block_number(100u32.into());

        // Pick internal because it has more logic
        let proposal_id = H256::repeat_byte(3);
        let _ = set_active_proposal::<T>(proposal_id, 5u32, 50u32);
        let now = <frame_system::Pallet<T>>::block_number();
        let mut id = H256::zero();
        let mut expired = false;
    }: {
        let result = Pallet::<T>::active_proposal_expiry_status(now);
        let (p_id, _p, p_expired) = result.expect("expired proposal exists");
        id = p_id;
        expired = p_expired;
     }
    verify {
        assert!(expired == true);
        assert!(id == proposal_id);
    }

    finalise_expired_voting {
        <frame_system::Pallet<T>>::set_block_number(100u32.into());

        let proposal_id = H256::repeat_byte(12);
        let active_proposal = set_active_proposal::<T>(proposal_id, 5u32, 50u32);

        let queued_proposal_id = H256::repeat_byte(7);
        let _ = queue_proposal::<T>(queued_proposal_id, 100u32);
    }: { let _ = Pallet::<T>::finalise_expired_voting(proposal_id, &active_proposal); }
    verify {
        assert!(ProposalStatus::<T>::get(proposal_id) == ProposalStatusEnum::Expired);
        assert!(ProposalStatus::<T>::get(queued_proposal_id) == ProposalStatusEnum::Active);
        assert!(ActiveInternalProposal::<T>::get() == Some(queued_proposal_id));
    }

}

impl_benchmark_test_suite!(
    Pallet,
    crate::mock::ExtBuilder::build_default().as_externality(),
    crate::mock::TestRuntime,
);
