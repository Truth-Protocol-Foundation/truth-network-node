//! # Validators manager Pallet
//!
//! This pallet provides functionality to add/remove validators.
//!
//! The pallet is based on the Substrate session pallet and implements related traits for session
//! management when validators are added or removed.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::string::String;

pub type EthereumTransactionId = u32;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{dispatch::DispatchResult, ensure, transactional};
use pallet_session::{self as session, Config as SessionConfig};
use sp_core::{bounded::BoundedVec, ecdsa, H512};
use sp_runtime::{
    scale_info::TypeInfo,
    traits::{Convert, Member},
    DispatchError,
};
use sp_std::prelude::*;

use pallet_avn::{
    self as avn, AccountToBytesConverter, BridgeInterface, BridgeInterfaceNotification,
    NewSessionHandler, ValidatorRegistrationNotifier,
};
use sp_avn_common::{
    eth_key_actions::decompress_eth_public_key, event_types::Validator, IngressCounter,
};

pub use pallet::*;

const PALLET_ID: &'static [u8; 14] = b"author_manager";

const DEFAULT_MINIMUM_VALIDATORS_COUNT: usize = 2;

pub mod default_weights;
pub use default_weights::WeightInfo;

pub type AVN<T> = avn::Pallet<T>;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{assert_ok, pallet_prelude::*};
    use frame_system::{offchain::SendTransactionTypes, pallet_prelude::*};
    use pallet_avn::{EthereumPublicKeyChecker, MAX_VALIDATOR_ACCOUNTS};
    use sp_avn_common::bounds::MaximumValidatorsBound;
    use sp_core::ecdsa;
    use sp_runtime::AccountId32;

    #[pallet::pallet]
    pub struct Pallet<T>(PhantomData<T>);

    pub type BalanceOf = <() as frame_support::traits::Currency<AccountId32>>::Balance;

    #[pallet::config]
    pub trait Config:
        SendTransactionTypes<Call<Self>>
        + frame_system::Config
        + session::Config
        + pallet_avn::Config
        + pallet_session::historical::Config
    {
        /// Overarching event type
        type RuntimeEvent: From<Event<Self>>
            + Into<<Self as frame_system::Config>::RuntimeEvent>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type AccountToBytesConvert: AccountToBytesConverter<Self::AccountId>;

        type ValidatorRegistrationNotifier: ValidatorRegistrationNotifier<
            <Self as session::Config>::ValidatorId,
        >;

        type WeightInfo: WeightInfo;

        type BridgeInterface: BridgeInterface;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ValidatorRegistered { validator_id: T::AccountId, eth_key: ecdsa::Public },
        ValidatorDeregistered { validator_id: T::AccountId },
        ValidatorActivationStarted { validator_id: T::AccountId },
        ValidatorActionConfirmed { action_id: ActionId<T::AccountId> },
        PublishingValidatorActionOnEthereumFailed { tx_id: u32 },
        PublishingValidatorActionOnEthereumSucceeded { tx_id: u32 },
    }

    #[pallet::error]
    pub enum Error<T> {
        NoTier1EventForAddingValidator,
        NoTier1EventForRemovingValidator,
        NoValidators,
        ValidatorAlreadyExists,
        InvalidIngressCounter,
        MinimumValidatorsReached,
        ErrorEndingVotingPeriod,
        VotingSessionIsNotValid,
        ErrorSubmitCandidateTxnToTier1,
        ErrorCalculatingPrimaryValidator,
        ValidatorsActionDataNotFound,
        RemovalAlreadyRequested,
        ErrorConvertingAccountIdToValidatorId,
        SlashedValidatorIsNotFound,
        ValidatorNotFound,
        InvalidPublicKey,
        /// The ethereum public key of this validator alredy exists
        ValidatorEthKeyAlreadyExists,
        ErrorRemovingAccountFromCollators,
        MaximumValidatorsReached,
    }

    #[pallet::storage]
    #[pallet::getter(fn validator_account_ids)]
    pub type ValidatorAccountIds<T: Config> =
        StorageValue<_, BoundedVec<T::AccountId, MaximumValidatorsBound>>;

    #[pallet::storage]
    pub type ValidatorActions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        IngressCounter,
        ValidatorsActionData,
        OptionQuery,
        GetDefault,
    >;

    #[pallet::storage]
    #[pallet::getter(fn get_validator_by_eth_public_key)]
    pub type EthereumPublicKeys<T: Config> =
        StorageMap<_, Blake2_128Concat, ecdsa::Public, T::AccountId>;

    #[pallet::storage]
    #[pallet::getter(fn get_ingress_counter)]
    pub type TotalIngresses<T: Config> = StorageValue<_, IngressCounter, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub validators: Vec<(T::AccountId, ecdsa::Public)>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self { validators: Vec::<(T::AccountId, ecdsa::Public)>::new() }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            log::debug!(
                "Validators Manager Genesis build entrypoint - total validators: {}",
                self.validators.len()
            );
            for (validator_account_id, eth_public_key) in &self.validators {
                assert_ok!(<ValidatorAccountIds<T>>::try_append(validator_account_id));
                <EthereumPublicKeys<T>>::insert(eth_public_key, validator_account_id);
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::add_collator())]
        #[transactional]
        pub fn add_collator(
            origin: OriginFor<T>,
            collator_account_id: T::AccountId,
            collator_eth_public_key: ecdsa::Public,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let validator_account_ids =
                Self::validator_account_ids().ok_or(Error::<T>::NoValidators)?;
            ensure!(!validator_account_ids.is_empty(), Error::<T>::NoValidators);

            ensure!(
                !validator_account_ids.contains(&collator_account_id),
                Error::<T>::ValidatorAlreadyExists
            );
            ensure!(
                !<EthereumPublicKeys<T>>::contains_key(&collator_eth_public_key),
                Error::<T>::ValidatorEthKeyAlreadyExists
            );

            ensure!(
                ValidatorAccountIds::<T>::get().unwrap_or_default().len() <
                    (<MaximumValidatorsBound as sp_core::TypedGet>::get() as usize),
                Error::<T>::MaximumValidatorsReached
            );

            Self::register_author(&collator_account_id, &collator_eth_public_key)?;

            <ValidatorAccountIds<T>>::try_append(collator_account_id.clone())
                .map_err(|_| Error::<T>::MaximumValidatorsReached)?;
            <EthereumPublicKeys<T>>::insert(collator_eth_public_key, collator_account_id);

            return Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_validator(MAX_VALIDATOR_ACCOUNTS))]
        #[transactional]
        pub fn remove_validator(
            origin: OriginFor<T>,
            collator_account_id: T::AccountId,
        ) -> DispatchResult {
            let _ = ensure_root(origin)?;

            Self::remove_deregistered_validator(&collator_account_id)?;

            Self::deposit_event(Event::<T>::ValidatorDeregistered {
                validator_id: collator_account_id,
            });

            return Ok(())
        }
    }

    impl<T: Config> EthereumPublicKeyChecker<T::AccountId> for Pallet<T> {
        fn get_validator_for_eth_public_key(
            eth_public_key: &ecdsa::Public,
        ) -> Option<T::AccountId> {
            Self::get_validator_by_eth_public_key(eth_public_key)
        }
    }
}

