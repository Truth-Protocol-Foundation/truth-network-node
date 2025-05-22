#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

pub mod asset_registry;
pub mod fees;
pub mod third_party_weights;
use asset_registry::CustomAssetProcessor;

use codec::{Decode, Encode};
use core::cmp::Ordering;
use orml_traits::parameter_type_with_key;
use pallet_avn::sr25519::AuthorityId as AvnId;
pub use pallet_avn_proxy::{Event as AvnProxyEvent, ProvableProxy};
use pallet_grandpa::AuthorityId as GrandpaId;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use pallet_node_manager::sr25519::AuthorityId as NodeManagerKeyId;
use pallet_session::historical as pallet_session_historical;
use scale_info::TypeInfo;
use smallvec::smallvec;
use sp_api::impl_runtime_apis;
use sp_arithmetic::FixedU128;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_runtime::{
    create_runtime_str, generic, impl_opaque_keys,
    traits::{AccountIdLookup, BlakeTwo256, Block as BlockT, ConvertInto, NumberFor, One},
    transaction_validity::{TransactionPriority, TransactionSource, TransactionValidity},
    ApplyExtrinsicResult, FixedPointNumber, Percent, RuntimeAppPublic,
};
use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

pub mod proxy_config;
use proxy_config::AvnProxyConfig;

pub use prediction_market_primitives::{constants::*, types::*};

pub use common_primitives::{
    constants::{
        currency::*, BLOCKS_PER_DAY, BLOCKS_PER_HOUR, BLOCKS_PER_YEAR, MILLISECS_PER_BLOCK,
        NODE_MANAGER_PALLET_ID, SLOT_DURATION,
    },
    types::{AccountId, Balance, BlockNumber},
};

// A few exports that help ease life for downstream crates.
pub use frame_support::{
    construct_runtime,
    dispatch::DispatchClass,
    parameter_types,
    traits::{
        AsEnsureOriginWithArg, ConstBool, ConstU128, ConstU32, ConstU64, ConstU8, Contains,
        Currency, EitherOfDiverse, EnsureOrigin, Imbalance, KeyOwnerProofSystem, LockIdentifier,
        OnUnbalanced, PrivilegeCmp, Randomness, StorageInfo,
    },
    weights::{
        constants::{
            BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_REF_TIME_PER_SECOND,
        },
        ConstantMultiplier, IdentityFee, Weight, WeightToFeeCoefficient, WeightToFeeCoefficients,
        WeightToFeePolynomial,
    },
    PalletId, StorageValue,
};
pub use frame_system::{
    limits::{BlockLength, BlockWeights},
    Call as SystemCall, EnsureRoot, EnsureSigned, EnsureSignedBy,
};
use pallet_avn_transaction_payment::AvnCurrencyAdapter;
pub use pallet_balances::Call as BalancesCall;
use pallet_collective::{EnsureProportionMoreThan, PrimeDefaultVote};
pub use pallet_timestamp::Call as TimestampCall;
use pallet_transaction_payment::{ConstFeeMultiplier, Multiplier};
use sp_avn_common::{
    event_discovery::{AdditionalEvents, EthBridgeEventsFilter, EthereumEventsFilterTrait},
    event_types::ValidEvents,
    InnerCallValidator, Proof,
};

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use sp_runtime::{Perbill, Permill, RuntimeDebug};
use sp_std::collections::btree_set::BTreeSet;

type AdvisoryCommitteeInstance = pallet_collective::Instance1;

type EnsureRootOrMoreThanHalfAdvisoryCommittee = EitherOfDiverse<
    EnsureRoot<AccountId>,
    EnsureProportionMoreThan<AccountId, AdvisoryCommitteeInstance, 1, 2>,
>;

type EnsureRootOrMoreThanOneThirdAdvisoryCommittee = EitherOfDiverse<
    EnsureRoot<AccountId>,
    EnsureProportionMoreThan<AccountId, AdvisoryCommitteeInstance, 1, 3>,
>;

// More than 66%
type EnsureRootOrMoreThanTwoThirdsAdvisoryCommittee = EitherOfDiverse<
    EnsureRoot<AccountId>,
    EnsureProportionMoreThan<AccountId, AdvisoryCommitteeInstance, 2, 3>,
>;

pub struct EnsureConfigAdmin;
impl EnsureOrigin<RuntimeOrigin> for EnsureConfigAdmin {
    type Success = AccountId;

    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        let origin = o.clone();
        if let Ok(who) = EnsureSigned::<AccountId>::ensure_origin(o.clone()) {
            if let Ok(admin) = PalletConfig::config_admin() {
                if who == admin {
                    return Ok(who);
                }
            }
        }

        Err(origin)
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        let admin = PalletConfig::config_admin().map_err(|_| ())?;
        Ok(RuntimeOrigin::from(frame_system::RawOrigin::Signed(admin)))
    }
}

pub type EnsureAdminOrRoot = EitherOfDiverse<EnsureConfigAdmin, EnsureRoot<AccountId>>;

// Accounts protected from being deleted due to a too low amount of funds.
pub struct DustRemovalWhitelist;

impl Contains<AccountId> for DustRemovalWhitelist
where
    frame_support::PalletId: sp_runtime::traits::AccountIdConversion<AccountId>,
{
    fn contains(ai: &AccountId) -> bool {
        let pallets = vec![
            AuthorizedPalletId::get(),
            CourtPalletId::get(),
            GlobalDisputesPalletId::get(),
            HybridRouterPalletId::get(),
            OrderbookPalletId::get(),
            PmPalletId::get(),
            TreasuryPalletId::get(),
        ];

        if let Some(pallet_id) = frame_support::PalletId::try_from_sub_account::<u128>(ai) {
            return pallets.contains(&pallet_id.0);
        }

        for pallet_id in pallets {
            let pallet_acc: AccountId = pallet_id.into_account_truncating();

            if pallet_acc == *ai {
                return true;
            }
        }

        false
    }
}

impl_fee_types!();

/// ORML adapter
pub type BasicCurrencyAdapter<R, B> =
    orml_currencies::BasicCurrencyAdapter<R, B, OrmlAmount, Balance>;
pub type CurrencyId = Asset<MarketId>;

pub type NegativeImbalance<T> = <pallet_balances::Pallet<T> as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

pub struct Treasury<R>(sp_std::marker::PhantomData<R>);
impl<R> OnUnbalanced<NegativeImbalance<R>> for Treasury<R>
where
    R: pallet_balances::Config + pallet_token_manager::Config,
    <R as frame_system::Config>::AccountId: From<AccountId>,
    <R as frame_system::Config>::AccountId: Into<AccountId>,
    <R as frame_system::Config>::RuntimeEvent: From<pallet_balances::Event<R>>,
{
    fn on_nonzero_unbalanced(amount: NegativeImbalance<R>) {
        let recipient: <R as frame_system::Config>::AccountId = PalletConfig::gas_fee_recipient()
            .map(Into::into)
            .unwrap_or_else(|_| <pallet_token_manager::Pallet<R>>::compute_treasury_account_id());

        <pallet_balances::Pallet<R>>::resolve_creating(&recipient, amount);
    }
}

