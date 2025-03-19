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

#![cfg(feature = "mock")]
#![allow(
    // Mocks are only used for fuzzing and unit tests
    clippy::arithmetic_side_effects,
    clippy::too_many_arguments,
)]

use crate as pallet_pm_neo_swaps;
use crate::{consts::*, AssetOf, MarketIdOf};
use common_primitives::types::{Balance, Hash, Moment};
use core::marker::PhantomData;
use frame_support::{
    construct_runtime, ord_parameter_types, parameter_types,
    traits::{Contains, Everything, NeverEnsureOrigin},
};
use frame_system::{mocking::MockBlockU32, EnsureRoot, EnsureSignedBy};
use orml_traits::{asset_registry::AssetProcessor, MultiCurrency};
use pallet_pm_neo_swaps::BalanceOf;
use parity_scale_codec::{alloc::sync::Arc, Encode};
use sp_keystore::{testing::MemoryKeystore, KeystoreExt};
pub use prediction_market_primitives::test_helper::get_account;
use prediction_market_primitives::{
    constants::{
        base_multiples::*,
        mock::{
            AddOutcomePeriod, AggregationPeriod, AppealBond, AppealPeriod, AuthorizedPalletId,
            BlockHashCount, BlocksPerYear, CloseEarlyBlockPeriod, CloseEarlyDisputeBond,
            CloseEarlyProtectionBlockPeriod, CloseEarlyProtectionTimeFramePeriod,
            CloseEarlyRequestBond, CloseEarlyTimeFramePeriod, CorrectionPeriod, CourtPalletId,
            ExistentialDeposit, ExistentialDeposits, GdVotingPeriod, GetNativeCurrencyId,
            GlobalDisputeLockId, GlobalDisputesPalletId, InflationPeriod, LockId, MaxAppeals,
            MaxApprovals, MaxCourtParticipants, MaxCreatorFee, MaxDelegations, MaxDisputeDuration,
            MaxDisputes, MaxEditReasonLen, MaxGlobalDisputeVotes, MaxGracePeriod,
            MaxLiquidityTreeDepth, MaxLocks, MaxMarketLifetime, MaxOracleDuration, MaxOwners,
            MaxRejectReasonLen, MaxReserves, MaxSelectedDraws, MaxYearlyInflation, MinCategories,
            MinDisputeDuration, MinJurorStake, MinOracleDuration, MinOutcomeVoteAmount,
            MinimumPeriod, NeoMaxSwapFee, NeoSwapsPalletId, OutsiderBond, PmPalletId,
            RemoveKeysLimit, RequestInterval, TreasuryPalletId, VotePeriod, VotingOutcomeFee, BASE,
            CENT_BASE,
        },
    },
    math::fixed::FixedMul,
    traits::{DeployPoolApi, DistributeFees},
    types::{
        Asset, BasicCurrencyAdapter, CurrencyId, CustomMetadata, MarketId, OrmlAmount,
        SignatureTest, TestAccountIdPK,
    },
};
use sp_core::H160;
use sp_runtime::{
    traits::{BlakeTwo256, ConstU32, Get, IdentityLookup, Zero},
    BuildStorage, DispatchError, DispatchResult, Perbill, Percent, SaturatedConversion,
};

pub fn alice() -> TestAccountIdPK {
    get_account(0u8)
}
#[allow(unused)]
pub fn bob() -> TestAccountIdPK {
    get_account(1u8)
}
pub fn charlie() -> TestAccountIdPK {
    get_account(2u8)
}
pub fn dave() -> TestAccountIdPK {
    get_account(3u8)
}
pub fn eve() -> TestAccountIdPK {
    get_account(4u8)
}
pub fn fee_account() -> TestAccountIdPK {
    get_account(5u8)
}
pub fn sudo() -> TestAccountIdPK {
    get_account(123u8)
}

pub const EXTERNAL_FEES: Balance = CENT_BASE;

