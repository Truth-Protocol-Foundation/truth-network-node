use crate::*;
use frame_support::{CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebugNoBound};
use sp_runtime::traits::Hash;

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

pub fn to_proposal<T: Config, K>(
    request: ProposalRequest<K>,
    proposer: Option<T::AccountId>,
) -> Result<Proposal<T, K>, Error<T>>
where
    K: Parameter + Member + MaxEncodedLen + TypeInfo + Clone + Eq + core::fmt::Debug,
{
    Ok(Proposal {
        title: BoundedVec::try_from(request.title).map_err(|_| Error::<T>::InvalidTitle)?,
        payload: to_payload(request.payload)?,
        rule: request.rule,
        source: request.source,
        external_ref: request.external_ref,
        proposer,
        created_at: BlockNumberFor::<T>::from(request.created_at),
        end_at: frame_system::Pallet::<T>::block_number() + request.max_vote_duration.into(),
    })
}

pub fn to_payload<T: Config>(raw: RawPayload) -> Result<Payload<T>, Error<T>> {
    match raw {
        RawPayload::Inline(data) => {
            let bounded =
                BoundedVec::try_from(data).map_err(|_| Error::<T>::InvalidInlinePayload)?;
            Ok(Payload::Inline(bounded))
        },
        RawPayload::Uri(data) => {
            let bounded = BoundedVec::try_from(data).map_err(|_| Error::<T>::InvalidUri)?;
            Ok(Payload::Uri(bounded))
        },
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
pub struct Proposal<T: Config, K>
where
    K: Parameter + Member + MaxEncodedLen + TypeInfo + Clone + Eq + core::fmt::Debug,
{
    pub title: BoundedVec<u8, T::MaxTitleLen>,
    pub payload: Payload<T>,
    pub rule: DecisionRule,
    pub source: ProposalSource<K>,
    /// A unique ref provided by the proposer. Used when sending notifications about this proposal.
    pub external_ref: H256,
    // Internal proposer or Root do not have an account id
    pub proposer: Option<T::AccountId>,
    pub created_at: BlockNumberFor<T>,
    pub end_at: BlockNumberFor<T>,
}

impl<
        T: Config,
        K: Parameter + Member + MaxEncodedLen + TypeInfo + Clone + Eq + core::fmt::Debug,
    > Proposal<T, K>
{
    pub fn generate_id(self) -> ProposalId {
        // External ref is unique globally, so we can use it to generate a unique id
        let data =
            (self.title, self.payload, self.rule, self.source, self.external_ref, self.created_at)
                .encode();
        let hash = sp_io::hashing::blake2_256(&data);
        ProposalId::from(hash)
    }

    pub fn is_valid(&self) -> bool {
        let base_is_valid = !self.title.is_empty() &&
            self.external_ref != H256::zero() &&
            self.end_at >= self.created_at + T::MinVotingPeriod::get();

        let payload_valid = match &self.payload {
            Payload::Inline(data) =>
                !data.is_empty() && matches!(self.source, ProposalSource::Internal(_)),
            Payload::Uri(data) =>
                !data.is_empty() && matches!(self.source, ProposalSource::External),
        };

        base_is_valid && payload_valid
    }
}

pub trait NodeManagerInterface<AccountId, SignerId> {
    fn is_authorized_watchtower(who: &AccountId) -> bool;

    fn get_node_signing_key(node: &AccountId) -> Option<SignerId>;

    fn get_node_from_local_signing_keys() -> Option<(AccountId, SignerId)>;

    /// Get the count of authorized watchtowers without fetching the full list
    fn get_authorized_watchtowers_count() -> u32;
}

pub trait VoteStatusNotifier {
    fn on_voting_completed(external_ref: H256, status: VotingStatusEnum) -> DispatchResult;
}

impl VoteStatusNotifier for () {
    fn on_voting_completed(_external_ref: H256, _status: VotingStatusEnum) -> DispatchResult {
        Ok(())
    }
}