pub struct DealWithFees<R>(sp_std::marker::PhantomData<R>);
impl<R> OnUnbalanced<NegativeImbalance<R>> for DealWithFees<R>
where
    R: pallet_balances::Config + pallet_token_manager::Config,
    <R as frame_system::Config>::AccountId: From<AccountId>,
    <R as frame_system::Config>::AccountId: Into<AccountId>,
    <R as frame_system::Config>::RuntimeEvent: From<pallet_balances::Event<R>>,
{
    fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalance<R>>) {
        if let Some(mut fees) = fees_then_tips.next() {
            if let Some(tips) = fees_then_tips.next() {
                tips.merge_into(&mut fees);
            }

            // 100% of fees + tips goes to the treasury
            <Treasury<R> as OnUnbalanced<_>>::on_unbalanced(fees);
        }
    }
}

/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
/// node's balance type.
///
/// This should typically create a mapping between the following ranges:
///   - `[0, MAXIMUM_BLOCK_WEIGHT]`
///   - `[Balance::min, Balance::max]`
///
/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
///   - Setting it to `0` will essentially disable the weight fee.
///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
    type Balance = Balance;
    fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
        // We adjust the fee conversion so that a simple token transfer
        // direct to chain costs base_fee TRUU.
        let base_fee = PalletConfig::base_gas_fee();

        // The magic number (2.380951) is the result of :
        // setting p = 50 * MILLI_BASE, the cost of a simple transfer was 119.04775 milli TRUU
        // (visual observation on polkadot.js). magic_number = 119.04775 / 50 = 2.380951
        let factor = FixedU128::saturating_from_rational(1_000_000u128, 2_380_951u128);

        let p = factor.saturating_mul_int(base_fee);
        let q = Balance::from(ExtrinsicBaseWeight::get().ref_time());
        smallvec![WeightToFeeCoefficient {
            degree: 1,
            negative: false,
            coeff_frac: Perbill::from_rational(p % q, q),
            coeff_integer: p / q,
        }]
    }
}

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
    use super::*;

    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

    /// Opaque block header type.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    /// Opaque block identifier type.
    pub type BlockId = generic::BlockId<Block>;

    impl_opaque_keys! {
        pub struct SessionKeys {
            pub aura: Aura,
            pub grandpa: Grandpa,
            pub authority_discovery: AuthorityDiscovery,
            pub im_online: ImOnline,
            pub avn: Avn,
        }
    }
}

// To learn more about runtime versioning, see:
// https://docs.substrate.io/main-docs/build/upgrade#runtime-versioning
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("tnf-node"),
    impl_name: create_runtime_str!("tnf-node"),
    authoring_version: 1,
    // The version of the runtime specification. A full node will not attempt to use its native
    //   runtime in substitute for the on-chain Wasm runtime unless all of `spec_name`,
    //   `spec_version`, and `authoring_version` are the same between Wasm and native.
    // This value is set to 100 to notify Polkadot-JS App (https://polkadot.js.org/apps) to use
    //   the compatible custom types.
    spec_version: 25,
    impl_version: 0,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
    state_version: 1,
};

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

/// We assume that ~10% of the block weight is consumed by `on_initialize` handlers.
/// This is used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(10);
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2 seconds of compute with a 6 second average block time, with maximum proof size.
const MAXIMUM_BLOCK_WEIGHT: Weight =
    Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND.saturating_mul(2), u64::MAX);

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
    pub const Version: RuntimeVersion = VERSION;

    pub RuntimeBlockLength: BlockLength =
        BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
    pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
        .base_block(BlockExecutionWeight::get())
        .for_class(DispatchClass::all(), |weights| {
            weights.base_extrinsic = ExtrinsicBaseWeight::get();
        })
        .for_class(DispatchClass::Normal, |weights| {
            weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
        })
        .for_class(DispatchClass::Operational, |weights| {
            weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
            // Operational transactions have some extra reserved space, so that they
            // are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
            weights.reserved = Some(
                MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
            );
        })
        .avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
        .build_or_panic();

    pub const SS58Prefix: u16= 42;
    pub const MaxAuthorities: u32 = 32;
    pub const ReportLongevity: u64 = Period::get() as u64 * 2u64;
}

// Configure FRAME pallets to include in runtime.

impl frame_system::Config for Runtime {
    /// The basic call filter to use in dispatchable.
    type BaseCallFilter = frame_support::traits::Everything;
    /// The block type for the runtime.
    type Block = Block;
    /// Block & extrinsics weights: base values and limits.
    type BlockWeights = RuntimeBlockWeights;
    /// The maximum length of a block (in bytes).
    type BlockLength = RuntimeBlockLength;
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The aggregated dispatch type that is available for extrinsics.
    type RuntimeCall = RuntimeCall;
    /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
    type Lookup = AccountIdLookup<AccountId, ()>;
    /// The type for storing how many extrinsics an account has signed.
    type Nonce = Nonce;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// The hashing algorithm used.
    type Hashing = BlakeTwo256;
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    /// The ubiquitous origin type.
    type RuntimeOrigin = RuntimeOrigin;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// The weight of database operations that the runtime can invoke.
    type DbWeight = RocksDbWeight;
    /// Version of the runtime.
    type Version = Version;
    /// Converts a module to the index of the module in `construct_runtime!`.
    ///
    /// This type is being generated by `construct_runtime!`.
    type PalletInfo = PalletInfo;
    /// What to do if a new account is created.
    type OnNewAccount = ();
    /// What to do if an account is fully reaped from the system.
    type OnKilledAccount = ();
    /// The data to be stored in an account.
    type AccountData = pallet_balances::AccountData<Balance>;
    /// Weight information for the extrinsics of this pallet.
    type SystemWeightInfo = ();
    /// This is used as an identifier of the chain. 42 is the generic substrate prefix.
    type SS58Prefix = SS58Prefix;
    /// The set code logic, just the default since we're not a parachain.
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_aura::Config for Runtime {
    type AuthorityId = AuraId;
    type DisabledValidators = ();
    type MaxAuthorities = MaxAuthorities;
    type AllowMultipleBlocksPerSlot = ConstBool<false>;
}

impl pallet_grandpa::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MaxAuthorities = MaxAuthorities;
    type MaxSetIdSessionEntries = ConstU64<0>;
    type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;
    type EquivocationReportSystem =
        pallet_grandpa::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
    type MaxNominators = MaxAuthorities;
}

parameter_types! {
    pub const MinimumPeriodValue: u64 = SLOT_DURATION / 2;
}
// Timestamp
/// Custom getter for minimum timestamp delta.
/// This ensures that consensus systems like Aura don't break assertions
/// in a benchmark environment
pub struct MinimumPeriod;
impl MinimumPeriod {
    /// Returns the value of this parameter type.
    pub fn get() -> u64 {
        #[cfg(feature = "runtime-benchmarks")]
        {
            use frame_benchmarking::benchmarking::get_whitelist;
            // Should that condition be true, we can assume that we are in a benchmark environment.
            if !get_whitelist().is_empty() {
                return u64::MAX;
            }
        }

        MinimumPeriodValue::get()
    }
}
impl<I: From<u64>> frame_support::traits::Get<I> for MinimumPeriod {
    fn get() -> I {
        I::from(Self::get())
    }
}
impl frame_support::traits::TypedGet for MinimumPeriod {
    type Type = u64;
    fn get() -> u64 {
        Self::get()
    }
}
impl pallet_timestamp::Config for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = Moment;
    type OnTimestampSet = Aura;
    type MinimumPeriod = MinimumPeriod; //ConstU64<{ SLOT_DURATION / 2 }>
    type WeightInfo = ();
}

