// Copyright 2022-2024 Forecasting Technologies LTD.
// Copyright 2021-2022 Zeitgeist PM LLC.
//
// This file is part of Zeitgeist.
//
// Zeitgeist is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at
// your option) any later version.
//
// Zeitgeist is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Zeitgeist. If not, see <https://www.gnu.org/licenses/>.

pub use crate::{
    asset::*, market::*, max_runtime_usize::*, outcome_report::OutcomeReport, proxy_type::*,
    serde_wrapper::*, traits::HasEthAddress,
};
use common_primitives::types::Balance;

#[cfg(feature = "arbitrary")]
use arbitrary::{Arbitrary, Result, Unstructured};
use frame_support::weights::Weight;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::sr25519;
use sp_runtime::{traits::Verify, MultiSignature};

/// Signed counter-part of Balance
pub type OrmlAmount = i128;

/// The index of the category for a `CategoricalOutcome` asset.
pub type CategoryIndex = u16;

/// Multihash for digest sizes up to 384 bit.
/// The multicodec encoding the hash algorithm uses only 1 byte,
/// effecitvely limiting the number of available hash types.
/// HashType (1B) + DigestSize (1B) + Hash (48B).
#[derive(TypeInfo, Clone, Debug, Decode, Encode, Eq, PartialEq)]
pub enum MultiHash {
    Sha3_384([u8; 50]),
}

// Implementation for the fuzzer
#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for MultiHash {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        let mut rand_bytes = <[u8; 50] as Arbitrary<'a>>::arbitrary(u)?;
        rand_bytes[0] = 0x15;
        rand_bytes[1] = 0x30;
        Ok(MultiHash::Sha3_384(rand_bytes))
    }

    fn size_hint(_depth: usize) -> (usize, Option<usize>) {
        (50, Some(50))
    }
}

/// ORML adapter
pub type BasicCurrencyAdapter<R, B> =
    orml_currencies::BasicCurrencyAdapter<R, B, OrmlAmount, Balance>;

pub type CurrencyId = Asset<MarketId>;

/// The asset id specifically used for pallet_assets_tx_payment for
/// paying transaction fees in different assets.
/// Since the polkadot extension and wallets can't handle custom asset ids other than just u32,
/// we are using a u32 as on the asset-hubs here.
pub type TxPaymentAssetId = u32;

/// Index of a transaction in the chain.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// The market identifier type.
pub type MarketId = u128;

/// Time
pub type Moment = u64;

/// The identifier type for pools.
pub type PoolId = u128;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// A type that represents an Ethereum Address
pub type EthAddress = sp_core::H160;

// Tests
pub type AccountIdTest = u128;
pub type SignatureTest = sr25519::Signature;
pub type TestAccountIdPK = <SignatureTest as Verify>::Signer;

#[cfg(feature = "std")]
pub type BlockTest<R> = frame_system::mocking::MockBlock<R>;

#[cfg(feature = "std")]
pub type UncheckedExtrinsicTest<R> = frame_system::mocking::MockUncheckedExtrinsic<R>;

#[derive(sp_runtime::RuntimeDebug, Clone, Decode, Encode, Eq, PartialEq, TypeInfo)]
pub struct ResultWithWeightInfo<R> {
    pub result: R,
    pub weight: Weight,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Decode,
    Default,
    Encode,
    Eq,
    MaxEncodedLen,
    Ord,
    PartialEq,
    PartialOrd,
    TypeInfo,
)]
/// Custom asset metadata
pub struct CustomMetadata {
    /// The Ethereum address of the asset
    pub eth_address: EthAddress,
    /// Whether an asset can be used as base_asset in pools.
    pub allow_as_base_asset: bool,
}

impl HasEthAddress for CustomMetadata {
    fn eth_address(&self) -> EthAddress {
        self.eth_address.clone()
    }

    fn set_eth_address(&mut self, eth_address: EthAddress) {
        self.eth_address = eth_address;
    }
}

#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq)]
pub enum AdminConfig<T> {
    MarketAdmin(T),
    VaultAccount(T),
}