#[derive(Encode, Decode, Default, Clone, Copy, PartialEq, Debug, Eq, TypeInfo, MaxEncodedLen)]
pub struct ActionId<AccountId: Member> {
    pub action_account_id: AccountId,
    pub ingress_counter: IngressCounter,
}

#[derive(Copy, Clone, Eq, PartialEq, Encode, Decode, Debug, TypeInfo, MaxEncodedLen)]
pub enum ValidatorsActionType {
    /// Validator has asked to leave voluntarily
    Resignation,
    /// Validator is being forced to leave due to a malicious behaviour
    Slashed,
    /// Validator activates himself after he joins an active session
    Activation,
    /// Default value
    Unknown,
}

impl Default for ValidatorsActionType {
    fn default() -> Self {
        ValidatorsActionType::Unknown
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Encode, Decode, Debug, TypeInfo, MaxEncodedLen)]
pub enum ValidatorsActionStatus {
    /// Validator enters this state immediately within removal extrinsic, ready for session
    /// confirmation
    AwaitingConfirmation,
    /// Validator enters this state within session handler, ready for signing and sending to T1
    Confirmed,
    /// Validator enters this state once T1 action request is sent, ready to be removed from
    /// hashmap
    Actioned,
    /// Validator enters this state once T1 event processed
    None,
}