/// Existential deposit.
pub const NATIVE_EXISTENTIAL_DEPOSIT: Balance = 0;
pub const DEFAULT_EXISTENTIAL_DEPOSIT: Balance = 10 * MICRO_BASE;

impl pallet_balances::Config for Runtime {
    type MaxLocks = ConstU32<50>;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    /// The type for recording an account's balance.
    type Balance = Balance;
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<NATIVE_EXISTENTIAL_DEPOSIT>;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type RuntimeHoldReason = ();
    type MaxHolds = ();
}

parameter_types! {
    // TNF want to have fixed fees regardless of chain usage
    pub FeeMultiplier: Multiplier = Multiplier::one();
    // This value was adjusted so that the length fee of an extrinsic is roughly
    // in line with the weight fees.
    // An extrinsic usually has a payload with a few hundred bytes, and its weight
    // fee should be of a few milli TRUU.
    // In consequence TransactionByteFee should be set at a few MICRO_BASE.
    // The actual value here was chosen to be a round number so that a Token Transfer be around 2mTRU, and a TRUU transfer be around 1 mTRUU.
    pub const TransactionByteFee: Balance = 5 * MICRO_BASE;
    pub const OperationalFeeMultiplier: u8 = 5;
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = AvnCurrencyAdapter<Balances, DealWithFees<Runtime>>;
    type OperationalFeeMultiplier = OperationalFeeMultiplier;
    type WeightToFee = WeightToFee;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type FeeMultiplierUpdate = ConstFeeMultiplier<FeeMultiplier>;
}

impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

impl pallet_avn::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AuthorityId = AvnId;
    type EthereumPublicKeyChecker = AuthorsManager;
    type NewSessionHandler = ();
    type DisabledValidatorChecker = ();
    type WeightInfo = pallet_avn::default_weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const Period: u32 = 12 * BLOCKS_PER_HOUR;
    pub const Offset: u32 = 0;
}

impl pallet_session::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = <Self as frame_system::Config>::AccountId;
    // we don't have stash and controller, thus we don't need the convert as well.
    type ValidatorIdOf = ConvertInto;
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionManager = AuthorsManager;
    type SessionHandler =
        <opaque::SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
    type Keys = opaque::SessionKeys;
    type WeightInfo = ();
}

impl pallet_authorship::Config for Runtime {
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
    type EventHandler = ImOnline;
}

impl pallet_authority_discovery::Config for Runtime {
    type MaxAuthorities = MaxAuthorities;
}

impl pallet_avn_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type KnownUserOrigin = EnsureAdminOrRoot;
    type WeightInfo = pallet_avn_transaction_payment::default_weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    // TODO [TYPE: review][PRI: medium][JIRA: SYS-358]: Configurable in eth-events pallet?
    pub const MinEthBlockConfirmation: u64 = 20;
}

pub struct EthBridgeTnfRuntimeEventsFilter;
impl EthereumEventsFilterTrait for EthBridgeTnfRuntimeEventsFilter {
    fn get() -> EthBridgeEventsFilter {
        let allowed_events: BTreeSet<ValidEvents> = vec![
            ValidEvents::AvtLowerClaimed,
            ValidEvents::Lifted,
            ValidEvents::LiftedToPredictionMarket,
        ]
        .into_iter()
        .collect();

        EthBridgeEventsFilter::try_from(allowed_events).unwrap_or_default()
    }
}

impl pallet_eth_bridge::Config for Runtime {
    type MaxQueuedTxRequests = ConstU32<100>;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type MinEthBlockConfirmation = MinEthBlockConfirmation;
    type ProcessedEventsChecker = ProcessedEventCustodian;
    type AccountToBytesConvert = Avn;
    type TimeProvider = pallet_timestamp::Pallet<Runtime>;
    type ReportCorroborationOffence = ();
    type WeightInfo = pallet_eth_bridge::default_weights::SubstrateWeight<Runtime>;
    type BridgeInterfaceNotification = (Summary, TokenManager, PredictionMarkets);
    type EthereumEventsFilter = EthBridgeTnfRuntimeEventsFilter;
}

parameter_types! {
    pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
    pub const MaxKeys: u32 = 10_000;
    pub const MaxPeerInHeartbeats: u32 = 10_000;
}

impl pallet_im_online::Config for Runtime {
    type AuthorityId = ImOnlineId;
    type RuntimeEvent = RuntimeEvent;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type ValidatorSet = Historical;
    type ReportUnresponsiveness = Offences;
    type UnsignedPriority = ImOnlineUnsignedPriority;
    type WeightInfo = pallet_im_online::weights::SubstrateWeight<Runtime>;
    type MaxKeys = MaxKeys;
    type MaxPeerInHeartbeats = MaxPeerInHeartbeats;
}

impl pallet_session::historical::Config for Runtime {
    type FullIdentification = AccountId;
    type FullIdentificationOf = ConvertInto;
}

impl pallet_offences::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
    type OnOffenceHandler = ();
}

// Multisig pallet config start
parameter_types! {
    // One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
    pub const DepositBase: Balance = deposit(1, 88);
    // Additional storage item size of 32 bytes.
    pub const DepositFactor: Balance = deposit(0, 32);
    pub const MaxSignatories: u32 = 100;
}

impl pallet_multisig::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = ConstU32<100>;
    type WeightInfo = pallet_multisig::weights::SubstrateWeight<Runtime>;
}

impl pallet_ethereum_events::Config for Runtime {
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type ProcessedEventHandler = (TokenManager, NftManager, PredictionMarkets);
    type MinEthBlockConfirmation = MinEthBlockConfirmation;
    type Public = <Signature as sp_runtime::traits::Verify>::Signer;
    type Signature = Signature;
    type ReportInvalidEthereumLog = Offences;
    type WeightInfo = pallet_ethereum_events::default_weights::SubstrateWeight<Runtime>;
    type EthereumEventsFilter = EthBridgeTnfRuntimeEventsFilter;
    type ProcessedEventsChecker = ProcessedEventCustodian;
}

impl pallet_token_manager::pallet::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type TokenBalance = Balance;
    type TokenId = EthAddress;
    type ProcessedEventsChecker = ProcessedEventCustodian;
    type Public = <Signature as sp_runtime::traits::Verify>::Signer;
    type Signature = Signature;
    type OnGrowthLiftedHandler = ();
    type TreasuryGrowthPercentage = TreasuryGrowthPercentage;
    type AvnTreasuryPotId = AvnTreasuryPotId;
    type WeightInfo = pallet_token_manager::default_weights::SubstrateWeight<Runtime>;
    type Scheduler = Scheduler;
    type Preimages = Preimage;
    type PalletsOrigin = OriginCaller;
    type BridgeInterface = EthBridge;
}

impl pallet_avn_proxy::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type Public = <Signature as sp_runtime::traits::Verify>::Signer;
    type Signature = Signature;
    type ProxyConfig = AvnProxyConfig;
    type WeightInfo = pallet_avn_proxy::default_weights::SubstrateWeight<Runtime>;
    type FeeHandler = TokenManager;
    type Token = EthAddress;
}

impl pallet_nft_manager::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type ProcessedEventsChecker = ProcessedEventCustodian;
    type Public = <Signature as sp_runtime::traits::Verify>::Signer;
    type Signature = Signature;
    type BatchBound = pallet_nft_manager::BatchNftBound;
    type WeightInfo = pallet_nft_manager::default_weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const MaxScheduledPerBlock: u32 = 50;

    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) *
        RuntimeBlockWeights::get().max_block;
}