pub const FOREIGN_ASSET: Asset<MarketId> = Asset::ForeignAsset(1);

parameter_types! {
    pub FeeAccount: TestAccountIdPK = fee_account();
}
ord_parameter_types! {
    pub const AuthorizedDisputeResolutionUser: TestAccountIdPK = alice();
}
ord_parameter_types! {
    pub const Sudo: TestAccountIdPK = sudo();
}
parameter_types! {
    pub storage NeoMinSwapFee: Balance = 0;
}
parameter_types! {
    pub const AdvisoryBond: Balance = 0;
    pub const AdvisoryBondSlashPercentage: Percent = Percent::from_percent(10);
    pub const OracleBond: Balance = 0;
    pub const ValidityBond: Balance = 0;
    pub const DisputeBond: Balance = 0;
    pub const MaxCategories: u16 = MAX_ASSETS + 1;
}

pub struct DeployPoolNoop;

impl DeployPoolApi for DeployPoolNoop {
    type AccountId = TestAccountIdPK;
    type Balance = Balance;
    type MarketId = MarketId;

    fn deploy_pool(
        _who: Self::AccountId,
        _market_id: Self::MarketId,
        _amount: Self::Balance,
        _swap_prices: Vec<Self::Balance>,
        _swap_fee: Self::Balance,
    ) -> DispatchResult {
        Ok(())
    }
}

pub struct ExternalFees<T, F>(PhantomData<T>, PhantomData<F>);

impl<T: crate::Config, F> DistributeFees for ExternalFees<T, F>
where
    F: Get<T::AccountId>,
{
    type Asset = AssetOf<T>;
    type AccountId = T::AccountId;
    type Balance = BalanceOf<T>;
    type MarketId = MarketIdOf<T>;

    fn distribute(
        _market_id: Self::MarketId,
        asset: Self::Asset,
        account: &Self::AccountId,
        amount: Self::Balance,
    ) -> Self::Balance {
        let fees = amount.bmul(EXTERNAL_FEES.saturated_into()).unwrap();
        match T::MultiCurrency::transfer(asset, account, &F::get(), fees) {
            Ok(_) => fees,
            Err(_) => Zero::zero(),
        }
    }

    fn fee_percentage(_market_id: Self::MarketId) -> Perbill {
        Perbill::from_rational(EXTERNAL_FEES, BASE)
    }
}

pub struct DustRemovalWhitelist;

impl Contains<TestAccountIdPK> for DustRemovalWhitelist {
    fn contains(account_id: &TestAccountIdPK) -> bool {
        *account_id == fee_account()
    }
}

construct_runtime!(
    pub enum Runtime {
        NeoSwaps: pallet_pm_neo_swaps,
        AssetManager: orml_currencies,
        AssetRegistry: pallet_pm_eth_asset_registry,
        Authorized: pallet_pm_authorized,
        Balances: pallet_balances,
        Court: pallet_pm_court,
        MarketCommons: pallet_pm_market_commons,
        PredictionMarkets: pallet_prediction_markets,
        RandomnessCollectiveFlip: pallet_insecure_randomness_collective_flip,
        GlobalDisputes: pallet_pm_global_disputes,
        System: frame_system,
        Timestamp: pallet_timestamp,
        Tokens: orml_tokens,
        Treasury: pallet_treasury,
        AVN: pallet_avn,
    }
);

impl crate::Config for Runtime {
    type MultiCurrency = AssetManager;
    type CompleteSetOperations = PredictionMarkets;
    type ExternalFees = ExternalFees<Runtime, FeeAccount>;
    type MarketCommons = MarketCommons;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type MaxLiquidityTreeDepth = MaxLiquidityTreeDepth;
    type MaxSwapFee = NeoMaxSwapFee;
    type PalletId = NeoSwapsPalletId;
    type WeightInfo = pallet_pm_neo_swaps::weights::WeightInfo<Runtime>;
    type SignedTxLifetime = ConstU32<16>;
    type Public = TestAccountIdPK;
    type Signature = SignatureTest;
}