impl Default for ValidatorsActionStatus {
    fn default() -> Self {
        ValidatorsActionStatus::None
    }
}

#[derive(Encode, Decode, Default, Clone, PartialEq, Debug, TypeInfo, MaxEncodedLen)]
pub struct ValidatorsActionData {
    pub status: ValidatorsActionStatus,
    pub eth_transaction_id: EthereumTransactionId,
    pub action_type: ValidatorsActionType,
}

impl ValidatorsActionData {
    fn new(
        status: ValidatorsActionStatus,
        eth_transaction_id: EthereumTransactionId,
        action_type: ValidatorsActionType,
    ) -> Self {
        return ValidatorsActionData { status, eth_transaction_id, action_type }
    }
}

impl ValidatorsActionType {
    fn is_deregistration(&self) -> bool {
        match self {
            ValidatorsActionType::Resignation => true,
            ValidatorsActionType::Slashed => true,
            _ => false,
        }
    }

    fn is_activation(&self) -> bool {
        match self {
            ValidatorsActionType::Activation => true,
            _ => false,
        }
    }
}

impl<AccountId: Member + Encode> ActionId<AccountId> {
    fn new(action_account_id: AccountId, ingress_counter: IngressCounter) -> Self {
        return ActionId::<AccountId> { action_account_id, ingress_counter }
    }
}

impl<T: Config> Pallet<T> {
    fn start_activation_for_registered_validator(
        registered_validator: &T::AccountId,
        tx_id: EthereumTransactionId,
    ) {
        let ingress_counter = Self::get_ingress_counter() + 1;

        TotalIngresses::<T>::put(ingress_counter);
        <ValidatorActions<T>>::insert(
            registered_validator,
            ingress_counter,
            ValidatorsActionData::new(
                ValidatorsActionStatus::AwaitingConfirmation,
                tx_id,
                ValidatorsActionType::Activation,
            ),
        );
    }

    fn register_author(
        collator_account_id: &T::AccountId,
        collator_eth_public_key: &ecdsa::Public,
    ) -> DispatchResult {
        let decompressed_eth_public_key = decompress_eth_public_key(*collator_eth_public_key)
            .map_err(|_| Error::<T>::InvalidPublicKey)?;
        let validator_id_bytes =
            <T as pallet::Config>::AccountToBytesConvert::into_bytes(collator_account_id);
        let function_name = b"addAuthor";

        let params = vec![
            (b"bytes".to_vec(), decompressed_eth_public_key.to_fixed_bytes().to_vec()),
            (b"bytes32".to_vec(), validator_id_bytes.to_vec()),
        ];
        let tx_id = <T as pallet::Config>::BridgeInterface::publish(
            function_name,
            &params,
            PALLET_ID.to_vec(),
        )
        .map_err(|e| DispatchError::Other(e.into()))?;

        let new_collator_id =
            <T as SessionConfig>::ValidatorIdOf::convert(collator_account_id.clone())
                .ok_or(Error::<T>::ErrorConvertingAccountIdToValidatorId)?;

        Self::start_activation_for_registered_validator(collator_account_id, tx_id);
        T::ValidatorRegistrationNotifier::on_validator_registration(&new_collator_id);
        Self::deposit_event(Event::<T>::ValidatorRegistered {
            validator_id: collator_account_id.clone(),
            eth_key: *collator_eth_public_key,
        });
        Ok(())
    }

    fn get_ethereum_public_key_if_exists(account_id: &T::AccountId) -> Option<ecdsa::Public> {
        return <EthereumPublicKeys<T>>::iter()
            .filter(|(_, acc)| acc == account_id)
            .map(|(pk, _)| pk)
            .nth(0)
    }

    fn remove_ethereum_public_key_if_required(validator_id: &T::AccountId) {
        let public_key_to_remove = Self::get_ethereum_public_key_if_exists(&validator_id);
        if let Some(public_key_to_remove) = public_key_to_remove {
            <EthereumPublicKeys<T>>::remove(public_key_to_remove);
        }
    }

