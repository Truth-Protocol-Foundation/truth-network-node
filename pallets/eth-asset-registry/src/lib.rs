#![cfg_attr(not(feature = "std"), no_std)]
// Older clippy versions give a false positive on the expansion of [pallet::call].
// This is fixed in https://github.com/rust-lang/rust-clippy/issues/8321
#![allow(clippy::large_enum_variant)]
#![allow(clippy::too_many_arguments)]

use frame_support::{pallet_prelude::*, traits::EnsureOriginWithArg};
use frame_system::pallet_prelude::*;
pub use orml_traits::asset_registry::{AssetMetadata, AssetProcessor, Inspect};
use prediction_market_primitives::{
    traits::{HasEthAddress, InspectEthAsset},
    types::EthAddress,
};
use scale_info::TypeInfo;
use sp_core::H160;
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, CheckedAdd, Member, One},
    ArithmeticError, DispatchResult,
};
use sp_std::prelude::*;

pub use pallet::*;
pub use weights::WeightInfo;

mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Additional non-standard metadata to store for each asset
        type CustomMetadata: Parameter + Member + TypeInfo + MaxEncodedLen + HasEthAddress;

        /// The type used as a unique asset id,
        type AssetId: Parameter
            + Member
            + Default
            + TypeInfo
            + MaybeSerializeDeserialize
            + MaxEncodedLen;

        /// Checks that an origin has the authority to register/update an asset
        type AuthorityOrigin: EnsureOriginWithArg<Self::RuntimeOrigin, Option<Self::AssetId>>;

        /// A filter ran upon metadata registration that assigns an is and
        /// potentially modifies the supplied metadata.
        type AssetProcessor: AssetProcessor<
            Self::AssetId,
            AssetMetadata<Self::Balance, Self::CustomMetadata, Self::StringLimit>,
        >;

        /// The balance type.
        type Balance: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen;

        /// The maximum length of a name or symbol.
        #[pallet::constant]
        type StringLimit: Get<u32>;

        /// Weight information for extrinsics in this module.
        type WeightInfo: WeightInfo;
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Asset was not found.
        AssetNotFound,
        /// The asset id is invalid.
        InvalidAssetId,
        /// Another asset was already register with this eth address.
        ConflictingEthAddress,
        /// Another asset was already register with this asset id.
        ConflictingAssetId,
        /// Name or symbol is too long.
        InvalidAssetString,
        /// EthAddress is required to register an asset
        EthAddressIsMandatory,
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(crate) fn deposit_event)]
    pub enum Event<T: Config> {
        RegisteredAsset {
            asset_id: T::AssetId,
            metadata: AssetMetadata<T::Balance, T::CustomMetadata, T::StringLimit>,
        },
        UpdatedAsset {
            asset_id: T::AssetId,
            metadata: AssetMetadata<T::Balance, T::CustomMetadata, T::StringLimit>,
        },
    }

    /// The metadata of an asset, indexed by asset id.
    #[pallet::storage]
    #[pallet::getter(fn metadata)]
    pub type Metadata<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::AssetId,
        AssetMetadata<T::Balance, T::CustomMetadata, T::StringLimit>,
        OptionQuery,
    >;

    /// Maps a token eth address to an asset id
    #[pallet::storage]
    #[pallet::getter(fn eth_address_to_asset_id)]
    pub type EthAddressToAssetId<T: Config> =
        StorageMap<_, Twox64Concat, EthAddress, T::AssetId, OptionQuery>;

    /// The last processed asset id - used when assigning a sequential id.
    #[pallet::storage]
    #[pallet::getter(fn last_asset_id)]
    pub(crate) type LastAssetId<T: Config> = StorageValue<_, T::AssetId, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub assets: Vec<(EthAddress, T::AssetId, Vec<u8>)>,
        pub last_asset_id: T::AssetId,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self { assets: vec![], last_asset_id: Default::default() }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            self.assets.iter().for_each(|(eth_address, asset_id, metadata_encoded)| {
                let metadata = AssetMetadata::decode(&mut &metadata_encoded[..])
                    .expect("Error decoding AssetMetadata");
                Pallet::<T>::do_register_asset_without_asset_processor(
                    *eth_address,
                    metadata,
                    asset_id.clone(),
                )
                .expect("Error registering Asset");
            });

            LastAssetId::<T>::set(self.last_asset_id.clone());
        }
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_asset())]
        pub fn register_eth_asset(
            origin: OriginFor<T>,
            eth_address: EthAddress,
            metadata: AssetMetadata<T::Balance, T::CustomMetadata, T::StringLimit>,
            asset_id: Option<T::AssetId>,
        ) -> DispatchResult {
            T::AuthorityOrigin::ensure_origin(origin, &asset_id)?;

            Self::do_register_asset(eth_address, metadata, asset_id)
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::update_asset())]
        pub fn update_eth_asset(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
            decimals: Option<u32>,
            name: Option<BoundedVec<u8, T::StringLimit>>,
            symbol: Option<BoundedVec<u8, T::StringLimit>>,
            eth_address: Option<EthAddress>,
            existential_deposit: Option<T::Balance>,
            additional: Option<T::CustomMetadata>,
        ) -> DispatchResult {
            T::AuthorityOrigin::ensure_origin(origin, &Some(asset_id.clone()))?;

            Self::do_update_asset(
                asset_id,
                decimals,
                name,
                symbol,
                existential_deposit,
                eth_address,
                additional,
            )?;

            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Register a new eth asset
    pub fn do_register_asset(
        eth_address: EthAddress,
        metadata: AssetMetadata<T::Balance, T::CustomMetadata, T::StringLimit>,
        maybe_asset_id: Option<T::AssetId>,
    ) -> DispatchResult {
        ensure!(eth_address != H160::zero().into(), Error::<T>::EthAddressIsMandatory);

        let (asset_id, metadata) = T::AssetProcessor::pre_register(maybe_asset_id, metadata)?;

        Self::do_register_asset_without_asset_processor(
            eth_address,
            metadata.clone(),
            asset_id.clone(),
        )?;

        T::AssetProcessor::post_register(asset_id, metadata)?;

        Ok(())
    }

    /// Like do_register_asset, but without calling pre_register and
    /// post_register hooks.
    /// This function is useful in tests but it might also come in useful to
    /// users.
    pub fn do_register_asset_without_asset_processor(
        eth_address: EthAddress,
        metadata: AssetMetadata<T::Balance, T::CustomMetadata, T::StringLimit>,
        asset_id: T::AssetId,
    ) -> DispatchResult {
        Metadata::<T>::try_mutate(&asset_id, |maybe_metadata| -> DispatchResult {
            // make sure this asset id has not been registered yet
            ensure!(maybe_metadata.is_none(), Error::<T>::ConflictingAssetId);

            *maybe_metadata = Some(metadata.clone());
            Self::do_insert_eth_address_mapping(eth_address, asset_id.clone())?;

            Ok(())
        })?;

        Self::deposit_event(Event::<T>::RegisteredAsset { asset_id, metadata });

        Ok(())
    }

    pub fn do_update_asset(
        asset_id: T::AssetId,
        decimals: Option<u32>,
        name: Option<BoundedVec<u8, T::StringLimit>>,
        symbol: Option<BoundedVec<u8, T::StringLimit>>,
        existential_deposit: Option<T::Balance>,
        eth_address: Option<EthAddress>,
        additional: Option<T::CustomMetadata>,
    ) -> DispatchResult {
        Metadata::<T>::try_mutate(&asset_id, |maybe_metadata| -> DispatchResult {
            let metadata = maybe_metadata.as_mut().ok_or(Error::<T>::AssetNotFound)?;
            if let Some(decimals) = decimals {
                metadata.decimals = decimals;
            }

            if let Some(name) = name {
                metadata.name = name;
            }

            if let Some(symbol) = symbol {
                metadata.symbol = symbol;
            }

            if let Some(eth_address) = eth_address {
                Self::do_update_eth_address(
                    asset_id.clone(),
                    metadata.additional.eth_address().clone().into(),
                    eth_address.clone(),
                )?;
                metadata.additional.set_eth_address(eth_address.into());
            }

            if let Some(existential_deposit) = existential_deposit {
                metadata.existential_deposit = existential_deposit;
            }

            if let Some(additional) = additional {
                metadata.additional = additional;
            }

            Self::deposit_event(Event::<T>::UpdatedAsset {
                asset_id: asset_id.clone(),
                metadata: metadata.clone(),
            });

            Ok(())
        })?;

        Ok(())
    }

    pub fn fetch_metadata_by_eth_address(
        eth_address: &EthAddress,
    ) -> Option<AssetMetadata<T::Balance, T::CustomMetadata, T::StringLimit>> {
        let asset_id = EthAddressToAssetId::<T>::get(eth_address)?;
        Metadata::<T>::get(asset_id)
    }

    pub fn fetch_eth_address_by_asset_id(asset_id: &T::AssetId) -> Option<EthAddress> {
        Metadata::<T>::get(asset_id)
            .and_then(|metadata| Some(metadata.additional.eth_address().into()))
    }

    fn do_insert_eth_address_mapping(
        eth_address: EthAddress,
        asset_id: T::AssetId,
    ) -> DispatchResult {
        EthAddressToAssetId::<T>::try_mutate(eth_address, |maybe_asset_id| {
            ensure!(maybe_asset_id.is_none(), Error::<T>::ConflictingEthAddress);
            *maybe_asset_id = Some(asset_id);
            Ok(())
        })
    }

    fn do_update_eth_address(
        asset_id: T::AssetId,
        old_eth_address: EthAddress,
        new_eth_address: EthAddress,
    ) -> DispatchResult {
        if new_eth_address != old_eth_address {
            // remove the old eth address lookup if it exists
            EthAddressToAssetId::<T>::remove(old_eth_address);

            // insert new eth address
            Self::do_insert_eth_address_mapping(new_eth_address, asset_id)?;
        }

        Ok(())
    }
}