impl pallet_insecure_randomness_collective_flip::Config for Runtime {}

impl pallet_prediction_markets::Config for Runtime {
    type AdvisoryBond = AdvisoryBond;
    type AdvisoryBondSlashPercentage = AdvisoryBondSlashPercentage;
    type ApproveOrigin = EnsureSignedBy<Sudo, TestAccountIdPK>;
    type AssetRegistry = AssetRegistry;
    type Authorized = Authorized;
    type CloseEarlyBlockPeriod = CloseEarlyBlockPeriod;
    type CloseEarlyDisputeBond = CloseEarlyDisputeBond;
    type CloseEarlyTimeFramePeriod = CloseEarlyTimeFramePeriod;
    type CloseEarlyProtectionBlockPeriod = CloseEarlyProtectionBlockPeriod;
    type CloseEarlyProtectionTimeFramePeriod = CloseEarlyProtectionTimeFramePeriod;
    type CloseEarlyRequestBond = CloseEarlyRequestBond;
    type CloseMarketEarlyOrigin = EnsureSignedBy<Sudo, TestAccountIdPK>;
    type CloseOrigin = EnsureSignedBy<Sudo, TestAccountIdPK>;
    type Court = Court;
    type Currency = Balances;
    type DeployPool = DeployPoolNoop;
    type DisputeBond = DisputeBond;
    type RuntimeEvent = RuntimeEvent;
    type GlobalDisputes = GlobalDisputes;
    type MaxCategories = MaxCategories;
    type MaxDisputes = MaxDisputes;
    type MinDisputeDuration = MinDisputeDuration;
    type MinOracleDuration = MinOracleDuration;
    type MaxCreatorFee = MaxCreatorFee;
    type MaxDisputeDuration = MaxDisputeDuration;
    type MaxGracePeriod = MaxGracePeriod;
    type MaxOracleDuration = MaxOracleDuration;
    type MaxMarketLifetime = MaxMarketLifetime;
    type MinCategories = MinCategories;
    type MaxEditReasonLen = MaxEditReasonLen;
    type MaxRejectReasonLen = MaxRejectReasonLen;
    type OracleBond = OracleBond;
    type OutsiderBond = OutsiderBond;
    type PalletId = PmPalletId;
    type RejectOrigin = EnsureSignedBy<Sudo, TestAccountIdPK>;
    type RequestEditOrigin = EnsureSignedBy<Sudo, TestAccountIdPK>;
    type ResolveOrigin = EnsureSignedBy<Sudo, TestAccountIdPK>;
    type AssetManager = AssetManager;
    type Slash = Treasury;
    type ValidityBond = ValidityBond;
    type RuntimeCall = RuntimeCall;
    type Public = TestAccountIdPK;
    type Signature = SignatureTest;
    type WeightInfo = pallet_prediction_markets::weights::WeightInfo<Runtime>;
    type TokenInterface = ();
}

impl pallet_pm_authorized::Config for Runtime {
    type AuthorizedDisputeResolutionOrigin =
        EnsureSignedBy<AuthorizedDisputeResolutionUser, TestAccountIdPK>;
    type CorrectionPeriod = CorrectionPeriod;
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type DisputeResolution = pallet_prediction_markets::Pallet<Runtime>;
    type MarketCommons = MarketCommons;
    type PalletId = AuthorizedPalletId;
    type WeightInfo = pallet_pm_authorized::weights::WeightInfo<Runtime>;
}

impl pallet_avn::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AuthorityId = pallet_avn::sr25519::AuthorityId;
    type EthereumPublicKeyChecker = ();
    type NewSessionHandler = ();
    type DisabledValidatorChecker = ();
    type WeightInfo = ();
}

