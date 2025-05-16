#![cfg_attr(not(feature = "std"), no_std)]
use frame_support::{
    dispatch::DispatchResult,
    pallet_prelude::*,
    traits::{IsSubType, StorageVersion},
};
use frame_system::{ensure_root, pallet_prelude::*};
use sp_runtime::traits::Dispatchable;

pub mod default_weights;
pub use default_weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
#[path = "tests/mock.rs"]
mod mock;
#[cfg(test)]
#[path = "tests/test.rs"]
mod test;

#[cfg(not(feature = "std"))]
use sp_std::prelude::*;

pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    /// The account that manages fees and gas fee recipients
    #[pallet::storage]
    pub type AdminAccount<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// The account that receives the chain's gas fees
    #[pallet::storage]
    pub type GasFeeRecipientAccount<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// The base gas fee for a simple token transfer
    #[pallet::storage]
    pub type BaseGasFee<T: Config> = StorageValue<_, u128, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub admin_account: Option<T::AccountId>,
        pub gas_fee_recipient: Option<T::AccountId>,
        pub base_gas_fee: u128,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self { admin_account: None, gas_fee_recipient: None, base_gas_fee: 0u128 }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            AdminAccount::<T>::set(self.admin_account.clone());
            GasFeeRecipientAccount::<T>::set(self.gas_fee_recipient.clone());
            BaseGasFee::<T>::set(self.base_gas_fee.clone());
        }
    }

    // Pallet Events
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new admin account has been set
        AdminAccountSet { new_admin: T::AccountId },
        /// A new gas fee recipient has been set
        GasFeeRecipientSet { new_account: T::AccountId },
        /// A new base gas fee has been set
        BaseGasFeeSet { new_base_gas_fee: u128 },
    }

    // Pallet Errors
    #[pallet::error]
    pub enum Error<T> {
        /// The admin account is not set
        AdminAccountNotSet,
        /// The gas fee recipient account is not set
        GasFeeRecipientNotSet,
        /// The numerator must be greater than zero
        BaseGasFeeZero,
        /// The sender is not the admin account
        SenderNotAdmin,
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>>
            + Into<<Self as frame_system::Config>::RuntimeEvent>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The overarching call type.
        type RuntimeCall: Parameter
            + Dispatchable<RuntimeOrigin = <Self as frame_system::Config>::RuntimeOrigin>
            + IsSubType<Call<Self>>
            + From<Call<Self>>;
        /// The weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Set the gas fee recipient account
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::set_gas_fee_recipient())]
        pub fn set_gas_fee_recipient(
            origin: OriginFor<T>,
            recipient: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let admin = <AdminAccount<T>>::get().ok_or(Error::<T>::AdminAccountNotSet)?;
            ensure!(who == admin, Error::<T>::SenderNotAdmin);

            <GasFeeRecipientAccount<T>>::mutate(|a| *a = Some(recipient.clone()));
            Self::deposit_event(Event::GasFeeRecipientSet { new_account: recipient });

            Ok(())
        }

        /// Set the base gas fee
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::set_base_gas_fee())]
        pub fn set_base_gas_fee(origin: OriginFor<T>, base_fee: u128) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let admin = <AdminAccount<T>>::get().ok_or(Error::<T>::AdminAccountNotSet)?;
            ensure!(who == admin, Error::<T>::SenderNotAdmin);
            ensure!(base_fee > 0u128, Error::<T>::BaseGasFeeZero);

            <BaseGasFee<T>>::mutate(|a| *a = base_fee.clone());
            Self::deposit_event(Event::BaseGasFeeSet { new_base_gas_fee: base_fee });

            Ok(())
        }

        /// Set the admin account
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::set_admin_account())]
        pub fn set_admin_account(
            origin: OriginFor<T>,
            admin_account: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            <AdminAccount<T>>::mutate(|a| *a = Some(admin_account.clone()));
            Self::deposit_event(Event::AdminAccountSet { new_admin: admin_account });

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn gas_fee_recipient() -> Result<T::AccountId, Error<T>> {
            Ok(<GasFeeRecipientAccount<T>>::get().ok_or(Error::<T>::GasFeeRecipientNotSet)?)
        }

        pub fn base_gas_fee() -> u128 {
            <BaseGasFee<T>>::get()
        }
    }
}