/// Used the compare the privilege of an origin inside the scheduler.
pub struct OriginPrivilegeCmp;

impl PrivilegeCmp<OriginCaller> for OriginPrivilegeCmp {
    fn cmp_privilege(left: &OriginCaller, right: &OriginCaller) -> Option<Ordering> {
        if left == right {
            return Some(Ordering::Equal);
        }

        match (left, right) {
            // Root is greater than anything.
            (OriginCaller::system(frame_system::RawOrigin::Root), _) => Some(Ordering::Greater),
            // For every other origin we don't care, as they are not used for `ScheduleOrigin`.
            _ => None,
        }
    }
}

impl pallet_scheduler::Config for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeEvent = RuntimeEvent;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = MaximumSchedulerWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = MaxScheduledPerBlock;
    type WeightInfo = pallet_scheduler::weights::SubstrateWeight<Runtime>;
    type OriginPrivilegeCmp = OriginPrivilegeCmp;
    type Preimages = Preimage;
}

impl pallet_preimage::Config for Runtime {
    type WeightInfo = pallet_preimage::weights::SubstrateWeight<Runtime>;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type ManagerOrigin = EnsureRoot<AccountId>;
    type BaseDeposit = ConstU128<{ 5 * BASE }>;
    type ByteDeposit = ConstU128<{ 100 * MICRO_BASE }>;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
    RuntimeCall: From<C>,
{
    type Extrinsic = UncheckedExtrinsic;
    type OverarchingCall = RuntimeCall;
}
parameter_types! {
    pub const AdvanceSlotGracePeriod: BlockNumber = 5;
    pub const MinBlockAge: BlockNumber = 5;
    pub const AvnTreasuryPotId: PalletId = TREASURY_PALLET_ID;
    pub const TreasuryGrowthPercentage: Perbill = Perbill::from_percent(100);
    pub const EthereumInstanceId: u8 = 1u8;
    pub const EthAutoSubmitSummaries: bool = true;
    pub const AvnAutoSubmitSummaries: bool = false;
    pub const AvnInstanceId: u8 = 2u8;
}

pub type EthSummary = pallet_summary::Instance1;
impl pallet_summary::Config<EthSummary> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AdvanceSlotGracePeriod = AdvanceSlotGracePeriod;
    type MinBlockAge = MinBlockAge;
    type AccountToBytesConvert = Avn;
    type ReportSummaryOffence = Offences;
    type WeightInfo = pallet_summary::default_weights::SubstrateWeight<Runtime>;
    type BridgeInterface = EthBridge;
    type AutoSubmitSummaries = EthAutoSubmitSummaries;
    type InstanceId = EthereumInstanceId;
}

pub type AvnAnchorSummary = pallet_summary::Instance2;
impl pallet_summary::Config<AvnAnchorSummary> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AdvanceSlotGracePeriod = AdvanceSlotGracePeriod;
    type MinBlockAge = MinBlockAge;
    type AccountToBytesConvert = Avn;
    type ReportSummaryOffence = Offences;
    type WeightInfo = pallet_summary::default_weights::SubstrateWeight<Runtime>;
    type BridgeInterface = EthBridge;
    type AutoSubmitSummaries = AvnAutoSubmitSummaries;
    type InstanceId = AvnInstanceId;
}

impl pallet_authors_manager::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AccountToBytesConvert = Avn;
    type ValidatorRegistrationNotifier = (); //AvnOffenceHandler;
    type WeightInfo = pallet_authors_manager::default_weights::SubstrateWeight<Runtime>;
    type BridgeInterface = EthBridge;
}

parameter_types! {
    pub const NodeManagerPalletId: PalletId = NODE_MANAGER_PALLET_ID;
}

impl pallet_node_manager::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type SignerId = NodeManagerKeyId;
    type Currency = Balances;
    type RewardPotId = NodeManagerPalletId;
    type Public = <Signature as sp_runtime::traits::Verify>::Signer;
    type Signature = Signature;
    type SignedTxLifetime = ConstU32<64>;
    type WeightInfo = pallet_node_manager::default_weights::SubstrateWeight<Runtime>;
}

impl pallet_utility::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type PalletsOrigin = OriginCaller;
    type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
}

impl pallet_config::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_config::default_weights::SubstrateWeight<Runtime>;
}

// Prediction market
impl pallet_insecure_randomness_collective_flip::Config for Runtime {}

parameter_types! {
    // Note: MaxMembers does not influence the pallet logic, but the worst-case weight estimation.
    pub const AdvisoryCommitteeMaxMembers: u32 = 100;
    // The maximum of proposals is currently u8::MAX otherwise the pallet_collective benchmark fails
    pub const AdvisoryCommitteeMaxProposals: u32 = 255;
    pub const AdvisoryCommitteeMotionDuration: BlockNumber = 3 * BLOCKS_PER_DAY;
    pub MaxProposalWeight: Weight = Perbill::from_percent(50) * RuntimeBlockWeights::get().max_block;
}

impl pallet_collective::Config<AdvisoryCommitteeInstance> for Runtime {
    type DefaultVote = PrimeDefaultVote;
    type RuntimeEvent = RuntimeEvent;
    type MaxMembers = AdvisoryCommitteeMaxMembers;
    type MaxProposals = AdvisoryCommitteeMaxProposals;
    type MaxProposalWeight = MaxProposalWeight;
    type MotionDuration = AdvisoryCommitteeMotionDuration;
    type RuntimeOrigin = RuntimeOrigin;
    type SetMembersOrigin = EnsureRoot<AccountId>;
    type Proposal = RuntimeCall;
    type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    // Authorized
    pub const AuthorizedPalletId: PalletId = AUTHORIZED_PALLET_ID;
    pub const CorrectionPeriod: BlockNumber = BLOCKS_PER_DAY;

}

impl pallet_pm_authorized::Config for Runtime {
    type AuthorizedDisputeResolutionOrigin = EnsureRootOrMoreThanHalfAdvisoryCommittee;
    type Currency = Balances;
    type CorrectionPeriod = CorrectionPeriod;
    type DisputeResolution = pallet_prediction_markets::Pallet<Runtime>;
    type RuntimeEvent = RuntimeEvent;
    type MarketCommons = MarketCommons;
    type PalletId = AuthorizedPalletId;
    type WeightInfo = pallet_pm_authorized::weights::WeightInfo<Runtime>;
}