    /// We assume the full public key doesn't have the `04` prefix
    #[allow(dead_code)]
    fn compress_eth_public_key(full_public_key: H512) -> ecdsa::Public {
        let mut compressed_public_key = [0u8; 33];

        // Take bytes 0..32 from the full plublic key ()
        compressed_public_key[1..=32].copy_from_slice(&full_public_key.0[0..32]);
        // If the last byte of the full public key is even, prefix compresssed public key with 2,
        // otherwise prefix with 3
        compressed_public_key[0] = if full_public_key.0[63] % 2 == 0 { 2u8 } else { 3u8 };

        return ecdsa::Public::from_raw(compressed_public_key)
    }

    fn remove(
        validator_id: &T::AccountId,
        ingress_counter: IngressCounter,
        action_type: ValidatorsActionType,
        eth_public_key: ecdsa::Public,
    ) -> DispatchResult {
        let mut validator_account_ids =
            Self::validator_account_ids().ok_or(Error::<T>::NoValidators)?;

        ensure!(
            Self::get_ingress_counter() + 1 == ingress_counter,
            Error::<T>::InvalidIngressCounter
        );
        ensure!(
            validator_account_ids.len() > DEFAULT_MINIMUM_VALIDATORS_COUNT,
            Error::<T>::MinimumValidatorsReached
        );
        ensure!(
            !<ValidatorActions<T>>::contains_key(validator_id, ingress_counter),
            Error::<T>::RemovalAlreadyRequested
        );

        let maybe_validator_index = validator_account_ids.iter().position(|v| v == validator_id);
        if maybe_validator_index.is_none() {
            // Exit early if deregistration is not in the system. As dicussed, we don't want to give
            // any feedback if the validator is not found.
            return Ok(())
        }

        let index_of_validator_to_remove = maybe_validator_index.expect("checked for none already");

        let decompressed_eth_public_key =
            decompress_eth_public_key(eth_public_key).map_err(|_| Error::<T>::InvalidPublicKey)?;

        let validator_id_bytes =
            <T as pallet::Config>::AccountToBytesConvert::into_bytes(validator_id);

        let function_name = b"removeAuthor";
        let params = vec![
            (b"bytes32".to_vec(), validator_id_bytes.to_vec()),
            (b"bytes".to_vec(), decompressed_eth_public_key.to_fixed_bytes().to_vec()),
        ];
        let tx_id = <T as pallet::Config>::BridgeInterface::publish(
            function_name,
            &params,
            PALLET_ID.to_vec(),
        )
        .map_err(|e| DispatchError::Other(e.into()))?;

        TotalIngresses::<T>::put(ingress_counter);
        <ValidatorActions<T>>::insert(
            validator_id,
            ingress_counter,
            ValidatorsActionData::new(
                ValidatorsActionStatus::AwaitingConfirmation,
                tx_id,
                action_type,
            ),
        );
        validator_account_ids.swap_remove(index_of_validator_to_remove);
        <ValidatorAccountIds<T>>::put(validator_account_ids);

        Ok(())
    }

    fn remove_deregistered_validator(resigned_validator: &T::AccountId) -> DispatchResult {
        // Take key from map.
        let t1_eth_public_key = match Self::get_ethereum_public_key_if_exists(resigned_validator) {
            Some(eth_public_key) => eth_public_key,
            _ => Err(Error::<T>::ValidatorNotFound)?,
        };

        let ingress_counter = Self::get_ingress_counter() + 1;
        return Self::remove(
            resigned_validator,
            ingress_counter,
            ValidatorsActionType::Resignation,
            t1_eth_public_key,
        )
    }

    fn validator_permanently_removed(
        active_validators: &Vec<Validator<T::AuthorityId, T::AccountId>>,
        disabled_validators: &Vec<T::AccountId>,
        deregistered_validator: &T::AccountId,
    ) -> bool {
        // If the validator exists in either vectors then they have not been removed from the
        // session
        return !active_validators.iter().any(|v| &v.account_id == deregistered_validator) &&
            !disabled_validators.iter().any(|v| v == deregistered_validator)
    }

