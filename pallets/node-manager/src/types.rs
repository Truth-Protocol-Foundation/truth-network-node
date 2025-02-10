use crate::*;
use sp_runtime::Saturating;

#[derive(
    Copy, Clone, PartialEq, Default, Eq, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen,
)]
pub struct UptimeInfo<BlockNumber> {
    /// Number of uptime reported
    pub count: u64,
    /// Block number when the uptime was last reported
    pub last_reported: BlockNumber,
}

impl<BlockNumber: Copy> UptimeInfo<BlockNumber> {
    pub fn new(count: u64, last_reported: BlockNumber) -> UptimeInfo<BlockNumber> {
        UptimeInfo { count, last_reported }
    }
}

#[derive(Encode, Decode, Default, Clone, PartialEq, Debug, Eq, TypeInfo, MaxEncodedLen)]
pub struct NodeInfo<SignerId, AccountId> {
    /// The node owner
    pub owner: AccountId,
    /// The node signing key
    pub signing_key: SignerId,
}

impl<
        AccountId: Clone + FullCodec + MaxEncodedLen + TypeInfo,
        SignerId: Clone + FullCodec + MaxEncodedLen + TypeInfo,
    > NodeInfo<SignerId, AccountId>
{
    pub fn new(owner: AccountId, signing_key: SignerId) -> NodeInfo<SignerId, AccountId> {
        NodeInfo { owner, signing_key }
    }
}

#[derive(Encode, Decode, TypeInfo, Debug, Clone, PartialEq)]
pub enum AdminConfig<AccountId, Balance> {
    NodeRegistrar(AccountId),
    Heartbeat(u32),
}
