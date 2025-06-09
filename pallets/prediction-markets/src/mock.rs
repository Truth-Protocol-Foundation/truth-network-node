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

#![allow(
    // Mocks are only used for fuzzing and unit tests
    clippy::arithmetic_side_effects
)]
#![cfg(feature = "mock")]

use crate as prediction_markets;
use crate::{AssetOf, BalanceOf, EthEvent, MarketIdOf, TokenInterface};
use common_primitives::{
    constants::MILLISECS_PER_BLOCK,
    types::{Balance, BlockNumber, Hash, Moment},
};
use core::marker::PhantomData;
use frame_support::{
    construct_runtime, ord_parameter_types, parameter_types,
    traits::{Everything, NeverEnsureOrigin, OnFinalize, OnInitialize},
};
use frame_system::{mocking::MockBlockU32, EnsureRoot, EnsureSignedBy};
use orml_traits::{asset_registry::AssetProcessor, MultiCurrency};
use parity_scale_codec::{alloc::sync::Arc, Encode};
pub use prediction_market_primitives::test_helper::get_account;
use prediction_market_primitives::{
    constants::mock::{
        AddOutcomePeriod, AggregationPeriod, AppealBond, AppealPeriod, AuthorizedPalletId,
        BlockHashCount, BlocksPerYear, CloseEarlyBlockPeriod, CloseEarlyDisputeBond,
        CloseEarlyProtectionBlockPeriod, CloseEarlyProtectionTimeFramePeriod,
        CloseEarlyRequestBond, CloseEarlyTimeFramePeriod, CorrectionPeriod, CourtPalletId,
        ExistentialDeposit, ExistentialDeposits, GdVotingPeriod, GetNativeCurrencyId,
        GlobalDisputeLockId, GlobalDisputesPalletId, InflationPeriod, LockId, MaxAppeals,
        MaxApprovals, MaxCategories, MaxCourtParticipants, MaxCreatorFee, MaxDelegations,
        MaxDisputeDuration, MaxDisputes, MaxEditReasonLen, MaxGlobalDisputeVotes, MaxGracePeriod,
        MaxLocks, MaxMarketLifetime, MaxOracleDuration, MaxOwners, MaxRejectReasonLen, MaxReserves,
        MaxSelectedDraws, MaxYearlyInflation, MinCategories, MinDisputeDuration, MinJurorStake,
        MinOracleDuration, MinOutcomeVoteAmount, MinimumPeriod, OutsiderBond, PmPalletId,
        RemoveKeysLimit, RequestInterval, TreasuryPalletId, VotePeriod, VotingOutcomeFee, BASE,
        CENT_BASE,
    },
    traits::{DeployPoolApi, DistributeFees},
    types::{
        Asset, BasicCurrencyAdapter, BlockTest, CurrencyId, CustomMetadata, MarketId, OrmlAmount,
        SignatureTest, TestAccountIdPK,
    },
};
use sp_arithmetic::{per_things::Percent, Perbill};
use sp_core::{Get, H160};
use sp_keystore::{testing::MemoryKeystore, KeystoreExt};
use sp_runtime::{
    traits::{BlakeTwo256, ConstU32, IdentityLookup, Zero},
    BuildStorage, DispatchError, DispatchResult, SaturatedConversion,
};
use std::cell::RefCell;

use pallet_pm_eth_asset_registry;

pub fn alice() -> TestAccountIdPK {
    get_account(0u8)
}
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
pub fn fred() -> TestAccountIdPK {
    get_account(5u8)
}
pub fn sudo() -> TestAccountIdPK {
    get_account(69u8)
}
pub fn approve_origin() -> TestAccountIdPK {
    get_account(70u8)
}
pub fn reject_origin() -> TestAccountIdPK {
    get_account(71u8)
}
pub fn close_market_early_origin() -> TestAccountIdPK {
    get_account(72u8)
}
pub fn close_origin() -> TestAccountIdPK {
    get_account(73u8)
}
pub fn request_edit_origin() -> TestAccountIdPK {
    get_account(74u8)
}
pub fn resolve_origin() -> TestAccountIdPK {
    get_account(75u8)
}
pub fn winning_fee_account() -> TestAccountIdPK {
    get_account(95u8)
}
pub fn market_admin() -> TestAccountIdPK {
    get_account(17u8)
}
pub const INITIAL_BALANCE: u128 = 1_000 * BASE;

#[allow(unused)]
pub struct DeployPoolMock;

#[allow(unused)]
#[derive(Clone)]
pub struct DeployPoolArgs {
    who: TestAccountIdPK,
    market_id: MarketId,
    amount: Balance,
    swap_prices: Vec<Balance>,
    swap_fee: Balance,
}

