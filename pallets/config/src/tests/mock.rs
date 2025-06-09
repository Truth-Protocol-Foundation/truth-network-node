// Copyright 2025 Truth Network.

#![cfg(test)]

use crate::{self as pallet_config, *};
use frame_support::{parameter_types, weights::Weight};
use frame_system as system;
pub use prediction_market_primitives::test_helper::TestAccount;
pub use sp_core::{sr25519, H256};
pub use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup, Verify},
    BuildStorage, Perbill,
};

pub type Signature = sr25519::Signature;
pub type AccountId = <Signature as Verify>::Signer;

type Block = frame_system::mocking::MockBlock<TestRuntime>;

frame_support::construct_runtime!(
    pub enum TestRuntime
    {
        System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
        PalletConfig: pallet_config::{Pallet, Call, Storage, Event<T>, Config<T>},
    }
);

impl Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = ();
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
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

pub fn gas_fee_recipient() -> AccountId {
    TestAccount::new([17u8; 32]).account_id()
}

pub fn alice() -> AccountId {
    TestAccount::new([7u8; 32]).account_id()
}

pub fn admin_account() -> AccountId {
    TestAccount::new([8u8; 32]).account_id()
}

#[derive(Default)]
pub struct ExtBuilder {
    pub storage: sp_runtime::Storage,
}

impl ExtBuilder {
    pub fn build(self) -> Self {
        let mut storage =
            frame_system::GenesisConfig::<TestRuntime>::default().build_storage().unwrap();

        // see the logs in tests when using `RUST_LOG=debug cargo test -- --nocapture`
        let _ = env_logger::builder().is_test(true).try_init();

        let _ = pallet_config::GenesisConfig::<TestRuntime> {
            admin_account: Some(admin_account()),
            gas_fee_recipient: Some(gas_fee_recipient()),
            base_gas_fee: 10000000000u128,
        }
        .assimilate_storage(&mut storage)
        .unwrap();

        Self { storage }
    }

    pub fn as_externality(self) -> sp_io::TestExternalities {
        let mut ext = sp_io::TestExternalities::from(self.storage);
        // Events do not get emitted on block 0, so we increment the block here
        ext.execute_with(|| {
            frame_system::Pallet::<TestRuntime>::set_block_number(1u32.into());
        });
        ext
    }
}