    fn clean_up_collator_data(action_account_id: T::AccountId, ingress_counter: IngressCounter) {
        <ValidatorActions<T>>::mutate(
            &action_account_id,
            ingress_counter,
            |validators_action_data_maybe| {
                if let Some(validators_action_data) = validators_action_data_maybe {
                    validators_action_data.status = ValidatorsActionStatus::Confirmed
                }
            },
        );
        Self::remove_ethereum_public_key_if_required(&action_account_id);

        let action_id = ActionId::new(action_account_id, ingress_counter);

        Self::deposit_event(Event::<T>::ValidatorActionConfirmed { action_id });
    }
}

impl<T: Config> NewSessionHandler<T::AuthorityId, T::AccountId> for Pallet<T> {
    fn on_genesis_session(_validators: &Vec<Validator<T::AuthorityId, T::AccountId>>) {
        log::trace!("Validators manager on_genesis_session");
    }

    fn on_new_session(
        _changed: bool,
        active_validators: &Vec<Validator<T::AuthorityId, T::AccountId>>,
        disabled_validators: &Vec<T::AccountId>,
    ) {
        log::trace!("Validators manager on_new_session");
        if <ValidatorActions<T>>::iter().count() > 0 {
            for (action_account_id, ingress_counter, validators_action_data) in
                <ValidatorActions<T>>::iter()
            {
                if validators_action_data.status == ValidatorsActionStatus::AwaitingConfirmation &&
                    validators_action_data.action_type.is_deregistration() &&
                    Self::validator_permanently_removed(
                        &active_validators,
                        &disabled_validators,
                        &action_account_id,
                    )
                {
                    Self::clean_up_collator_data(action_account_id, ingress_counter);
                } else if validators_action_data.status ==
                    ValidatorsActionStatus::AwaitingConfirmation &&
                    validators_action_data.action_type.is_activation()
                {
                    <ValidatorActions<T>>::mutate(
                        &action_account_id,
                        ingress_counter,
                        |validators_action_data_maybe| {
                            if let Some(validators_action_data) = validators_action_data_maybe {
                                validators_action_data.status = ValidatorsActionStatus::Confirmed
                            }
                        },
                    );

                    Self::deposit_event(Event::<T>::ValidatorActivationStarted {
                        validator_id: action_account_id.clone(),
                    });
                }
            }
        }
    }
}

impl<T: Config> session::SessionManager<T::AccountId> for Pallet<T> {
    fn new_session(new_index: u32) -> Option<Vec<T::AccountId>> {
        // Retrieve the authors from storage
        let authors_option = ValidatorAccountIds::<T>::get();

        if let Some(authors) = authors_option {
            if authors.is_empty() {
                // We never want to pass an empty set of collators. This would brick the chain.
                log::error!("💥 keeping old session because of empty collator set!");
                None
            } else {
                log::debug!(
                    "[AUTH-MGR] assembling new authors for new session {} with these authors {:#?} at #{:?}",
                    new_index,
                    authors,
                    <frame_system::Pallet<T>>::block_number(),
                );

                Some(authors.into())
            }
        } else {
            // Handle the case where no authors are present in storage
            log::error!("💥 keeping old session because no authors found in storage!");
            None
        }
    }

    fn end_session(_end_index: u32) {
        // nothing to do here
    }

    fn start_session(_start_index: u32) {
        // nothing to do here
    }
}

impl<T: Config> BridgeInterfaceNotification for Pallet<T> {
    fn process_result(tx_id: u32, caller_id: Vec<u8>, succeeded: bool) -> DispatchResult {
        // TODO: Update data structure to use tx_id as key
        if caller_id == PALLET_ID.to_vec() {
            if succeeded {
                log::info!(
                    "✅  Transaction with ID {} was successfully published to Ethereum.",
                    tx_id
                );
                Self::deposit_event(Event::<T>::PublishingValidatorActionOnEthereumSucceeded {
                    tx_id,
                });
            } else {
                log::error!("❌ Transaction with ID {} failed to publish to Ethereum.", tx_id);
                Self::deposit_event(Event::<T>::PublishingValidatorActionOnEthereumFailed {
                    tx_id,
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod benchmarking;