parameter_types! {
    // Court
    /// (Slashable) Bond that is provided for overriding the last appeal.
    /// This bond increases exponentially with the number of appeals.
    /// Slashed in case the final outcome does match the appealed outcome for which the `AppealBond`
    /// was deposited.
    pub const AppealBond: Balance = 2000 * BASE;
    /// The blocks per year required to calculate the yearly inflation for court incentivisation.
    pub const BlocksPerYear: BlockNumber = BLOCKS_PER_YEAR;
    /// Pallet identifier, mainly used for named balance reserves. DO NOT CHANGE.
    pub const CourtPalletId: PalletId = COURT_PALLET_ID;
    /// The time in which the jurors can cast their secret vote.
    pub const CourtVotePeriod: BlockNumber = 3 * BLOCKS_PER_DAY;
    /// The time in which the jurors should reveal their secret vote.
    pub const CourtAggregationPeriod: BlockNumber = 3 * BLOCKS_PER_DAY;
    /// The time in which a court case can get appealed.
    pub const CourtAppealPeriod: BlockNumber = BLOCKS_PER_DAY;
    /// The lock identifier for the court votes.
    pub const CourtLockId: LockIdentifier = COURT_LOCK_ID;
    /// The time in which the inflation is periodically issued.
    pub const InflationPeriod: BlockNumber = 30 * BLOCKS_PER_DAY;
    /// The maximum number of appeals until the court fails.
    pub const MaxAppeals: u32 = 4;
    /// The maximum number of delegations per juror account.
    pub const MaxDelegations: u32 = 5;
    /// The maximum number of randomly selected `MinJurorStake` draws / atoms of jurors for a dispute.
    pub const MaxSelectedDraws: u32 = 510;
    /// The maximum number of jurors / delegators that can be registered.
    pub const MaxCourtParticipants: u32 = 1_000;
    /// The maximum yearly inflation for court incentivisation.
    pub const MaxYearlyInflation: Perbill = Perbill::from_percent(10);
    /// The minimum stake a user needs to reserve to become a juror.
    pub const MinJurorStake: Balance = 500 * BASE;
    /// The interval for requesting multiple court votes at once.
    pub const RequestInterval: BlockNumber = 7 * BLOCKS_PER_DAY;
}
impl pallet_pm_court::Config for Runtime {
    type AppealBond = AppealBond;
    type BlocksPerYear = BlocksPerYear;
    type VotePeriod = CourtVotePeriod;
    type AggregationPeriod = CourtAggregationPeriod;
    type AppealPeriod = CourtAppealPeriod;
    type LockId = CourtLockId;
    type PalletId = CourtPalletId;
    type Currency = Balances;
    type DisputeResolution = pallet_prediction_markets::Pallet<Runtime>;
    type RuntimeEvent = RuntimeEvent;
    type InflationPeriod = InflationPeriod;
    type MarketCommons = MarketCommons;
    type MaxAppeals = MaxAppeals;
    type MaxDelegations = MaxDelegations;
    type MaxSelectedDraws = MaxSelectedDraws;
    type MaxCourtParticipants = MaxCourtParticipants;
    type MaxYearlyInflation = MaxYearlyInflation;
    type MinJurorStake = MinJurorStake;
    type MonetaryGovernanceOrigin = EnsureRoot<AccountId>;
    type Random = RandomnessCollectiveFlip;
    type RequestInterval = RequestInterval;
    type Slash = Treasury<Runtime>;
    type TreasuryPalletId = AvnTreasuryPotId;
    type WeightInfo = pallet_pm_court::weights::WeightInfo<Runtime>;
}

impl pallet_pm_market_commons::Config for Runtime {
    type Balance = Balance;
    type MarketId = MarketId;
    type Timestamp = Timestamp;
}

// Prediction Market parameters
parameter_types! {
    /// (Slashable) Bond that is provided for creating an advised market that needs approval.
    /// Slashed in case the market is rejected.
    pub const AdvisoryBond: Balance = 100 * BASE;
    /// The percentage of the advisory bond that gets slashed when a market is rejected.
    pub const AdvisoryBondSlashPercentage: Percent = Percent::from_percent(0);
    /// (Slashable) Bond that is provided for disputing an early market close by the market creator.
    pub const CloseEarlyDisputeBond: Balance = 2_000 * BASE;
    // Fat-finger protection for the advisory committe to reject
    // the early market schedule.
    pub const CloseEarlyProtectionTimeFramePeriod: Moment = CloseEarlyProtectionBlockPeriod::get() as u64 * MILLISECS_PER_BLOCK as u64;
    // Fat-finger protection for the advisory committe to reject
    // the early market schedule.
    pub const CloseEarlyProtectionBlockPeriod: BlockNumber = 12 * BLOCKS_PER_HOUR;
    /// (Slashable) Bond that is provided for scheduling an early market close.
    pub const CloseEarlyRequestBond: Balance = 2_000 * BASE;
    /// (Slashable) Bond that is provided for disputing the outcome.
    /// Unreserved in case the dispute was justified otherwise slashed.
    /// This is when the resolved outcome is different to the default (reported) outcome.
    pub const DisputeBond: Balance = 2_000 * BASE;
    /// Maximum number of disputes.
    pub const MaxDisputes: u16 = 1;
    /// The dispute_duration is time where users can dispute the outcome.
    /// Minimum block period for a dispute.
    pub const MinDisputeDuration: BlockNumber = MIN_DISPUTE_DURATION;
    /// Maximum block period for a dispute.
    pub const MaxDisputeDuration: BlockNumber = MAX_DISPUTE_DURATION;
    /// Maximum Categories a prediciton market can have (excluding base asset).
    pub const MaxCategories: u16 = MAX_CATEGORIES;
    /// Max creator fee, bounds the fraction per trade volume that is moved to the market creator.
    pub const MaxCreatorFee: Perbill = Perbill::from_percent(1);
    /// Maximum string length for edit reason.
    pub const MaxEditReasonLen: u32 = 1024;
    /// Maximum block period for a grace_period.
    /// The grace_period is a delay between the point where the market closes and the point where the oracle may report.
    pub const MaxGracePeriod: BlockNumber = MAX_GRACE_PERIOD;
    /// The maximum allowed duration of a market from creation to market close in blocks.
    pub const MaxMarketLifetime: BlockNumber = MAX_MARKET_LIFETIME;
    /// Maximum block period for an oracle_duration.
    /// The oracle_duration is a duration where the oracle has to submit its report.
    pub const MaxOracleDuration: BlockNumber = MAX_ORACLE_DURATION;
    /// Maximum string length allowed for reject reason.
    pub const MaxRejectReasonLen: u32 = 1024;
    /// Minimum number of categories. The trivial minimum is 2, which represents a binary market.
    pub const MinCategories: u16 = 2;
    /// Minimum block period for an oracle_duration.
    pub const MinOracleDuration: BlockNumber = MIN_ORACLE_DURATION;
    /// (Slashable) The orcale bond. Slashed in case the final outcome does not match the
    /// outcome the oracle reported.
    pub const OracleBond: Balance = 100 * BASE;
    /// (Slashable) A bond for an outcome reporter, who is not the oracle.
    /// Slashed in case the final outcome does not match the outcome by the outsider.
    // If we remove the whitelist restriction for market creation, review this figure and ensure its > OracleBond
    pub const OutsiderBond: Balance = 2000 * BASE;
    /// Pallet identifier, mainly used for named balance reserves. DO NOT CHANGE.
    pub const PmPalletId: PalletId = PM_PALLET_ID;
    // Waiting time for market creator to close the market after an early close schedule.
    pub const CloseEarlyBlockPeriod: BlockNumber = 5 * BLOCKS_PER_DAY;
    pub const CloseEarlyTimeFramePeriod: Moment = CloseEarlyBlockPeriod::get() as u64 * MILLISECS_PER_BLOCK as u64;
    /// (Slashable) A bond for creation markets that do not require approval. Slashed in case
    /// the market is forcefully destroyed.
    // The low amount is assuming only whitelisted accounts can create a market
    pub const ValidityBond: Balance = 100 * BASE;
    // Orderbook parameters
    pub const OrderbookPalletId: PalletId = ORDERBOOK_PALLET_ID;
    // Hybrid Router parameters
    pub const HybridRouterPalletId: PalletId = HYBRID_ROUTER_PALLET_ID;
    /// Maximum number of orders that can be placed in a single trade transaction.
    pub const MaxOrders: u32 = 100;
    /// The percentage of winning we deduct from the winner.
    pub const WinnerFeePercentage: Perbill = Perbill::from_percent(5);
}

