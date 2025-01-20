#![cfg(test)]

use super::mock;
use crate as pallet_pm_eth_asset_registry;
use crate::{
    //mock::{AdminAssetTwo, AssetRegistry, CustomMetadata, RuntimeOrigin, },
    Error,
    LastAssetId,
    Metadata,
    H160,
};
use frame_support::{assert_noop, assert_ok, pallet_prelude::*};
use mock::{RuntimeCall, *};
use orml_traits::asset_registry::AssetMetadata;
use scale_info::TypeInfo;

use sp_runtime::traits::{BadOrigin, Dispatchable};

use prediction_market_primitives::traits::HasEthAddress;

fn dummy_metadata() -> AssetMetadata<
    <TestRuntime as pallet_pm_eth_asset_registry::Config>::Balance,
    CustomMetadata,
    <TestRuntime as pallet_pm_eth_asset_registry::Config>::StringLimit,
> {
    AssetMetadata {
        decimals: 12,
        name: BoundedVec::truncate_from("dummy token".as_bytes().to_vec()),
        symbol: BoundedVec::truncate_from("DMY".as_bytes().to_vec()),
        existential_deposit: 0,
        location: None,
        additional: CustomMetadata { eth_address: H160::from([1; 20]) },
    }
}

#[test]
fn genesis_issuance_should_work() {
    ExtBuilder::default().build_with_genesis_assets().execute_with(|| {
        let metadata1 = AssetMetadata {
            decimals: 6,
            name: BoundedVec::truncate_from("Eth USDC - foreign token".as_bytes().to_vec()),
            symbol: BoundedVec::truncate_from("USDC".as_bytes().to_vec()),
            existential_deposit: 0,
            location: None,
            additional: CustomMetadata { eth_address: H160::from([1; 20]) },
        };
        let metadata2 = AssetMetadata {
            decimals: 18,
            name: BoundedVec::truncate_from("tnf native token".as_bytes().to_vec()),
            symbol: BoundedVec::truncate_from("TNF".as_bytes().to_vec()),
            existential_deposit: 0,
            location: None,
            additional: CustomMetadata { eth_address: H160::from([2; 20]) },
        };
        assert_eq!(AssetRegistry::metadata(4).unwrap(), metadata1);
        assert_eq!(AssetRegistry::metadata(5).unwrap(), metadata2);
        assert_eq!(LastAssetId::<TestRuntime>::get(), 5);
    });
}

#[test]
/// tests the SequentialId AssetProcessor
fn test_sequential_id_normal_behavior() {
    ExtBuilder::default().build().execute_with(|| {
        let metadata1 = dummy_metadata();

        let metadata2 = AssetMetadata {
            name: BoundedVec::truncate_from("Test token 2".as_bytes().to_vec()),
            symbol: BoundedVec::truncate_from("TKN2".as_bytes().to_vec()),
            additional: CustomMetadata { eth_address: H160::from([20; 20]) },
            ..dummy_metadata()
        };
        AssetRegistry::register_eth_asset(
            RuntimeOrigin::root(),
            H160::from([10; 20]),
            metadata1.clone(),
            None,
        )
        .unwrap();
        AssetRegistry::register_eth_asset(
            RuntimeOrigin::root(),
            H160::from([20; 20]),
            metadata2.clone(),
            None,
        )
        .unwrap();

        assert_eq!(AssetRegistry::metadata(1).unwrap(), metadata1);
        assert_eq!(AssetRegistry::metadata(2).unwrap(), metadata2);
    });
}

#[test]
fn test_sequential_id_with_invalid_id_returns_error() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(AssetRegistry::register_eth_asset(
            RuntimeOrigin::root(),
            H160::from([10; 20]),
            dummy_metadata(),
            Some(1)
        ));
        assert_noop!(
            AssetRegistry::register_eth_asset(
                RuntimeOrigin::root(),
                H160::from([10; 20]),
                dummy_metadata(),
                Some(1)
            ),
            Error::<TestRuntime>::InvalidAssetId
        );
    });
}

#[test]
fn test_register_duplicate_eth_address_returns_error() {
    ExtBuilder::default().build().execute_with(|| {
        let metadata = dummy_metadata();
        let eth_address = H160::from([10; 20]);

        assert_ok!(AssetRegistry::register_eth_asset(
            RuntimeOrigin::root(),
            eth_address,
            metadata.clone(),
            None
        ));
        let register_eth_asset =
            RuntimeCall::AssetRegistry(crate::Call::<TestRuntime>::register_eth_asset {
                eth_address,
                metadata,
                asset_id: None,
            });
        assert_noop!(
            register_eth_asset.dispatch(RuntimeOrigin::root()),
            Error::<TestRuntime>::ConflictingEthAddress
        );
    });
}

#[test]
fn test_register_duplicate_asset_id_returns_error() {
    ExtBuilder::default().build().execute_with(|| {
        let eth_address = H160::from([10; 20]);

        assert_ok!(AssetRegistry::register_eth_asset(
            RuntimeOrigin::root(),
            eth_address,
            dummy_metadata(),
            Some(1)
        ));
        assert_noop!(
            AssetRegistry::do_register_asset_without_asset_processor(
                eth_address,
                dummy_metadata(),
                1
            ),
            Error::<TestRuntime>::ConflictingAssetId
        );
    });
}