impl pallet_pm_court::Config for Runtime {
    type AppealBond = AppealBond;
    type BlocksPerYear = BlocksPerYear;
    type DisputeResolution = pallet_prediction_markets::Pallet<Runtime>;
    type VotePeriod = VotePeriod;
    type AggregationPeriod = AggregationPeriod;
    type AppealPeriod = AppealPeriod;
    type LockId = LockId;
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type InflationPeriod = InflationPeriod;
    type MarketCommons = MarketCommons;
    type MaxAppeals = MaxAppeals;
    type MaxDelegations = MaxDelegations;
    type MaxSelectedDraws = MaxSelectedDraws;
    type MaxCourtParticipants = MaxCourtParticipants;
    type MaxYearlyInflation = MaxYearlyInflation;
    type MinJurorStake = MinJurorStake;
    type MonetaryGovernanceOrigin = EnsureRoot<TestAccountIdPK>;
    type PalletId = CourtPalletId;
    type Random = RandomnessCollectiveFlip;
    type RequestInterval = RequestInterval;
    type Slash = Treasury;
    type TreasuryPalletId = TreasuryPalletId;
    type WeightInfo = pallet_pm_court::weights::WeightInfo<Runtime>;
}

impl frame_system::Config for Runtime {
    type AccountData = pallet_balances::AccountData<Balance>;
    type AccountId = TestAccountIdPK;
    type BaseCallFilter = Everything;
    type Block = MockBlockU32<Runtime>;
    type BlockHashCount = BlockHashCount;
    type BlockLength = ();
    type BlockWeights = ();
    type RuntimeCall = RuntimeCall;
    type DbWeight = ();
    type RuntimeEvent = RuntimeEvent;
    type Hash = Hash;
    type Hashing = BlakeTwo256;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Nonce = u64;
    type MaxConsumers = ConstU32<16>;
    type OnKilledAccount = ();
    type OnNewAccount = ();
    type RuntimeOrigin = RuntimeOrigin;
    type PalletInfo = PalletInfo;
    type SS58Prefix = ();
    type SystemWeightInfo = ();
    type Version = ();
    type OnSetCode = ();
}

impl orml_currencies::Config for Runtime {
    type GetNativeCurrencyId = GetNativeCurrencyId;
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances>;
    type WeightInfo = ();
}

impl orml_tokens::Config for Runtime {
    type Amount = OrmlAmount;
    type Balance = Balance;
    type CurrencyId = CurrencyId;
    type DustRemovalWhitelist = DustRemovalWhitelist;
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposits = ExistentialDeposits;
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type CurrencyHooks = ();
    type ReserveIdentifier = [u8; 8];
    type WeightInfo = ();
}

impl pallet_balances::Config for Runtime {
    type AccountStore = System;
    type Balance = Balance;
    type DustRemoval = ();
    type FreezeIdentifier = ();
    type RuntimeHoldReason = ();
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type MaxHolds = ();
    type MaxFreezes = ();
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    type WeightInfo = ();
}

impl pallet_pm_market_commons::Config for Runtime {
    type Balance = Balance;
    type MarketId = MarketId;
    type Timestamp = Timestamp;
}

impl pallet_timestamp::Config for Runtime {
    type MinimumPeriod = MinimumPeriod;
    type Moment = Moment;
    type OnTimestampSet = ();
    type WeightInfo = ();
}

impl pallet_pm_global_disputes::Config for Runtime {
    type AddOutcomePeriod = AddOutcomePeriod;
    type RuntimeEvent = RuntimeEvent;
    type DisputeResolution = pallet_prediction_markets::Pallet<Runtime>;
    type MarketCommons = MarketCommons;
    type Currency = Balances;
    type GlobalDisputeLockId = GlobalDisputeLockId;
    type GlobalDisputesPalletId = GlobalDisputesPalletId;
    type MaxGlobalDisputeVotes = MaxGlobalDisputeVotes;
    type MaxOwners = MaxOwners;
    type MinOutcomeVoteAmount = MinOutcomeVoteAmount;
    type RemoveKeysLimit = RemoveKeysLimit;
    type GdVotingPeriod = GdVotingPeriod;
    type VotingOutcomeFee = VotingOutcomeFee;
    type WeightInfo = pallet_pm_global_disputes::weights::WeightInfo<Runtime>;
}