impl<T: Config> InspectEthAsset for Pallet<T> {
    type AssetId = T::AssetId;
    type Balance = T::Balance;
    type CustomMetadata = T::CustomMetadata;
    type StringLimit = T::StringLimit;

    fn asset_id(eth_address: &EthAddress) -> Option<Self::AssetId> {
        Pallet::<T>::eth_address_to_asset_id(eth_address)
    }

    fn metadata(
        id: &Self::AssetId,
    ) -> Option<AssetMetadata<Self::Balance, Self::CustomMetadata, Self::StringLimit>> {
        Pallet::<T>::metadata(id)
    }

    fn metadata_by_eth_address(
        eth_address: &EthAddress,
    ) -> Option<AssetMetadata<Self::Balance, Self::CustomMetadata, Self::StringLimit>> {
        Pallet::<T>::fetch_metadata_by_eth_address(&eth_address)
    }

    fn eth_address_by_asset_id(asset_id: &Self::AssetId) -> Option<EthAddress> {
        Pallet::<T>::fetch_eth_address_by_asset_id(asset_id)
    }
}

// Alias for AssetMetadata to improve readability (and to placate clippy)
pub type DefaultAssetMetadata<T> = AssetMetadata<
    <T as Config>::Balance,
    <T as Config>::CustomMetadata,
    <T as Config>::StringLimit,
>;

/// An AssetProcessor that assigns a sequential ID
pub struct SequentialId<T>(sp_std::marker::PhantomData<T>);

impl<T> AssetProcessor<T::AssetId, DefaultAssetMetadata<T>> for SequentialId<T>
where
    T: Config,
    T::AssetId: AtLeast32BitUnsigned,
{
    fn pre_register(
        id: Option<T::AssetId>,
        asset_metadata: DefaultAssetMetadata<T>,
    ) -> Result<(T::AssetId, DefaultAssetMetadata<T>), DispatchError> {
        let next_id = LastAssetId::<T>::get()
            .checked_add(&T::AssetId::one())
            .ok_or(ArithmeticError::Overflow)?;

        match id {
            Some(explicit_id) if explicit_id != next_id => {
                // we don't allow non-sequential ids
                Err(Error::<T>::InvalidAssetId.into())
            },
            _ => {
                LastAssetId::<T>::put(&next_id);
                Ok((next_id, asset_metadata))
            },
        }
    }
}
