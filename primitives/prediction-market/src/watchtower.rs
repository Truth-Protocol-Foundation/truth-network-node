use alloc::vec::Vec;
use frame_support::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{Perbill, RuntimeDebug};

pub type ProposalId = H256;

#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo)]
pub enum RawPayload {
    /// Small proposals that can fit safely in the runtime
    Inline(Vec<u8>),

    /// A link to off-chain proposal data (e.g. IPFS hash)
    Uri(Vec<u8>),
}

#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum DecisionRule {
    /// Yes > No to win
    SimpleMajority,
    /// Yes > No AND turnout >= min_turnout (percent of snapshot).
    MajorityWithTurnout { min_turnout: Perbill },
    /// Yes / (Yes+No) >= threshold AND turnout >= min_turnout (optional).
    Threshold { threshold: Perbill },
}

#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub enum ProposalSource<K>
where
    K: Parameter + Member + MaxEncodedLen + TypeInfo + Clone + Eq + core::fmt::Debug,
{
    /// External proposals created by other users. These require manual review and voting.
    External,
    /// Proposals created by other pallets. These can be voted on automatically by the pallet.
    Internal(K),
}

#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum VotingStatusEnum {
    Ongoing,
    Resolved { passed: bool },
    Cancelled,
    Unknown,
}

//implement default for VotingStatusEnum to be Unknown
impl Default for VotingStatusEnum {
    fn default() -> Self {
        VotingStatusEnum::Unknown
    }
}

#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo)]
#[scale_info(skip_type_params(K))]
pub struct ProposalRequest<K>
where
    K: Parameter + Member + MaxEncodedLen + TypeInfo + Clone + Eq + core::fmt::Debug,
{
    pub title: Vec<u8>,
    pub payload: RawPayload,
    pub rule: DecisionRule,
    pub source: ProposalSource<K>,
    /// A unique ref provided by the proposer. Used when sending notifications about this proposal.
    pub external_ref: H256,
    pub created_at: u32,
    pub max_vote_duration: u32,
}

// Interface for other pallets to interact with the watchtower pallet
pub trait WatchtowerInterface {
    type K: Parameter + Member + MaxEncodedLen + TypeInfo + Clone + Eq + core::fmt::Debug;
    type AccountId: Parameter;

    fn submit_proposal(
        proposer: Option<Self::AccountId>,
        proposal: ProposalRequest<Self::K>,
    ) -> DispatchResult;
    fn get_voting_status(proposal_id: ProposalId) -> VotingStatusEnum;
}
