use alloc::vec::Vec;
use frame_support::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{Perbill, RuntimeDebug, traits::AtLeast32Bit};

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
pub enum ProposalSource<ProposalKind>
where
    ProposalKind: Parameter + Member + MaxEncodedLen + TypeInfo + Clone + Eq + core::fmt::Debug,
{
    /// External proposals created by other users. These require manual review and voting.
    External,
    /// Proposals created by other pallets. These can be voted on automatically by the pallet.
    Internal(ProposalKind),
}

#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum VotingStatusEnum {
    Queued,
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
#[scale_info(skip_type_params(ProposalKind))]
pub struct ProposalRequest<ProposalKind>
where
    ProposalKind: Parameter + Member + MaxEncodedLen + TypeInfo + Clone + Eq + core::fmt::Debug,
{
    pub title: Vec<u8>,
    pub payload: RawPayload,
    pub rule: DecisionRule,
    pub source: ProposalSource<ProposalKind>,
    /// A unique ref provided by the proposer. Used when sending notifications about this proposal.
    pub external_ref: H256,
    pub created_at: u32,
    pub max_vote_duration: u32,
}

// Interface for other pallets to interact with the watchtower pallet
pub trait WatchtowerInterface {
    type ProposalKind: Parameter + Member + MaxEncodedLen + TypeInfo + Clone + Eq + core::fmt::Debug;
    type AccountId: Parameter;

    fn submit_proposal(
        proposer: Option<Self::AccountId>,
        proposal: ProposalRequest<Self::ProposalKind>,
    ) -> DispatchResult;

    fn get_voting_status(proposal_id: ProposalId) -> VotingStatusEnum;
    fn get_proposer(proposal_id: ProposalId) -> Option<Self::AccountId>;
}

pub trait WatchtowerHooks {
    type Proposal: Parameter;

    /// Called when Watchtower raises an alert/notification.
    fn on_proposal_submitted(proposal_id: ProposalId, proposal: Self::Proposal) -> DispatchResult;
    fn on_consensus_reached(proposal_id: ProposalId, external_ref: &H256) -> DispatchResult;
    fn on_cancelled(proposal_id: ProposalId, external_ref: &H256) -> DispatchResult;
}

/*
    //-------------------------------------------//
    // This is a placehold and should be removed //
    //-------------------------------------------//
*/

#[derive(Encode, Decode, Default, Clone, Copy, PartialEq, Debug, Eq, TypeInfo, MaxEncodedLen)]
pub struct RootRange<BlockNumber: AtLeast32Bit> {
    pub from_block: BlockNumber,
    pub to_block: BlockNumber,
}

impl<BlockNumber: AtLeast32Bit> RootRange<BlockNumber> {
    pub fn new(from_block: BlockNumber, to_block: BlockNumber) -> Self {
        return RootRange::<BlockNumber> { from_block, to_block }
    }
}

#[derive(Encode, Decode, Default, Clone, Copy, PartialEq, Debug, Eq, TypeInfo, MaxEncodedLen)]
pub struct RootId<BlockNumber: AtLeast32Bit> {
    pub range: RootRange<BlockNumber>,
    pub ingress_counter: u32,
}

impl<BlockNumber: AtLeast32Bit + Encode> RootId<BlockNumber> {
    pub fn new(range: RootRange<BlockNumber>, ingress_counter: u32) -> Self {
        return RootId::<BlockNumber> { range, ingress_counter }
    }
}