use crate::*;
use frame_support::{CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebugNoBound};

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
    pub threshold: Perbill,
    pub source: ProposalSource,
    pub decision_rule: DecisionRule,
    /// A unique ref provided by the proposer. Used when sending notifications about this proposal.
    pub external_ref: H256,
    // Internal proposer or Root do not have an account id
    pub proposer: Option<T::AccountId>,
    pub created_at: BlockNumberFor<T>,
    pub vote_duration: u32,
    pub end_at: Option<BlockNumberFor<T>>,
}

impl<T: Config> Proposal<T> {
    pub fn generate_id(&self) -> ProposalId {
        // External ref is unique globally, so we can use it to generate a unique id
        let data = (self.external_ref, self.created_at, self.vote_duration).encode();
        let hash = sp_io::hashing::blake2_256(&data);
        ProposalId::from(hash)
    }

    pub fn is_valid(&self, current_block: BlockNumberFor<T>) -> bool {
        let base_is_valid = !self.title.is_empty() &&
            self.external_ref != H256::zero() &&
            self.vote_duration >= MinVotingPeriod::<T>::get().saturated_into::<u32>() &&
            self.threshold <= Perbill::one() &&
            self.created_at <= current_block;

        let payload_valid = match &self.payload {
            Payload::Inline(data) =>
                !data.is_empty() && matches!(self.source, ProposalSource::Internal(_)),
            Payload::Uri(data) =>
                !data.is_empty() && matches!(self.source, ProposalSource::External),
        };

        base_is_valid && payload_valid
    }
}
#[derive(Encode, Decode, TypeInfo, Debug, Clone, PartialEq)]
pub enum AdminConfig<BlockNumber> {
    MinVotingPeriod(BlockNumber),
}
