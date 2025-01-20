//! Mocks for the EthAssetRegistry module.

#![cfg(test)]

use super::*;
use frame_support::{
    construct_runtime, derive_impl, ord_parameter_types, parameter_types,
    traits::EnsureOriginWithArg,
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use sp_runtime::{traits::IdentityLookup, AccountId32, BuildStorage};

use crate as pallet_pm_eth_asset_registry;

pub type AccountId = AccountId32;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for TestRuntime {
    type Nonce = u64;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u64>;
}

type Balance = u64;

pub const ADMIN_ASSET_TWO: AccountId = AccountId32::new([42u8; 32]);
pub type EthAddress = H160;
pub type AssetId = u32;

parameter_types! {
    pub const StringLimit: u32 = 50;
}

ord_parameter_types! {
    pub const AdminAssetTwo: AccountId = ADMIN_ASSET_TWO;
}

pub struct AssetAuthority;
impl EnsureOriginWithArg<RuntimeOrigin, Option<u32>> for AssetAuthority {
    type Success = ();

    fn try_origin(
        origin: RuntimeOrigin,
        asset_id: &Option<u32>,
    ) -> Result<Self::Success, RuntimeOrigin> {
        match asset_id {
            // We mock an edge case where the asset_id 2 requires a special origin check.
            Some(2) => <EnsureSignedBy<AdminAssetTwo, AccountId32> as EnsureOrigin<
                RuntimeOrigin,
            >>::try_origin(origin.clone())
            .map(|_| ())
            .map_err(|_| origin),

            // Any other `asset_id` defaults to EnsureRoot
            _ => <EnsureRoot<AccountId> as EnsureOrigin<RuntimeOrigin>>::try_origin(origin),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin(_asset_id: &Option<u32>) -> Result<RuntimeOrigin, ()> {
        unimplemented!()
    }
}

#[derive(scale_info::TypeInfo, Encode, Decode, Clone, Eq, PartialEq, Debug, MaxEncodedLen)]
pub struct CustomMetadata {
    pub eth_address: EthAddress,
}

impl HasEthAddress for CustomMetadata {
    fn eth_address(&self) -> EthAddress {
        self.eth_address.clone()
    }

    fn set_eth_address(&mut self, eth_address: EthAddress) {
        self.eth_address = eth_address;
    }
}

impl Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type AssetId = AssetId;
    type AuthorityOrigin = AssetAuthority;
    type CustomMetadata = CustomMetadata;
    type AssetProcessor = SequentialId<TestRuntime>;
    type StringLimit = StringLimit;
    type WeightInfo = ();
}

type Block = frame_system::mocking::MockBlock<TestRuntime>;

construct_runtime!(
    pub enum TestRuntime {
        System: frame_system,
        AssetRegistry: pallet_pm_eth_asset_registry,
    }
);

#[derive(Default)]
pub struct ExtBuilder {}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let t = frame_system::GenesisConfig::<TestRuntime>::default().build_storage().unwrap();

        t.into()
    }

    pub fn build_with_genesis_assets(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::<TestRuntime>::default().build_storage().unwrap();

        pallet_pm_eth_asset_registry::GenesisConfig::<TestRuntime> {
            assets: vec![
                (
                    H160::from([1; 20]),
                    4,
                    AssetMetadata::<Balance, CustomMetadata, StringLimit>::encode(&AssetMetadata {
                        decimals: 6,
                        name: BoundedVec::truncate_from(
                            "Eth USDC - foreign token".as_bytes().to_vec(),
                        ),
                        symbol: BoundedVec::truncate_from("USDC".as_bytes().to_vec()),
                        existential_deposit: 0,
                        location: None,
                        additional: CustomMetadata { eth_address: H160::from([1; 20]) },
                    }),
                ),
                (
                    H160::from([2; 20]),
                    5,
                    AssetMetadata::<Balance, CustomMetadata, StringLimit>::encode(&AssetMetadata {
                        decimals: 18,
                        name: BoundedVec::truncate_from("tnf native token".as_bytes().to_vec()),
                        symbol: BoundedVec::truncate_from("TNF".as_bytes().to_vec()),
                        existential_deposit: 0,
                        location: None,
                        additional: CustomMetadata { eth_address: H160::from([2; 20]) },
                    }),
                ),
            ],
            last_asset_id: 5,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