impl_winner_fees!();

impl pallet_prediction_markets::Config for Runtime {
    type AdvisoryBond = AdvisoryBond;
    type AdvisoryBondSlashPercentage = AdvisoryBondSlashPercentage;
    type ApproveOrigin = EnsureRootOrMoreThanOneThirdAdvisoryCommittee;
    type Authorized = Authorized;
    type Currency = Balances;
    type Court = Court;
    type CloseEarlyDisputeBond = CloseEarlyDisputeBond;
    type CloseMarketEarlyOrigin = EnsureRootOrMoreThanOneThirdAdvisoryCommittee;
    type CloseOrigin = EnsureRoot<AccountId>;
    type CloseEarlyProtectionTimeFramePeriod = CloseEarlyProtectionTimeFramePeriod;
    type CloseEarlyProtectionBlockPeriod = CloseEarlyProtectionBlockPeriod;
    type CloseEarlyRequestBond = CloseEarlyRequestBond;
    type DeployPool = NeoSwaps;
    type DisputeBond = DisputeBond;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type GlobalDisputes = GlobalDisputes;
    type MaxCategories = MaxCategories;
    type MaxCreatorFee = MaxCreatorFee;
    type MaxDisputes = MaxDisputes;
    type MaxMarketLifetime = MaxMarketLifetime;
    type MinDisputeDuration = MinDisputeDuration;
    type MaxDisputeDuration = MaxDisputeDuration;
    type MaxGracePeriod = MaxGracePeriod;
    type MaxOracleDuration = MaxOracleDuration;
    type MinOracleDuration = MinOracleDuration;
    type MinCategories = MinCategories;
    type MaxEditReasonLen = MaxEditReasonLen;
    type MaxRejectReasonLen = MaxRejectReasonLen;
    type OracleBond = OracleBond;
    type OutsiderBond = OutsiderBond;
    type PalletId = PmPalletId;
    type CloseEarlyBlockPeriod = CloseEarlyBlockPeriod;
    type CloseEarlyTimeFramePeriod = CloseEarlyTimeFramePeriod;
    type RejectOrigin = EnsureRootOrMoreThanTwoThirdsAdvisoryCommittee;
    type RequestEditOrigin = EnsureRootOrMoreThanOneThirdAdvisoryCommittee;
    type ResolveOrigin = EnsureRoot<AccountId>;
    type AssetManager = AssetManager;
    type Slash = Treasury<Runtime>;
    type ValidityBond = ValidityBond;
    type WeightInfo = pallet_prediction_markets::weights::WeightInfo<Runtime>;
    type AssetRegistry = AssetRegistry;
    type Public = <Signature as sp_runtime::traits::Verify>::Signer;
    type Signature = Signature;
    type TokenInterface = TokenManager;
    type WinnerFeePercentage = WinnerFeePercentage;
    type WinnerFeeHandler = WinnerFee;
}

parameter_types! {
    // Asset registry
    pub const AssetRegistryStringLimit: u32 = 1024;
}

impl pallet_pm_eth_asset_registry::Config for Runtime {
    type AssetId = CurrencyId;
    type AuthorityOrigin = EnsureRoot<AccountId>;
    type Balance = Balance;
    type CustomMetadata = CustomMetadata;
    type RuntimeEvent = RuntimeEvent;
    type StringLimit = AssetRegistryStringLimit;
    type AssetProcessor = CustomAssetProcessor;
    type WeightInfo = ();
}

parameter_types! {
    pub const GetNativeCurrencyId: CurrencyId = Asset::Tru;
    pub const TreasuryPalletId: PalletId = TREASURY_PALLET_ID;
    pub TnfTreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}

impl orml_currencies::Config for Runtime {
    type GetNativeCurrencyId = GetNativeCurrencyId;
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances>;
    type WeightInfo = third_party_weights::orml_currencies::WeightInfo<Runtime>;
}

// Shared within tests
// Balance
parameter_types! {
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
}

parameter_type_with_key! {
    pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
        match currency_id {
            Asset::Tru => NATIVE_EXISTENTIAL_DEPOSIT,
            Asset::ForeignAsset(id) => {
                let maybe_metadata = <
                pallet_pm_eth_asset_registry::Pallet<Runtime> as prediction_market_primitives::traits::InspectEthAsset
                >::metadata(&Asset::ForeignAsset(*id));

                if let Some(metadata) = maybe_metadata {
                    return metadata.existential_deposit;
                }

                1
            },
            Asset::CategoricalOutcome(_,_) => DEFAULT_EXISTENTIAL_DEPOSIT,
            Asset::CombinatorialOutcome => DEFAULT_EXISTENTIAL_DEPOSIT,
            Asset::PoolShare(_)  => DEFAULT_EXISTENTIAL_DEPOSIT,
            Asset::ScalarOutcome(_,_)  => DEFAULT_EXISTENTIAL_DEPOSIT,
            Asset::ParimutuelShare(_,_)  => DEFAULT_EXISTENTIAL_DEPOSIT,
        }
    };
}

pub struct CurrencyHooks<R>(sp_std::marker::PhantomData<R>);
impl<C: orml_tokens::Config> orml_traits::currency::MutationHooks<AccountId, CurrencyId, Balance>
    for CurrencyHooks<C>
{
    type OnDust = orml_tokens::TransferDust<Runtime, TnfTreasuryAccount>;
    type OnKilledTokenAccount = ();
    type OnNewTokenAccount = ();
    type OnSlash = ();
    type PostDeposit = ();
    type PostTransfer = ();
    type PreDeposit = ();
    type PreTransfer = ();
}

impl orml_tokens::Config for Runtime {
    type Amount = OrmlAmount;
    type Balance = Balance;
    type CurrencyHooks = CurrencyHooks<Runtime>;
    type CurrencyId = CurrencyId;
    type DustRemovalWhitelist = DustRemovalWhitelist;
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposits = ExistentialDeposits;
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    type WeightInfo = third_party_weights::orml_tokens::WeightInfo<Runtime>;
}

// Global disputes parameters
parameter_types! {
    pub const AddOutcomePeriod: BlockNumber = 20;
    pub const GlobalDisputeLockId: LockIdentifier = GLOBAL_DISPUTES_LOCK_ID;
    pub const GlobalDisputesPalletId: PalletId = GLOBAL_DISPUTES_PALLET_ID;
    pub const MaxGlobalDisputeVotes: u32 = 50;
    pub const MaxOwners: u32 = 10;
    pub const MinOutcomeVoteAmount: Balance = 10 * CENT_BASE;
    pub const RemoveKeysLimit: u32 = 250;
    pub const GdVotingPeriod: BlockNumber = 140;
    pub const VotingOutcomeFee: Balance = 100 * CENT_BASE;
}