impl pallet_treasury::Config for Runtime {
    type ApproveOrigin = EnsureSignedBy<Sudo, TestAccountIdPK>;
    type Burn = ();
    type BurnDestination = ();
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type MaxApprovals = MaxApprovals;
    type OnSlash = ();
    type PalletId = TreasuryPalletId;
    type ProposalBond = ();
    type ProposalBondMinimum = ();
    type ProposalBondMaximum = ();
    type RejectOrigin = EnsureSignedBy<Sudo, TestAccountIdPK>;
    type SpendFunds = ();
    type SpendOrigin = NeverEnsureOrigin<Balance>;
    type SpendPeriod = ();
    type WeightInfo = ();
}

type AssetMetadata =
    orml_traits::asset_registry::AssetMetadata<Balance, CustomMetadata, ConstU32<1024>>;

pub struct NoopAssetProcessor {}
impl AssetProcessor<CurrencyId, AssetMetadata> for NoopAssetProcessor {
    fn pre_register(
        id: Option<CurrencyId>,
        asset_metadata: AssetMetadata,
    ) -> Result<(CurrencyId, AssetMetadata), DispatchError> {
        Ok((id.unwrap(), asset_metadata))
    }
}

impl pallet_pm_eth_asset_registry::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CustomMetadata = CustomMetadata;
    type AssetId = CurrencyId;
    type AuthorityOrigin = EnsureRoot<TestAccountIdPK>;
    type Balance = Balance;
    type StringLimit = ConstU32<1024>;
    type AssetProcessor = NoopAssetProcessor;
    type WeightInfo = ();
}

#[allow(unused)]
pub struct ExtBuilder {
    balances: Vec<(TestAccountIdPK, Balance)>,
}

// TODO(#1222): Remove this in favor of adding whatever the account need in the individual tests.
#[allow(unused)]
impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            balances: vec![
                (alice(), 100_000_000_001 * _1),
                (charlie(), _1),
                (dave(), _1),
                (eve(), _1),
            ],
        }
    }
}

#[allow(unused)]
impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let keystore = MemoryKeystore::new();
        let mut t = frame_system::GenesisConfig::<Runtime>::default().build_storage().unwrap();
        // see the logs in tests when using `RUST_LOG=debug cargo test -- --nocapture`
        let _ = env_logger::builder().is_test(true).try_init();
        pallet_balances::GenesisConfig::<Runtime> { balances: self.balances }
            .assimilate_storage(&mut t)
            .unwrap();

        orml_tokens::GenesisConfig::<Runtime> {
            balances: vec![(alice(), FOREIGN_ASSET, 100_000_000_001 * _1)],
        }
        .assimilate_storage(&mut t)
        .unwrap();
        let custom_metadata = prediction_market_primitives::types::CustomMetadata {
            allow_as_base_asset: true,
            ..Default::default()
        };

        pallet_pm_eth_asset_registry::GenesisConfig::<Runtime> {
            assets: vec![(
                H160::from([1; 20]),
                FOREIGN_ASSET,
                AssetMetadata {
                    decimals: 18,
                    name: "MKL".as_bytes().to_vec().try_into().unwrap(),
                    symbol: "MKL".as_bytes().to_vec().try_into().unwrap(),
                    existential_deposit: 0,
                    location: None,
                    additional: custom_metadata,
                }
                .encode(),
            )],
            last_asset_id: FOREIGN_ASSET,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let mut test_ext: sp_io::TestExternalities = t.into();
        test_ext.register_extension(KeystoreExt(Arc::new(keystore)));
        test_ext.execute_with(|| System::set_block_number(1));
        test_ext
    }
}