thread_local! {
    pub static DEPLOY_POOL_CALL_DATA: RefCell<Vec<DeployPoolArgs>> = const { RefCell::new(vec![]) };
    pub static DEPLOY_POOL_RETURN_VALUE: RefCell<DispatchResult> = const { RefCell::new(Ok(())) };
}

#[allow(unused)]
impl DeployPoolApi for DeployPoolMock {
    type AccountId = TestAccountIdPK;
    type Balance = Balance;
    type MarketId = MarketId;

    fn deploy_pool(
        who: Self::AccountId,
        market_id: Self::MarketId,
        amount: Self::Balance,
        swap_prices: Vec<Self::Balance>,
        swap_fee: Self::Balance,
    ) -> DispatchResult {
        DEPLOY_POOL_CALL_DATA.with(|value| {
            value.borrow_mut().push(DeployPoolArgs {
                who,
                market_id,
                amount,
                swap_prices,
                swap_fee,
            })
        });
        DEPLOY_POOL_RETURN_VALUE.with(|v| *v.borrow())
    }
}

#[allow(unused)]
impl DeployPoolMock {
    pub fn called_once_with(
        who: TestAccountIdPK,
        market_id: MarketId,
        amount: Balance,
        swap_prices: Vec<Balance>,
        swap_fee: Balance,
    ) -> bool {
        if DEPLOY_POOL_CALL_DATA.with(|value| value.borrow().len()) != 1 {
            return false;
        }
        let args = DEPLOY_POOL_CALL_DATA.with(|value| value.borrow()[0].clone());
        args.who == who &&
            args.market_id == market_id &&
            args.amount == amount &&
            args.swap_prices == swap_prices &&
            args.swap_fee == swap_fee
    }

    pub fn return_error() {
        DEPLOY_POOL_RETURN_VALUE
            .with(|value| *value.borrow_mut() = Err(DispatchError::Other("neo-swaps")));
    }
}

ord_parameter_types! {
    pub const Sudo: TestAccountIdPK = sudo();
    pub const ApproveOrigin: TestAccountIdPK = approve_origin();
    pub const RejectOrigin: TestAccountIdPK = reject_origin();
    pub const CloseMarketEarlyOrigin: TestAccountIdPK = close_market_early_origin();
    pub const CloseOrigin: TestAccountIdPK = close_origin();
    pub const RequestEditOrigin: TestAccountIdPK = request_edit_origin();
    pub const ResolveOrigin: TestAccountIdPK = resolve_origin();
}

parameter_types! {
    pub const AdvisoryBond: Balance = 11 * CENT_BASE;
    pub const AdvisoryBondSlashPercentage: Percent = Percent::from_percent(10);
    pub const OracleBond: Balance = 25 * CENT_BASE;
    pub const ValidityBond: Balance = 53 * CENT_BASE;
    pub const DisputeBond: Balance = 109 * CENT_BASE;
    pub const WinnerFeePercentage: Perbill = Perbill::from_percent(5);
    pub FeeAccount: TestAccountIdPK = winning_fee_account();

}

construct_runtime!(
    pub enum Runtime {
        Authorized: pallet_pm_authorized,
        Balances: pallet_balances,
        Court: pallet_pm_court,
        AssetManager: orml_currencies,
        MarketCommons: pallet_pm_market_commons,
        PredictionMarkets: prediction_markets,
        RandomnessCollectiveFlip: pallet_insecure_randomness_collective_flip,
        GlobalDisputes: pallet_pm_global_disputes,
        System: frame_system,
        Timestamp: pallet_timestamp,
        Tokens: orml_tokens,
        Treasury: pallet_treasury,
        AssetRegistry: pallet_pm_eth_asset_registry,
        AVN: pallet_avn,
    }
);

impl pallet_avn::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AuthorityId = pallet_avn::sr25519::AuthorityId;
    type EthereumPublicKeyChecker = ();
    type NewSessionHandler = ();
    type DisabledValidatorChecker = ();
    type WeightInfo = ();
}

pub struct NoopTokenInterface {}
impl TokenInterface<H160, TestAccountIdPK> for NoopTokenInterface {
    fn process_lift(_event: &EthEvent) -> DispatchResult {
        Ok(())
    }

    fn deposit_tokens(
        _token_id: H160,
        _recipient_account_id: TestAccountIdPK,
        _raw_amount: u128,
    ) -> DispatchResult {
        Ok(())
    }
}

pub fn fee_percentage<T: crate::Config>() -> Perbill {
    WinnerFeePercentage::get()
}

pub fn calculate_fee<T: crate::Config>(amount: BalanceOf<T>) -> BalanceOf<T> {
    fee_percentage::<T>().mul_floor(amount.saturated_into::<BalanceOf<T>>())
}