impl pallet_pm_global_disputes::Config for Runtime {
    type AddOutcomePeriod = AddOutcomePeriod;
    type Currency = Balances;
    type DisputeResolution = pallet_prediction_markets::Pallet<Runtime>;
    type RuntimeEvent = RuntimeEvent;
    type GlobalDisputeLockId = GlobalDisputeLockId;
    type GlobalDisputesPalletId = GlobalDisputesPalletId;
    type MarketCommons = MarketCommons;
    type MaxGlobalDisputeVotes = MaxGlobalDisputeVotes;
    type MaxOwners = MaxOwners;
    type MinOutcomeVoteAmount = MinOutcomeVoteAmount;
    type RemoveKeysLimit = RemoveKeysLimit;
    type GdVotingPeriod = GdVotingPeriod;
    type VotingOutcomeFee = VotingOutcomeFee;
    type WeightInfo = pallet_pm_global_disputes::weights::WeightInfo<Runtime>;
}

parameter_types! {
    // NeoSwaps
    pub const NeoSwapsMaxSwapFee: Balance = 10 * CENT_BASE;
    pub const NeoSwapsPalletId: PalletId = NS_PALLET_ID;
    pub const MaxLiquidityTreeDepth: u32 = 9u32;
}

impl_market_creator_fees!();

impl pallet_pm_neo_swaps::Config for Runtime {
    type CompleteSetOperations = PredictionMarkets;
    type ExternalFees = AdditionalSwapFee;
    type MarketCommons = MarketCommons;
    type MultiCurrency = AssetManager;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_pm_neo_swaps::weights::WeightInfo<Runtime>;
    type MaxLiquidityTreeDepth = MaxLiquidityTreeDepth;
    type MaxSwapFee = NeoSwapsMaxSwapFee;
    type PalletId = NeoSwapsPalletId;
    type SignedTxLifetime = ConstU32<16>;
    type Public = <Signature as sp_runtime::traits::Verify>::Signer;
    type Signature = Signature;
    type PalletAdminGetter = PredictionMarkets;
    type OnLiquidityProvided = PredictionMarkets;
}

impl pallet_pm_order_book::Config for Runtime {
    type AssetManager = AssetManager;
    type ExternalFees = AdditionalSwapFee;
    type RuntimeEvent = RuntimeEvent;
    type MarketCommons = MarketCommons;
    type PalletId = OrderbookPalletId;
    type WeightInfo = pallet_pm_order_book::weights::WeightInfo<Runtime>;
}

impl pallet_pm_hybrid_router::Config for Runtime {
    type AssetManager = AssetManager;
    #[cfg(feature = "runtime-benchmarks")]
    type AmmPoolDeployer = NeoSwaps;
    #[cfg(feature = "runtime-benchmarks")]
    type CompleteSetOperations = PredictionMarkets;
    type MarketCommons = MarketCommons;
    type Amm = NeoSwaps;
    type Orderbook = Orderbook;
    type MaxOrders = MaxOrders;
    type RuntimeEvent = RuntimeEvent;
    type PalletId = HybridRouterPalletId;
    type RuntimeCall = RuntimeCall;
    type Public = <Signature as sp_runtime::traits::Verify>::Signer;
    type Signature = Signature;
    type WeightInfo = pallet_pm_hybrid_router::weights::WeightInfo<Runtime>;
}

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
    pub struct Runtime {
        System: frame_system = 0,
        Timestamp: pallet_timestamp = 1,
        Aura: pallet_aura = 2,
        Grandpa: pallet_grandpa = 3,
        Balances: pallet_balances = 4,
        TransactionPayment: pallet_transaction_payment = 5,
        Sudo: pallet_sudo = 6,
        Session: pallet_session = 7,
        Authorship: pallet_authorship = 8,
        AuthorityDiscovery: pallet_authority_discovery = 9,
        Historical: pallet_session_historical::{Pallet} = 10,
        Offences: pallet_offences = 11,
        ImOnline: pallet_im_online = 12,
        Scheduler: pallet_scheduler::{Pallet, Storage, Event<T>, Call} = 19,
        Preimage: pallet_preimage::{Pallet, Call, Storage, Event<T>} = 20,
        Utility: pallet_utility = 24,
        Multisig: pallet_multisig::{Pallet, Call, Storage, Event<T>} = 25,

        // AvN pallets
        Avn: pallet_avn = 13,
        AvnTransactionPayment: pallet_avn_transaction_payment = 14,
        EthBridge: pallet_eth_bridge = 15,
        Summary: pallet_summary::<Instance1> = 16,
        EthereumEvents: pallet_ethereum_events = 17,
        TokenManager: pallet_token_manager = 18,
        AvnProxy: pallet_avn_proxy = 21,
        AuthorsManager: pallet_authors_manager = 22,
        NftManager: pallet_nft_manager = 23,
        AnchorSummary: pallet_summary::<Instance2> = 26,
        NodeManager: pallet_node_manager = 27,
        PalletConfig: pallet_config = 28,

        // Prediction Market pallets
        AdvisoryCommittee: pallet_collective::<Instance1>::{Call, Config<T>, Event<T>, Origin<T>, Pallet, Storage} = 30,
        AssetRegistry: pallet_pm_eth_asset_registry::{Call, Config<T>, Event<T>, Pallet, Storage} = 31,
        RandomnessCollectiveFlip: pallet_insecure_randomness_collective_flip::{Pallet, Storage} = 32,
        AssetManager: orml_currencies::{Call, Pallet, Storage} = 33,
        Tokens: orml_tokens::{Config<T>, Event<T>, Pallet, Storage} = 34,

        MarketCommons: pallet_pm_market_commons::{Pallet, Storage} = 40,
        Authorized: pallet_pm_authorized::{Call, Event<T>, Pallet, Storage} = 41,
        Court: pallet_pm_court::{Call, Event<T>, Pallet, Storage} = 42,
        PredictionMarkets: pallet_prediction_markets::{Call, Config<T>, Event<T>, Pallet, Storage} = 43,
        GlobalDisputes: pallet_pm_global_disputes::{Call, Event<T>, Pallet, Storage} = 44,
        NeoSwaps: pallet_pm_neo_swaps::{Call, Config<T>, Event<T>, Pallet, Storage} = 45,
        Orderbook: pallet_pm_order_book::{Call, Event<T>, Pallet, Storage} = 46,
        HybridRouter: pallet_pm_hybrid_router::{Call, Event<T>, Pallet, Storage} = 47,
    }
);

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, ()>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
    generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
    (
        pallet_eth_bridge::migration::EthBridgeMigrations<Runtime>,
        pallet_node_manager::migration::RewardPeriodInfoUpgrade<Runtime>,
    ),
>;

#[cfg(feature = "runtime-benchmarks")]
#[macro_use]
extern crate frame_benchmarking;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
    define_benchmarks!(
        [frame_benchmarking, BaselineBench::<Runtime>]
        [frame_system, SystemBench::<Runtime>]
        [pallet_balances, Balances]
        [pallet_timestamp, Timestamp]
        [pallet_sudo, Sudo]
        [pallet_im_online, ImOnline]
        [pallet_utility, Utility]
        // AvN pallets
        [pallet_avn, Avn]
        [pallet_avn_transaction_payment, AvnTransactionPayment]
        [pallet_ethereum_events, EthereumEvents]
        [pallet_summary, Summary]
        [pallet_token_manager, TokenManager]
        [pallet_avn_proxy, AvnProxy]
        [pallet_nft_manager, NftManager]
        [pallet_node_manager, NodeManager]
        [pallet_config, PalletConfig]
        // [pallet_eth_bridge, EthBridge]
        [pallet_multisig, Multisig]
        // Tnf pallets
        [pallet_authors_manager, AuthorsManager]
        [pallet_prediction_markets, PredictionMarkets]
        [pallet_pm_neo_swaps, NeoSwaps]
        [pallet_pm_hybrid_router, HybridRouter]
    );
}

