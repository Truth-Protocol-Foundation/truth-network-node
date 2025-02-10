// Copyright 2025 Truth Network.

#![cfg(test)]

use crate::{self as pallet_node_manager, *};
use common_primitives::constants::{currency::BASE, NODE_MANAGER_PALLET_ID};
use frame_support::{parameter_types, weights::Weight};
use frame_system as system;
use sp_core::{
    offchain::testing::{OffchainState, PendingRequest},
    sr25519, H256,
};
use sp_runtime::{
    testing::{TestXt, UintAuthorityId},
    traits::{BlakeTwo256, ConvertInto, IdentifyAccount, IdentityLookup, Verify},
    BuildStorage, Perbill, SaturatedConversion,
};
use sp_state_machine::BasicExternalities;
use std::cell::RefCell;

pub type Signature = sr25519::Signature;
pub type AccountId = <Signature as Verify>::Signer;
pub type Extrinsic = TestXt<RuntimeCall, ()>;

type Block = frame_system::mocking::MockBlock<TestRuntime>;

frame_support::construct_runtime!(
    pub enum TestRuntime
    {
        System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
        NodeManager: pallet_node_manager::{Pallet, Call, Storage, Event<T>, Config<T>},
        AVN: pallet_avn::{Pallet, Storage, Event, Config<T>},
    }
);
impl Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type SignerId = UintAuthorityId;
    type Public = AccountId;
    type Signature = Signature;
    type WeightInfo = ();
}

impl<LocalCall> system::offchain::SendTransactionTypes<LocalCall> for TestRuntime
where
    RuntimeCall: From<LocalCall>,
{
    type OverarchingCall = RuntimeCall;
    type Extrinsic = Extrinsic;
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024 as u64, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const ChallengePeriod: u64 = 2;
}

impl system::Config for TestRuntime {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type Nonce = u64;
    type RuntimeCall = RuntimeCall;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_avn::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type AuthorityId = UintAuthorityId;
    type EthereumPublicKeyChecker = ();
    type NewSessionHandler = ();
    type DisabledValidatorChecker = ();
    type WeightInfo = ();
}