pub struct WinningFees<T, F>(PhantomData<T>, PhantomData<F>);

impl<T: crate::Config, F> DistributeFees for WinningFees<T, F>
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
        let fees = calculate_fee::<T>(amount);
        match T::AssetManager::transfer(asset, account, &F::get(), fees) {
            Ok(_) => fees,
            Err(_) => Zero::zero(),
        }
    }
}

impl crate::Config for Runtime {
    type AdvisoryBond = AdvisoryBond;
    type AdvisoryBondSlashPercentage = AdvisoryBondSlashPercentage;
    type ApproveOrigin = EnsureSignedBy<ApproveOrigin, TestAccountIdPK>;
    type AssetRegistry = AssetRegistry;
    type Authorized = Authorized;
    type CloseEarlyDisputeBond = CloseEarlyDisputeBond;
    type CloseMarketEarlyOrigin = EnsureSignedBy<CloseMarketEarlyOrigin, TestAccountIdPK>;
    type CloseEarlyProtectionTimeFramePeriod = CloseEarlyProtectionTimeFramePeriod;
    type CloseEarlyProtectionBlockPeriod = CloseEarlyProtectionBlockPeriod;
    type CloseEarlyRequestBond = CloseEarlyRequestBond;
    type CloseOrigin = EnsureSignedBy<CloseOrigin, TestAccountIdPK>;
    type Currency = Balances;
    type MaxCreatorFee = MaxCreatorFee;
    type Court = Court;
    type DeployPool = DeployPoolMock;
    type DisputeBond = DisputeBond;
    type RuntimeEvent = RuntimeEvent;
    type GlobalDisputes = GlobalDisputes;
    type MaxCategories = MaxCategories;
    type MaxDisputes = MaxDisputes;
    type MinDisputeDuration = MinDisputeDuration;
    type MinOracleDuration = MinOracleDuration;
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
    type CloseEarlyBlockPeriod = CloseEarlyBlockPeriod;
    type CloseEarlyTimeFramePeriod = CloseEarlyTimeFramePeriod;
    type RejectOrigin = EnsureSignedBy<RejectOrigin, TestAccountIdPK>;
    type RequestEditOrigin = EnsureSignedBy<RequestEditOrigin, TestAccountIdPK>;
    type ResolveOrigin = EnsureSignedBy<ResolveOrigin, TestAccountIdPK>;
    type AssetManager = AssetManager;
    type Slash = Treasury;
    type ValidityBond = ValidityBond;
    type WeightInfo = prediction_markets::weights::WeightInfo<Runtime>;
    type RuntimeCall = RuntimeCall;
    type Public = TestAccountIdPK;
    type Signature = SignatureTest;
    type TokenInterface = NoopTokenInterface;
    type WinnerFeePercentage = WinnerFeePercentage;
    type WinnerFeeHandler = WinningFees<Runtime, FeeAccount>;
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
    type Nonce = u32;
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

impl orml_tokens::Config for Runtime {
    type Amount = OrmlAmount;
    type Balance = Balance;
    type CurrencyId = CurrencyId;
    type DustRemovalWhitelist = Everything;
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposits = ExistentialDeposits;
    type MaxLocks = ();
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

impl pallet_insecure_randomness_collective_flip::Config for Runtime {}

impl pallet_timestamp::Config for Runtime {
    type MinimumPeriod = MinimumPeriod;
    type Moment = Moment;
    type OnTimestampSet = ();
    type WeightInfo = ();
}

ord_parameter_types! {
    pub const AuthorizedDisputeResolutionUser: TestAccountIdPK = alice();
}

impl pallet_pm_authorized::Config for Runtime {
    type AuthorizedDisputeResolutionOrigin =
        EnsureSignedBy<AuthorizedDisputeResolutionUser, TestAccountIdPK>;
    type CorrectionPeriod = CorrectionPeriod;
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type DisputeResolution = prediction_markets::Pallet<Runtime>;
    type MarketCommons = MarketCommons;
    type PalletId = AuthorizedPalletId;
    type WeightInfo = pallet_pm_authorized::weights::WeightInfo<Runtime>;
}

impl pallet_pm_court::Config for Runtime {
    type AppealBond = AppealBond;
    type BlocksPerYear = BlocksPerYear;
    type DisputeResolution = prediction_markets::Pallet<Runtime>;
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

impl pallet_pm_market_commons::Config for Runtime {
    type Balance = Balance;
    type MarketId = MarketId;
    type Timestamp = Timestamp;
}

impl pallet_pm_global_disputes::Config for Runtime {
    type AddOutcomePeriod = AddOutcomePeriod;
    type RuntimeEvent = RuntimeEvent;
    type DisputeResolution = prediction_markets::Pallet<Runtime>;
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

pub struct ExtBuilder {
    balances: Vec<(TestAccountIdPK, Balance)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        DEPLOY_POOL_CALL_DATA.with(|value| value.borrow_mut().clear());
        Self {
            balances: vec![
                (alice(), INITIAL_BALANCE),
                (bob(), INITIAL_BALANCE),
                (charlie(), INITIAL_BALANCE),
                (dave(), INITIAL_BALANCE),
                (eve(), INITIAL_BALANCE),
                (fred(), INITIAL_BALANCE),
                (sudo(), INITIAL_BALANCE),
            ],
        }
    }
}

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
            balances: (0..69)
                .map(|idx| (get_account(idx), Asset::ForeignAsset(100), INITIAL_BALANCE))
                .collect(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let custom_metadata = prediction_market_primitives::types::CustomMetadata {
            allow_as_base_asset: true,
            ..Default::default()
        };

        pallet_pm_eth_asset_registry::GenesisConfig::<Runtime> {
            assets: vec![
                (
                    H160::from([1; 20]),
                    Asset::ForeignAsset(100),
                    AssetMetadata {
                        decimals: 18,
                        name: "ACALA USD".as_bytes().to_vec().try_into().unwrap(),
                        symbol: "AUSD".as_bytes().to_vec().try_into().unwrap(),
                        existential_deposit: 0,
                        location: None,
                        additional: custom_metadata,
                    }
                    .encode(),
                ),
                (
                    H160::from([2; 20]),
                    Asset::ForeignAsset(420),
                    AssetMetadata {
                        decimals: 18,
                        name: "FANCY_TOKEN".as_bytes().to_vec().try_into().unwrap(),
                        symbol: "FTK".as_bytes().to_vec().try_into().unwrap(),
                        existential_deposit: 0,
                        location: None,
                        additional: prediction_market_primitives::types::CustomMetadata::default(),
                    }
                    .encode(),
                ),
            ],
            last_asset_id: Asset::ForeignAsset(420),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        prediction_markets::GenesisConfig::<Runtime> {
            vault_account: Some(sudo()),
            market_admin: Some(market_admin()),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let mut test_ext: sp_io::TestExternalities = t.into();
        test_ext.register_extension(KeystoreExt(Arc::new(keystore)));
        test_ext.execute_with(|| System::set_block_number(1));
        test_ext
    }
}

pub fn run_to_block(n: BlockNumber) {
    while System::block_number() < n {
        Balances::on_finalize(System::block_number());
        Court::on_finalize(System::block_number());
        PredictionMarkets::on_finalize(System::block_number());
        System::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        System::on_initialize(System::block_number());
        PredictionMarkets::on_initialize(System::block_number());
        Court::on_initialize(System::block_number());
        Balances::on_initialize(System::block_number());
    }
}

pub fn run_blocks(n: BlockNumber) {
    run_to_block(System::block_number() + n);
}

// Our `on_initialize` compensates for the fact that `on_initialize` takes the timestamp from the
// previous block. Therefore, manually setting timestamp during tests becomes cumbersome without a
// utility function like this.
pub fn set_timestamp_for_on_initialize(time: Moment) {
    Timestamp::set_timestamp(time - MILLISECS_PER_BLOCK as u64);
}

type Block = MockBlockU32<Runtime>;

sp_api::mock_impl_runtime_apis! {
    impl pallet_prediction_markets_runtime_api::PredictionMarketsApi<BlockTest<Runtime>, MarketId, Hash> for Runtime {
        fn market_outcome_share_id(_: MarketId, _: u16) -> Asset<MarketId> {
            Asset::PoolShare(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;

    // We run this test to ensure that bonds are mutually non-equal (some of the tests in
    // `tests.rs` require this to be true).
    #[test]
    fn test_bonds_are_pairwise_non_equal() {
        assert_ne!(
            <Runtime as Config>::AdvisoryBond::get(),
            <Runtime as Config>::OracleBond::get()
        );
        assert_ne!(
            <Runtime as Config>::AdvisoryBond::get(),
            <Runtime as Config>::ValidityBond::get()
        );
        assert_ne!(
            <Runtime as Config>::AdvisoryBond::get(),
            <Runtime as Config>::DisputeBond::get()
        );
        assert_ne!(
            <Runtime as Config>::OracleBond::get(),
            <Runtime as Config>::ValidityBond::get()
        );
        assert_ne!(<Runtime as Config>::OracleBond::get(), <Runtime as Config>::DisputeBond::get());
        assert_ne!(
            <Runtime as Config>::ValidityBond::get(),
            <Runtime as Config>::DisputeBond::get()
        );
    }
}
