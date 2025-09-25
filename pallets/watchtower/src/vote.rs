use crate::*;

impl<T: Config> Pallet<T> {
    pub fn threshold_achieved(proposal_id: ProposalId, threshold: Perbill) -> Option<bool> {
        let vote = Votes::<T>::get(proposal_id);
        let total_voters = T::Watchtowers::get_authorized_watchtowers_count();
        let min_votes = threshold * total_voters;

        if vote.ayes >= min_votes {
            Some(true)
        } else if vote.nays >= min_votes {
            Some(false)
        } else {
            None
        }
    }

    pub fn get_consensus_result(
        proposal_id: ProposalId,
        proposal: &Proposal<T>,
        result: bool,
    ) -> ProposalStatusEnum {
        if result {
            ProposalStatusEnum::Resolved { passed: true }
        } else {
            ProposalStatusEnum::Resolved { passed: false }
        }
    }

    pub fn get_vote_result_on_expiry(
        proposal_id: ProposalId,
        proposal: &Proposal<T>,
    ) -> ProposalStatusEnum {
        match proposal.source {
            ProposalSource::Internal(_) => ProposalStatusEnum::Expired,
            ProposalSource::External => {
                let votes = Votes::<T>::get(proposal_id);
                if proposal.decision_rule == DecisionRule::SimpleMajority && votes.ayes > votes.nays
                {
                    ProposalStatusEnum::Resolved { passed: true }
                } else {
                    ProposalStatusEnum::Resolved { passed: false }
                }
            },
        }
    }

    pub fn finalise_voting(
        proposal_id: ProposalId,
        proposal: &Proposal<T>,
        consensus_result: ProposalStatusEnum,
    ) -> DispatchResult {
        ProposalStatus::<T>::insert(proposal_id, consensus_result.clone());
        T::VoteStatusNotifier::on_voting_completed(
            proposal.external_ref,
            consensus_result.clone(),
        )?;

        Self::deposit_event(Event::VotingEnded {
            proposal_id,
            external_ref: proposal.external_ref,
            consensus_result,
        });

        // If this was an internal proposal, activate the next one in the queue
        if let ProposalSource::Internal(_) = proposal.source {
            ActiveInternalProposal::<T>::kill();
            if let Ok(next_proposal_id) = Self::dequeue() {
                ActiveInternalProposal::<T>::put(next_proposal_id);
                ProposalStatus::<T>::insert(next_proposal_id, ProposalStatusEnum::Active);
                Proposals::<T>::mutate(next_proposal_id, |p_opt| {
                    if let Some(p) = p_opt {
                        p.end_at = Some(
                            frame_system::Pallet::<T>::block_number() + p.vote_duration.into(),
                        );
                    }
                });
            }
        }


        Ok(())
    }

    pub fn finalise_voting_if_required(
        proposal_id: ProposalId,
        proposal: &Proposal<T>,
    ) -> DispatchResult {
        let mut consensus_result = None;
        if let Some(consensus) = Self::threshold_achieved(proposal_id, proposal.threshold) {
            consensus_result = Some(Self::get_consensus_result(proposal_id, proposal, consensus));
        } else {
            let current_block = <frame_system::Pallet<T>>::block_number();
            if current_block >= proposal.end_at.unwrap_or(0u32.into()) {
                consensus_result = Some(Self::get_vote_result_on_expiry(proposal_id, proposal));
            }
        }

        if let Some(consensus_result) = consensus_result {
            Self::finalise_voting(proposal_id, proposal, consensus_result)?;
        }

        Ok(())
    }

}