#[test]
fn test_update_metadata_works() {
    ExtBuilder::default().build().execute_with(|| {
        let eth_address = H160::from([10; 20]);

        let old_metadata = dummy_metadata();
        assert_ok!(AssetRegistry::register_eth_asset(
            RuntimeOrigin::root(),
            eth_address,
            old_metadata.clone(),
            None
        ));

        let new_metadata = AssetMetadata {
            decimals: 11,
            name: BoundedVec::truncate_from("new native token2".as_bytes().to_vec()),
            symbol: BoundedVec::truncate_from("NTV2".as_bytes().to_vec()),
            existential_deposit: 1,
            location: None,
            additional: CustomMetadata { eth_address: H160::from([21; 20]) },
        };
        assert_ok!(AssetRegistry::update_eth_asset(
            RuntimeOrigin::root(),
            1,
            Some(new_metadata.decimals),
            Some(new_metadata.name.clone()),
            Some(new_metadata.symbol.clone()),
            Some(new_metadata.additional.eth_address()),
            Some(new_metadata.existential_deposit),
            Some(new_metadata.additional.clone())
        ));

        let old_eth_address: EthAddress = old_metadata.additional.eth_address().try_into().unwrap();
        let new_eth_address: EthAddress =
            new_metadata.additional.eth_address().clone().try_into().unwrap();

        // check that the old location was removed and the new one added
        assert_eq!(AssetRegistry::eth_address_to_asset_id(old_eth_address), None);
        assert_eq!(AssetRegistry::eth_address_to_asset_id(new_eth_address), Some(1));

        assert_eq!(AssetRegistry::metadata(1).unwrap(), new_metadata);
    });
}

#[test]
fn test_update_metadata_fails_with_unknown_asset() {
    ExtBuilder::default().build().execute_with(|| {
        let eth_address = H160::from([10; 20]);
        let old_metadata = dummy_metadata();
        assert_ok!(AssetRegistry::register_eth_asset(
            RuntimeOrigin::root(),
            eth_address,
            old_metadata,
            None
        ));

        assert_noop!(
            AssetRegistry::update_eth_asset(
                RuntimeOrigin::root(),
                4,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            Error::<TestRuntime>::AssetNotFound
        );
    });
}

#[test]
fn test_asset_authority() {
    ExtBuilder::default().build().execute_with(|| {
        let eth_address = H160::from([10; 20]);
        let metadata = dummy_metadata();

        // Assert that root can register an asset with id 1
        assert_ok!(AssetRegistry::register_eth_asset(
            RuntimeOrigin::root(),
            eth_address,
            metadata,
            Some(1)
        ));

        // Assert that only Account42 can register asset with id 2
        let metadata = AssetMetadata { location: None, ..dummy_metadata() };

        // It fails when signed with root...
        assert_noop!(
            AssetRegistry::register_eth_asset(
                RuntimeOrigin::root(),
                eth_address,
                metadata.clone(),
                Some(2)
            ),
            BadOrigin
        );
        // It works when signed with the right account
        let eth_address_2 = H160::from([11; 20]);
        assert_ok!(AssetRegistry::register_eth_asset(
            RuntimeOrigin::signed(AdminAssetTwo::get()),
            eth_address_2,
            metadata,
            Some(2)
        ));
    });
}

#[test]
fn test_decode_bounded_vec() {
    pub mod unbounded {
        use super::*;

        #[frame_support::storage_alias]
        pub type Metadata<T: pallet_pm_eth_asset_registry::Config> = StorageMap<
            pallet_pm_eth_asset_registry::Pallet<T>,
            Twox64Concat,
            <T as pallet_pm_eth_asset_registry::Config>::AssetId,
            AssetMetadata<
                <T as pallet_pm_eth_asset_registry::Config>::Balance,
                <T as pallet_pm_eth_asset_registry::Config>::CustomMetadata,
            >,
            OptionQuery,
        >;

        #[derive(TypeInfo, Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
        pub struct AssetMetadata<Balance, CustomMetadata: Parameter + Member + TypeInfo> {
            pub decimals: u32,
            pub name: Vec<u8>,
            pub symbol: Vec<u8>,
            pub existential_deposit: Balance,
            pub location: Option<u32>,
            pub additional: CustomMetadata,
        }
    }

    ExtBuilder::default().build().execute_with(|| {
        let para_name = "para A native token".as_bytes().to_vec();
        let para_symbol = "paraA".as_bytes().to_vec();
        unbounded::Metadata::<TestRuntime>::insert(
            0,
            unbounded::AssetMetadata {
                decimals: 12,
                name: para_name.clone(),
                symbol: para_symbol.clone(),
                existential_deposit: 0,
                location: None,
                additional: CustomMetadata { eth_address: H160::from([12; 20]) },
            },
        );

        let asset_metadata = Metadata::<TestRuntime>::get(0);
        assert_eq!(
            asset_metadata.map(|m| (m.name.to_vec(), m.symbol.to_vec())),
            Some((para_name, para_symbol))
        );
    });
}