use sp_avn_common::event_discovery::{EthBlockRange, EthereumEventsPartition};
use sp_core::H160;

impl_runtime_apis! {
    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: Block) {
            Executive::execute_block(block);
        }

        fn initialize_block(header: &<Block as BlockT>::Header) {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }

        fn metadata_versions() -> sp_std::vec::Vec<u32> {
            Runtime::metadata_versions()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: Block,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }


    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
        fn slot_duration() -> sp_consensus_aura::SlotDuration {
            sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
        }

        fn authorities() -> Vec<AuraId> {
            Aura::authorities().into_inner()
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
            opaque::SessionKeys::generate(seed)
        }

        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl sp_consensus_grandpa::GrandpaApi<Block> for Runtime {
        fn grandpa_authorities() -> sp_consensus_grandpa::AuthorityList {
            Grandpa::grandpa_authorities()
        }

        fn current_set_id() -> sp_consensus_grandpa::SetId {
            Grandpa::current_set_id()
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            _equivocation_proof: sp_consensus_grandpa::EquivocationProof<
                <Block as BlockT>::Hash,
                NumberFor<Block>,
            >,
            _key_owner_proof: sp_consensus_grandpa::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            None
        }

        fn generate_key_ownership_proof(
            _set_id: sp_consensus_grandpa::SetId,
            _authority_id: GrandpaId,
        ) -> Option<sp_consensus_grandpa::OpaqueKeyOwnershipProof> {
            // NOTE: this is the only implementation possible since we've
            // defined our key owner proof type as a bottom type (i.e. a type
            // with no values).
            None
        }
    }

    impl sp_authority_discovery::AuthorityDiscoveryApi<Block> for Runtime {
        fn authorities() -> Vec<AuthorityDiscoveryId> {
            AuthorityDiscovery::authorities()
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
        fn account_nonce(account: AccountId) -> Nonce {
            System::account_nonce(account)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
        fn query_info(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_info(uxt, len)
        }
        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_fee_details(uxt, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
        for Runtime
    {
        fn query_call_info(
            call: RuntimeCall,
            len: u32,
        ) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_call_info(call, len)
        }
        fn query_call_fee_details(
            call: RuntimeCall,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_call_fee_details(call, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl pallet_eth_bridge_runtime_api::EthEventHandlerApi<Block, AccountId> for Runtime {
        fn query_authors() -> Vec<([u8; 32], [u8; 32])> {
            let authors = Avn::validators().to_vec();
            let res = authors.iter().map(|author| {
                let mut address: [u8; 32] = Default::default();
                address.copy_from_slice(&author.account_id.encode()[0..32]);

                let mut key: [u8; 32] = Default::default();
                key.copy_from_slice(&author.key.to_raw_vec()[0..32]);

                return (address, key)
            }).collect();
            return res
        }

        fn query_active_block_range()-> Option<(EthBlockRange, u16)> {
            if let Some(active_eth_range) =  EthBridge::active_ethereum_range(){
                Some((active_eth_range.range, active_eth_range.partition))
            } else {
                None
            }
        }

        fn query_has_author_casted_vote(account_id: AccountId) -> bool{
           pallet_eth_bridge::author_has_cast_event_vote::<Runtime>(&account_id) ||
           pallet_eth_bridge::author_has_submitted_latest_block::<Runtime>(&account_id)
        }

        fn query_signatures() -> Vec<sp_core::H256> {
            EthBridge::signatures()
        }

        fn query_bridge_contract() -> H160 {
            Avn::get_bridge_contract_address()
        }

        fn submit_vote(author: AccountId,
            events_partition: EthereumEventsPartition,
            signature: sp_core::sr25519::Signature,
        ) -> Option<()>{
            EthBridge::submit_vote(author, events_partition, signature.into()).ok()
        }

        fn submit_latest_ethereum_block(
            author: AccountId,
            latest_seen_block: u32,
            signature: sp_core::sr25519::Signature
        ) -> Option<()>{
            EthBridge::submit_latest_ethereum_block_vote(author, latest_seen_block, signature.into()).ok()
        }

        fn additional_transactions() -> Option<AdditionalEvents> {
            if let Some(active_eth_range) =  EthBridge::active_ethereum_range(){
                Some(active_eth_range.additional_transactions)
            } else {
                None
            }
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl frame_benchmarking::Benchmark<Block> for Runtime {
        fn benchmark_metadata(extra: bool) -> (
            Vec<frame_benchmarking::BenchmarkList>,
            Vec<frame_support::traits::StorageInfo>,
        ) {
            use frame_benchmarking::{baseline, Benchmarking, BenchmarkList};
            use frame_support::traits::StorageInfoTrait;
            use frame_system_benchmarking::Pallet as SystemBench;
            use baseline::Pallet as BaselineBench;

            let mut list = Vec::<BenchmarkList>::new();
            list_benchmarks!(list, extra);

            let storage_info = AllPalletsWithSystem::storage_info();

            (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
            use frame_benchmarking::{baseline, Benchmarking, BenchmarkBatch};

            use frame_system_benchmarking::Pallet as SystemBench;
            use baseline::Pallet as BaselineBench;

            #[allow(non_local_definitions)]
            impl frame_system_benchmarking::Config for Runtime {}
            #[allow(non_local_definitions)]
            impl baseline::Config for Runtime {}

            use frame_support::traits::WhitelistedStorageKeys;
            let whitelist = AllPalletsWithSystem::whitelisted_storage_keys();

            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);
            add_benchmarks!(params, batches);

            Ok(batches)
        }
    }

    #[cfg(feature = "try-runtime")]
    impl frame_try_runtime::TryRuntime<Block> for Runtime {
        fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here. If any of the pre/post migration checks fail, we shall stop
            // right here and right now.
            let weight = Executive::try_runtime_upgrade(checks).unwrap();
            (weight, RuntimeBlockWeights::get().max_block)
        }

        fn execute_block(
            block: Block,
            state_root_check: bool,
            signature_check: bool,
            select: frame_try_runtime::TryStateSelect
        ) -> Weight {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here.
            Executive::try_execute_block(block, state_root_check, signature_check, select).expect("execute-block failed")
        }
    }
}

use pallet_avn::{EventMigration, ProcessedEventsChecker};
use sp_avn_common::{bounds::ProcessingBatchBound, event_types::EthEventId};
use sp_runtime::BoundedVec;

pub struct ProcessedEventCustodian {}
impl ProcessedEventsChecker for ProcessedEventCustodian {
    fn processed_event_exists(event_id: &EthEventId) -> bool {
        EthBridge::processed_event_exists(event_id) ||
            EthereumEvents::processed_event_exists(event_id)
    }

    fn add_processed_event(event_id: &EthEventId, accepted: bool) -> Result<(), ()> {
        frame_support::ensure!(!Self::processed_event_exists(event_id), ());
        EthBridge::add_processed_event(event_id, accepted)
    }

    fn get_events_to_migrate() -> Option<BoundedVec<EventMigration, ProcessingBatchBound>> {
        EthereumEvents::get_events_to_migrate()
    }
}
