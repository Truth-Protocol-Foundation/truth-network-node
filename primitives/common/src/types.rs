use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{IdentifyAccount, Verify},
    MultiSignature, RuntimeDebug,
};

// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// Balance of an account.
pub type Balance = u128;

/// An index to a block.
pub type BlockNumber = u32;

/// Index of a transaction in the chain.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// Time
pub type Moment = u64;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Generic voting status that can be used by any pallet requiring voting functionality.
/// This provides a standard set of voting outcomes without coupling to specific pallet types.
#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, Clone, Copy, RuntimeDebug)]
pub enum VotingStatus {
    /// The vote/proposal was accepted by consensus
    Accepted,
    /// The vote/proposal was rejected by consensus  
    Rejected,
}
