use crate::*;
use frame_support::{CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebugNoBound};
use sp_core::hashing::blake2_256;

#[derive(
    Encode,
    Decode,
    RuntimeDebugNoBound,
    CloneNoBound,
    PartialEqNoBound,
    EqNoBound,
    TypeInfo,
    MaxEncodedLen,
)]
#[scale_info(skip_type_params(T))]
pub enum Payload<T: Config> {
    /// Small proposals that can fit safely in the runtime
    Inline(BoundedVec<u8, T::MaxInlineLen>),

    /// A link to off-chain proposal data (e.g. IPFS hash)
    Uri(BoundedVec<u8, T::MaxUriLen>),
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

#[derive(
    Encode,
    Decode,
    RuntimeDebugNoBound,
    CloneNoBound,
    PartialEqNoBound,
    EqNoBound,
    TypeInfo,
    MaxEncodedLen,
)]
#[scale_info(skip_type_params(T))]
pub enum ProposalSource<T: Config> {
    /// External proposals created by other users. These require manual review and voting.
    External,
    /// Proposals created by other pallets. These can be voted on automatically by the pallet.
    Internal(T::ProposalKind),
}

#[derive(
    Encode,
    Decode,
    RuntimeDebugNoBound,
    CloneNoBound,
    PartialEqNoBound,
    EqNoBound,
    TypeInfo,
    MaxEncodedLen,
)]
#[scale_info(skip_type_params(T))]
pub struct Proposal<T: Config> {
    pub title: BoundedVec<u8, T::MaxTitleLen>,
    pub payload: Payload<T>,
    pub rule: DecisionRule,
    pub source: ProposalSource<T>,
    /// A unique ref provided by the proposer. Used when sending notifications about this proposal.
    pub external_ref: H256,
    pub proposer: T::AccountId,
    pub created_at: BlockNumberFor<T>,
    pub end_at: BlockNumberFor<T>,
}

impl<T: Config> Proposal<T> {
    pub fn generate_id(self) -> ProposalId {
        let data =
            (self.title, self.payload, self.rule, self.source, self.external_ref, self.created_at)
                .encode();
        let hash = blake2_256(&data.clone());
        ProposalId::from(hash)
    }

    pub fn is_valid(&self) -> bool {
        self.end_at > self.created_at &&
            self.end_at >= frame_system::Pallet::<T>::block_number() + T::MinVotingPeriod::get() &&
            !self.title.is_empty() &&
            self.external_ref != H256::zero()
    }
}

pub trait NodeManagerInterface<AccountId, SignerId> {
    fn is_authorized_watchtower(who: &AccountId) -> bool;

    fn get_node_signing_key(node: &AccountId) -> Option<SignerId>;

    fn get_node_from_local_signing_keys() -> Option<(AccountId, SignerId)>;

    /// Get the count of authorized watchtowers without fetching the full list
    fn get_authorized_watchtowers_count() -> u32;
}

pub trait VoteStatusNotifier<BlockNumber: AtLeast32Bit> {
    fn on_voting_completed(external_ref: H256, status: VotingStatusEnum) -> DispatchResult;
}

/// Interface for other pallets to interact with the watchtower pallet
pub trait WatchtowerInterface {
    type Config: Config;

    fn submit_proposal(proposal: Proposal<Self::Config>) -> DispatchResult;
    fn get_voting_status(proposal_id: ProposalId) -> VotingStatusEnum;
    fn get_proposal(proposal_id: ProposalId) -> Option<Proposal<Self::Config>>;
}
