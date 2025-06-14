// Copyright 2023-2024 Forecasting Technologies LTD.
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

#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod consts;
mod helpers;
mod liquidity_tree;
mod macros;
mod math;
pub mod migration;
#[cfg(test)]
mod mock;
mod tests;
pub mod traits;
pub mod types;
pub mod weights;

pub use pallet::*;

pub const WITHDRAW_FEES_CONTEXT: &[u8] = b"neo_swap::withdraw_fees_context";
pub const EXIT_CONTEXT: &[u8] = b"neo_swap::exit_context";
pub const JOIN_CONTEXT: &[u8] = b"neo_swap::join_context";

#[frame_support::pallet]
mod pallet {
    use super::{EXIT_CONTEXT, JOIN_CONTEXT, WITHDRAW_FEES_CONTEXT};
    use crate::{
        consts::LN_NUMERICAL_LIMIT,
        liquidity_tree::types::{BenchmarkInfo, LiquidityTree, LiquidityTreeError},
        math::{Math, MathOps},
        traits::{pool_operations::PoolOperations, LiquiditySharesManager},
        types::{FeeDistribution, MaxAssets, Pool},
        weights::*,
    };
    use alloc::{collections::BTreeMap, vec, vec::Vec};
    use common_primitives::constants::currency::{BASE, CENT_BASE};
    use core::marker::PhantomData;
    use frame_support::{
        dispatch::DispatchResultWithPostInfo,
        ensure,
        pallet_prelude::{BuildGenesisConfig, OptionQuery, StorageMap, StorageValue},
        require_transactional,
        traits::{Get, IsSubType, IsType, StorageVersion},
        transactional, PalletError, PalletId, Parameter, Twox64Concat,
    };
    use frame_system::{
        ensure_signed,
        pallet_prelude::{BlockNumberFor, OriginFor},
    };
    use orml_traits::MultiCurrency;
    use pallet_pm_market_commons::MarketCommonsPalletApi;
    use parity_scale_codec::{Decode, Encode};
    use prediction_market_primitives::{
        hybrid_router_api_types::{AmmSoftFail, AmmTrade, ApiError},
        math::{
            checked_ops_res::{CheckedAddRes, CheckedSubRes},
            fixed::{BaseProvider, FixedDiv, FixedMul, PredictionMarketBase},
        },
        traits::{
            CompleteSetOperationsApi, DeployPoolApi, DistributeFees, HybridRouterAmmApi,
            OnLiquidityProvided, PalletAdminGetter,
        },
        types::{Asset, MarketStatus, ScoringRule},
    };
    use scale_info::{prelude::boxed::Box, TypeInfo};
    use sp_avn_common::{verify_signature, InnerCallValidator, Proof};
    use sp_runtime::{
        traits::{
            AccountIdConversion, CheckedSub, Dispatchable, IdentifyAccount, Member, Saturating,
            Verify, Zero,
        },
        DispatchError, DispatchResult, RuntimeDebug, SaturatedConversion,
    };

    pub(crate) const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

    // These should not be config parameters to avoid misconfigurations.
    pub(crate) const EXIT_FEE: u128 = CENT_BASE / 10; // 0.1%
    /// The minimum allowed swap fee. Hardcoded to avoid misconfigurations which may lead to
    /// exploits.
    pub(crate) const MIN_SWAP_FEE: u128 = BASE / 1_000; // 0.1%.
    /// The maximum allowed spot price when creating a pool.
    pub(crate) const MAX_SPOT_PRICE: u128 = BASE - CENT_BASE / 2;
    /// The minimum allowed spot price when creating a pool.
    pub(crate) const MIN_SPOT_PRICE: u128 = CENT_BASE / 2;
    /// The minimum vallowed value of a pool's liquidity parameter.
    pub(crate) const MIN_LIQUIDITY: u128 = BASE;
    /// The minimum percentage each new LP position must increase the liquidity by, represented as
    /// fractional (0.0139098411 represents 1.39098411%).
    pub(crate) const MIN_RELATIVE_LP_POSITION_VALUE: u128 = 139098411; // 1.39098411%

    pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    pub(crate) type AssetOf<T> = Asset<MarketIdOf<T>>;
    pub(crate) type BalanceOf<T> =
        <<T as Config>::MultiCurrency as MultiCurrency<AccountIdOf<T>>>::Balance;
    pub(crate) type AssetIndexType = u16;
    pub(crate) type MarketIdOf<T> =
        <<T as Config>::MarketCommons as MarketCommonsPalletApi>::MarketId;
    pub(crate) type LiquidityTreeOf<T> = LiquidityTree<T, <T as Config>::MaxLiquidityTreeDepth>;
    pub(crate) type PoolOf<T> = Pool<T, LiquidityTreeOf<T>, MaxAssets>;
    pub(crate) type AmmTradeOf<T> = AmmTrade<BalanceOf<T>>;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type CompleteSetOperations: CompleteSetOperationsApi<
            AccountId = Self::AccountId,
            Balance = BalanceOf<Self>,
            MarketId = MarketIdOf<Self>,
        >;

        /// Distribute external fees. The fees are paid from the pool account, which in turn has
        /// received the fees from the trader.
        type ExternalFees: DistributeFees<
            Asset = AssetOf<Self>,
            AccountId = AccountIdOf<Self>,
            Balance = BalanceOf<Self>,
            MarketId = MarketIdOf<Self>,
        >;

        type MarketCommons: MarketCommonsPalletApi<
            AccountId = Self::AccountId,
            BlockNumber = BlockNumberFor<Self>,
        >;

        type MultiCurrency: MultiCurrency<Self::AccountId, CurrencyId = AssetOf<Self>>;

        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The overarching call type.
        type RuntimeCall: Parameter
            + Dispatchable<RuntimeOrigin = <Self as frame_system::Config>::RuntimeOrigin>
            + IsSubType<Call<Self>>
            + From<Call<Self>>;

        type WeightInfo: WeightInfoZeitgeist;

        /// The maximum allowed liquidity tree depth per pool. Each pool can support
        /// `2^(depth + 1) - 1` liquidity providers. **Must** be less than 16.
        #[pallet::constant]
        type MaxLiquidityTreeDepth: Get<u32>;

        #[pallet::constant]
        type MaxSwapFee: Get<BalanceOf<Self>>;

        #[pallet::constant]
        type PalletId: Get<PalletId>;

        #[pallet::constant]
        type SignedTxLifetime: Get<u32>;

        type Public: IdentifyAccount<AccountId = Self::AccountId>;

        #[cfg(not(feature = "runtime-benchmarks"))]
        type Signature: Verify<Signer = Self::Public> + Member + Decode + Encode + TypeInfo;

        #[cfg(feature = "runtime-benchmarks")]
        type Signature: Verify<Signer = Self::Public>
            + Member
            + Decode
            + Encode
            + TypeInfo
            + From<sp_core::sr25519::Signature>;

        type PalletAdminGetter: PalletAdminGetter<AccountId = Self::AccountId>;

        type OnLiquidityProvided: OnLiquidityProvided<
            AccountId = Self::AccountId,
            MarketId = MarketIdOf<Self>,
        >;
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::storage]
    pub(crate) type Pools<T: Config> = StorageMap<_, Twox64Concat, MarketIdOf<T>, PoolOf<T>>;

    /// The account that receives the early exit fee
    #[pallet::storage]
    pub type EarlyExitFeeAccount<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// The amount of additional swap fee to be paid
    #[pallet::storage]
    pub type AdditionalSwapFee<T: Config> = StorageValue<_, BalanceOf<T>, OptionQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub additional_swap_fee: BalanceOf<T>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self { additional_swap_fee: 0u128.saturated_into() }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            AdditionalSwapFee::<T>::set(Some(self.additional_swap_fee.clone()));
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(fn deposit_event)]
    pub enum Event<T>
    where
        T: Config,
    {
        /// Informant bought a position. `amount_in` is the amount of collateral paid by `who`,
        /// including swap and external fees.
        BuyExecuted {
            who: T::AccountId,
            market_id: MarketIdOf<T>,
            asset_out: AssetOf<T>,
            amount_in: BalanceOf<T>,
            amount_out: BalanceOf<T>,
            swap_fee_amount: BalanceOf<T>,
            external_fee_amount: BalanceOf<T>,
        },
        /// Informant sold a position. `amount_out` is the amount of collateral received by `who`,
        /// with swap and external fees already deducted.
        SellExecuted {
            who: T::AccountId,
            market_id: MarketIdOf<T>,
            asset_in: AssetOf<T>,
            amount_in: BalanceOf<T>,
            amount_out: BalanceOf<T>,
            swap_fee_amount: BalanceOf<T>,
            external_fee_amount: BalanceOf<T>,
        },
        /// Liquidity provider withdrew fees.
        FeesWithdrawn { who: T::AccountId, market_id: MarketIdOf<T>, amount: BalanceOf<T> },
        /// Liquidity provider joined the pool.
        JoinExecuted {
            who: T::AccountId,
            market_id: MarketIdOf<T>,
            pool_shares_amount: BalanceOf<T>,
            amounts_in: Vec<BalanceOf<T>>,
            new_liquidity_parameter: BalanceOf<T>,
        },
        /// Liquidity provider left the pool.
        ExitExecuted {
            who: T::AccountId,
            market_id: MarketIdOf<T>,
            pool_shares_amount: BalanceOf<T>,
            amounts_out: Vec<BalanceOf<T>>,
            new_liquidity_parameter: BalanceOf<T>,
        },
        /// Pool was createed.
        PoolDeployed {
            who: T::AccountId,
            market_id: MarketIdOf<T>,
            account_id: T::AccountId,
            reserves: BTreeMap<AssetOf<T>, BalanceOf<T>>,
            collateral: AssetOf<T>,
            liquidity_parameter: BalanceOf<T>,
            pool_shares_amount: BalanceOf<T>,
            swap_fee: BalanceOf<T>,
        },
        /// Pool was destroyed.
        PoolDestroyed {
            who: T::AccountId,
            market_id: MarketIdOf<T>,
            amounts_out: Vec<BalanceOf<T>>,
        },
        /// A fee for the additional swap fee was set.
        AdditionalSwapFeeSet { new_fee: BalanceOf<T> },
        /// The account that receives the early exit fee was set.
        EarlyExitFeeAccountSet { new_account: T::AccountId },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The number of assets in the pool is above the allowed maximum.
        AssetCountAboveMax,
        /// Amount paid is above the specified maximum.
        AmountInAboveMax,
        /// Amount received is below the specified minimum.
        AmountOutBelowMin,
        /// Specified asset was not found in this pool.
        AssetNotFound,
        /// Market already has an associated pool.
        DuplicatePool,
        /// Incorrect asset count.
        IncorrectAssetCount,
        // Length of `max_amounts_in`, `max_amounts_out` or `spot_prices` must be equal to the
        // number of outcomes in the market.
        IncorrectVecLen,
        /// User doesn't own enough pool shares.
        InsufficientPoolShares,
        /// The liquidity in the pool is too low.
        LiquidityTooLow,
        /// Sum of spot prices is not `1`.
        InvalidSpotPrices,
        /// Market's trading mechanism is not LMSR.
        InvalidTradingMechanism,
        /// Pool can only be traded on if the market is active.
        MarketNotActive,
        /// Some calculation failed. This shouldn't happen.
        MathError,
        /// The user is not allowed to execute this command.
        NotAllowed,
        /// This feature is not yet implemented.
        NotImplemented,
        /// Some value in the operation is too large or small.
        NumericalLimits(NumericalLimitsError),
        /// Outstanding fees prevent liquidity withdrawal.
        OutstandingFees,
        /// Specified market does not have a pool.
        PoolNotFound,
        /// Spot price is above the allowed maximum.
        SpotPriceAboveMax,
        /// Spot price is below the allowed minimum.
        SpotPriceBelowMin,
        /// Pool's swap fee exceeds the allowed upper limit.
        SwapFeeAboveMax,
        /// Pool's swap fee is below the allowed lower limit.
        SwapFeeBelowMin,
        /// This shouldn't happen.
        Unexpected,
        /// Specified monetary amount is zero.
        ZeroAmount,
        /// An error occurred when handling the liquidty tree.
        LiquidityTreeError(LiquidityTreeError),
        /// The relative value of a new LP position is too low.
        MinRelativeLiquidityThresholdViolated,
        /// Narrowing type conversion occurred.
        NarrowingConversion,
        /// The sender is not the signer of the transaction
        SenderIsNotSigner,
        /// Signed transaction has failed validation
        UnauthorizedSignedTransaction,
        /// The signed transaction has expired
        SignedTransactionExpired,
        /// Early exit fee account must be set before using it
        EarlyExitFeeAccountNotSet,
        /// Additional swap fee must be set before using it
        AdditionalSwapFeeNotSet,
        /// The user is not the pallet admin
        SenderNotMarketAdmin,
    }

    #[derive(Decode, Encode, Eq, PartialEq, PalletError, RuntimeDebug, TypeInfo)]
    pub enum NumericalLimitsError {
        /// Selling is not allowed at prices this low.
        SpotPriceTooLow,
        /// Sells which move the price below this threshold are not allowed.
        SpotPriceSlippedTooLow,
        /// The maximum buy or sell amount was exceeded.
        MaxAmountExceeded,
        /// The minimum buy or sell amount was exceeded.
        MinAmountNotMet,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Buy outcome tokens from the specified market.
        ///
        /// The `amount_in` is paid in collateral. The transaction fails if the amount of outcome
        /// tokens received is smaller than `min_amount_out`. The user must correctly specify the
        /// number of outcomes for benchmarking reasons.
        ///
        /// The `amount_in` parameter must also satisfy lower and upper limits due to numerical
        /// constraints. In fact, after `amount_in` has been adjusted for fees, the following must
        /// hold:
        ///
        /// - `amount_in_minus_fees <= EXP_NUMERICAL_LIMIT * pool.liquidity_parameter`.
        /// - `exp(amount_in_minus_fees/pool.liquidity_parameter) - 1 + p <= LN_NUMERICAL_LIMIT`,
        ///   where `p` is the spot price of `asset_out`.
        ///
        /// # Parameters
        ///
        /// - `origin`: The origin account making the purchase.
        /// - `market_id`: Identifier for the market related to the trade.
        /// - `asset_count`: Number of assets in the pool.
        /// - `asset_out`: Asset to be purchased.
        /// - `amount_in`: Amount of collateral paid by the user.
        /// - `min_amount_out`: Minimum number of outcome tokens the user expects to receive.
        ///
        /// # Complexity
        ///
        /// Depends on the implementation of `CompleteSetOperationsApi` and `ExternalFees`; when
        /// using the canonical implementations, the runtime complexity is `O(asset_count)`.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::buy((*asset_count).saturated_into()))]
        #[transactional]
        pub fn buy(
            origin: OriginFor<T>,
            #[pallet::compact] market_id: MarketIdOf<T>,
            asset_count: AssetIndexType,
            asset_out: AssetOf<T>,
            #[pallet::compact] amount_in: BalanceOf<T>,
            #[pallet::compact] min_amount_out: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let asset_count_real = T::MarketCommons::market(&market_id)?.outcomes();
            ensure!(asset_count == asset_count_real, Error::<T>::IncorrectAssetCount);
            let _ = Self::do_buy(who, market_id, asset_out, amount_in, min_amount_out)?;
            Ok(Some(T::WeightInfo::buy(asset_count.into())).into())
        }

        /// Sell outcome tokens to the specified market.
        ///
        /// The `amount_in` is paid in outcome tokens. The transaction fails if the amount of
        /// outcome tokens received is smaller than `min_amount_out`. The user must
        /// correctly specify the number of outcomes for benchmarking reasons.
        ///
        /// The `amount_in` parameter must also satisfy lower and upper limits due to numerical
        /// constraints. In fact, the following must hold:
        ///
        /// - `amount_in <= EXP_NUMERICAL_LIMIT * pool.liquidity_parameter`.
        /// - The spot price of `asset_in` is greater than `exp(-EXP_NUMERICAL_LIMIT)` before and
        ///   after execution
        ///
        /// # Parameters
        ///
        /// - `origin`: The origin account making the sale.
        /// - `market_id`: Identifier for the market related to the trade.
        /// - `asset_count`: Number of assets in the pool.
        /// - `asset_in`: Asset to be sold.
        /// - `amount_in`: Amount of outcome tokens paid by the user.
        /// - `min_amount_out`: Minimum amount of collateral the user expects to receive.
        ///
        /// # Complexity
        ///
        /// Depends on the implementation of `CompleteSetOperationsApi` and `ExternalFees`; when
        /// using the canonical implementations, the runtime complexity is `O(asset_count)`.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::sell((*asset_count).saturated_into()))]
        #[transactional]
        pub fn sell(
            origin: OriginFor<T>,
            #[pallet::compact] market_id: MarketIdOf<T>,
            asset_count: AssetIndexType,
            asset_in: AssetOf<T>,
            #[pallet::compact] amount_in: BalanceOf<T>,
            #[pallet::compact] min_amount_out: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let asset_count_real = T::MarketCommons::market(&market_id)?.outcomes();
            ensure!(asset_count == asset_count_real, Error::<T>::IncorrectAssetCount);
            let _ = Self::do_sell(who, market_id, asset_in, amount_in, min_amount_out)?;
            Ok(Some(T::WeightInfo::sell(asset_count.into())).into())
        }

        /// Join the liquidity pool for the specified market.
        ///
        /// The LP receives pool shares in exchange for staking outcome tokens into the pool. The
        /// `max_amounts_in` vector specifies the maximum number of each outcome token that the LP
        /// is willing to deposit. These amounts are used to adjust the outcome balances in
        /// the pool according to the new proportion of pool shares owned by the LP.
        ///
        /// Note that the user must acquire the outcome tokens in a separate transaction, either by
        /// buying from the pool or by using complete set operations.
        ///
        /// # Parameters
        ///
        /// - `market_id`: Identifier for the market related to the pool.
        /// - `pool_shares_amount`: The number of new pool shares the LP will receive.
        /// - `max_amounts_in`: Vector of the maximum amounts of each outcome token the LP is
        ///   willing to deposit (with outcomes specified in the order of `MarketCommonsApi`).
        ///
        /// # Complexity
        ///
        /// `O(n + d)` where `n` is the number of assets in the pool and `d` is the depth of the
        /// pool's liquidity tree, or, equivalently, `log_2(m)` where `m` is the number of liquidity
        /// providers in the pool.
        #[pallet::call_index(2)]
        #[pallet::weight(
            T::WeightInfo::join_in_place(max_amounts_in.len().saturated_into())
                .max(T::WeightInfo::join_reassigned(max_amounts_in.len().saturated_into()))
                .max(T::WeightInfo::join_leaf(max_amounts_in.len().saturated_into()))
        )]
        #[transactional]
        pub fn join(
            origin: OriginFor<T>,
            #[pallet::compact] market_id: MarketIdOf<T>,
            #[pallet::compact] pool_shares_amount: BalanceOf<T>,
            max_amounts_in: Vec<BalanceOf<T>>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let asset_count = T::MarketCommons::market(&market_id)?.outcomes();
            let asset_count_usize: usize = asset_count.into();
            // Ensure that the conversion in the weight calculation doesn't saturate.
            let _: u32 =
                max_amounts_in.len().try_into().map_err(|_| Error::<T>::NarrowingConversion)?;
            ensure!(max_amounts_in.len() == asset_count_usize, Error::<T>::IncorrectVecLen);
            Self::do_join(who, market_id, pool_shares_amount, max_amounts_in)
        }

        /// Exit the liquidity pool for the specified market.
        ///
        /// The LP relinquishes pool shares in exchange for withdrawing outcome tokens from the
        /// pool. The `min_amounts_out` vector specifies the minimum number of each outcome token
        /// that the LP expects to withdraw. These minimum amounts are used to adjust the outcome
        /// balances in the pool, taking into account the reduction in the LP's pool share
        /// ownership.
        ///
        /// The transaction will fail unless the LP withdraws their fees from the pool beforehand. A
        /// batch transaction is very useful here.
        ///
        /// If the LP withdraws all pool shares that exist, then the pool is afterwards destroyed. A
        /// new pool can be deployed at any time, provided that the market is still open. If there
        /// are funds left in the pool account (this can happen due to exit fees), the remaining
        /// funds are destroyed.
        ///
        /// The LP is not allowed to leave a positive but small amount liquidity in the pool. If the
        /// liquidity parameter drops below a certain threshold, the transaction will fail. The only
        /// solution is to withdraw _all_ liquidity and let the pool die.
        ///
        /// # Parameters
        ///
        /// - `market_id`: Identifier for the market related to the pool.
        /// - `pool_shares_amount_out`: The number of pool shares the LP will relinquish.
        /// - `min_amounts_out`: Vector of the minimum amounts of each outcome token the LP expects
        ///   to withdraw (with outcomes specified in the order given by `MarketCommonsApi`).
        ///
        /// # Complexity
        ///
        /// `O(n + d)` where `n` is the number of assets in the pool and `d` is the depth of the
        /// pool's liquidity tree, or, equivalently, `log_2(m)` where `m` is the number of liquidity
        /// providers in the pool.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::exit(min_amounts_out.len().saturated_into()))]
        #[transactional]
        pub fn exit(
            origin: OriginFor<T>,
            #[pallet::compact] market_id: MarketIdOf<T>,
            #[pallet::compact] pool_shares_amount_out: BalanceOf<T>,
            min_amounts_out: Vec<BalanceOf<T>>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let asset_count = T::MarketCommons::market(&market_id)?.outcomes();
            let asset_count_u32: u32 = asset_count.into();
            let min_amounts_out_len: u32 =
                min_amounts_out.len().try_into().map_err(|_| Error::<T>::NarrowingConversion)?;
            ensure!(min_amounts_out_len == asset_count_u32, Error::<T>::IncorrectVecLen);
            Self::do_exit(who, market_id, pool_shares_amount_out, min_amounts_out)?;
            Ok(Some(T::WeightInfo::exit(min_amounts_out_len)).into())
        }

        /// Withdraw swap fees from the specified market.
        ///
        /// The transaction will fail if the caller is not a liquidity provider. Should always be
        /// used before calling `exit`.
        ///
        /// # Parameters
        ///
        /// - `market_id`: Identifier for the market related to the pool.
        ///
        /// # Complexity
        ///
        /// `O(1)`.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::withdraw_fees())]
        #[transactional]
        pub fn withdraw_fees(
            origin: OriginFor<T>,
            #[pallet::compact] market_id: MarketIdOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_withdraw_fees(who, market_id)?;
            Ok(())
        }

        /// Deploy a pool for the specified market and provide liquidity.
        ///
        /// The sender specifies a vector of `spot_prices` for the market's outcomes in the order
        /// given by the `MarketCommonsApi`. The transaction will fail if the spot prices don't add
        /// up to exactly `BASE`.
        ///
        /// Depending on the values in the `spot_prices`, the transaction will transfer different
        /// amounts of each outcome to the pool. The sender specifies a maximum `amount` of outcome
        /// tokens to spend.
        ///
        /// Note that the sender must acquire the outcome tokens in a separate transaction by using
        /// complete set operations. It's therefore convenient to batch this function together with
        /// a `buy_complete_set` with `amount` as amount of complete sets to buy.
        ///
        /// Deploying the pool will cost the signer an additional fee to the tune of the
        /// collateral's existential deposit. This fee is placed in the pool account and ensures
        /// that swap fees can be stored in the pool account without triggering dusting or failed
        /// transfers.
        ///
        /// The operation is currently limited to binary and scalar markets.
        ///
        /// # Complexity
        ///
        /// `O(n)` where `n` is the number of assets in the pool.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::deploy_pool(spot_prices.len().saturated_into()))]
        #[transactional]
        pub fn deploy_pool(
            origin: OriginFor<T>,
            #[pallet::compact] market_id: MarketIdOf<T>,
            #[pallet::compact] amount: BalanceOf<T>,
            spot_prices: Vec<BalanceOf<T>>,
            #[pallet::compact] swap_fee: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let asset_count = T::MarketCommons::market(&market_id)?.outcomes();
            let asset_count_u32: u32 = asset_count.into();
            let spot_prices_len: u32 =
                spot_prices.len().try_into().map_err(|_| Error::<T>::NarrowingConversion)?;
            ensure!(spot_prices_len == asset_count_u32, Error::<T>::IncorrectVecLen);
            Self::do_deploy_pool(who, market_id, amount, spot_prices, swap_fee)?;
            Ok(Some(T::WeightInfo::deploy_pool(spot_prices_len)).into())
        }

        // TODO update weight
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::signed_join(max_amounts_in.len().saturated_into()))]
        #[transactional]
        pub fn signed_join(
            origin: OriginFor<T>,
            proof: Proof<T::Signature, T::AccountId>,
            #[pallet::compact] market_id: MarketIdOf<T>,
            #[pallet::compact] pool_shares_amount: BalanceOf<T>,
            max_amounts_in: Vec<BalanceOf<T>>,
            block_number: BlockNumberFor<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            ensure!(who == proof.signer, Error::<T>::SenderIsNotSigner);
            ensure!(
                block_number.saturating_add(T::SignedTxLifetime::get().into()) >
                    frame_system::Pallet::<T>::block_number(),
                Error::<T>::SignedTransactionExpired
            );

            let encoded_payload = Self::encode_signed_join_params(
                &proof.relayer,
                &market_id,
                &pool_shares_amount,
                &max_amounts_in,
                &block_number,
            );

            ensure!(
                verify_signature::<T::Signature, T::AccountId>(&proof, &encoded_payload).is_ok(),
                Error::<T>::UnauthorizedSignedTransaction
            );

            let asset_count = T::MarketCommons::market(&market_id)?.outcomes();
            let asset_count_usize: usize = asset_count.into();
            // Ensure that the conversion in the weight calculation doesn't saturate.
            let _: u32 =
                max_amounts_in.len().try_into().map_err(|_| Error::<T>::NarrowingConversion)?;
            ensure!(max_amounts_in.len() == asset_count_usize, Error::<T>::IncorrectVecLen);

            Self::do_join(who.clone(), market_id, pool_shares_amount, max_amounts_in)?;

            Ok(().into())
        }

        // TODO update weight
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::signed_withdraw_fees())]
        #[transactional]
        pub fn signed_withdraw_fees(
            origin: OriginFor<T>,
            proof: Proof<T::Signature, T::AccountId>,
            #[pallet::compact] market_id: MarketIdOf<T>,
            block_number: BlockNumberFor<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            ensure!(who == proof.signer, Error::<T>::SenderIsNotSigner);
            ensure!(
                block_number.saturating_add(T::SignedTxLifetime::get().into()) >
                    frame_system::Pallet::<T>::block_number(),
                Error::<T>::SignedTransactionExpired
            );

            let encoded_payload =
                Self::encode_signed_withdraw_fees_params(&proof.relayer, &market_id, &block_number);

            ensure!(
                verify_signature::<T::Signature, T::AccountId>(&proof, &encoded_payload).is_ok(),
                Error::<T>::UnauthorizedSignedTransaction
            );

            Self::do_withdraw_fees(who, market_id)?;

            // TODO return weight
            Ok(().into())
        }

        // TODO update weight
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::signed_exit(min_amounts_out.len().saturated_into()))]
        #[transactional]
        pub fn signed_exit(
            origin: OriginFor<T>,
            proof: Proof<T::Signature, T::AccountId>,
            #[pallet::compact] market_id: MarketIdOf<T>,
            #[pallet::compact] pool_shares_amount_out: BalanceOf<T>,
            min_amounts_out: Vec<BalanceOf<T>>,
            block_number: BlockNumberFor<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            ensure!(who == proof.signer, Error::<T>::SenderIsNotSigner);

            ensure!(
                block_number.saturating_add(T::SignedTxLifetime::get().into()) >
                    frame_system::Pallet::<T>::block_number(),
                Error::<T>::SignedTransactionExpired
            );

            let encoded_payload = Self::encode_signed_exit_params(
                &proof.relayer,
                &market_id,
                &pool_shares_amount_out,
                &min_amounts_out,
                &block_number,
            );

            ensure!(
                verify_signature::<T::Signature, T::AccountId>(&proof, &encoded_payload).is_ok(),
                Error::<T>::UnauthorizedSignedTransaction
            );

            let asset_count = T::MarketCommons::market(&market_id)?.outcomes();
            let asset_count_u32: u32 = asset_count.into();
            let min_amounts_out_len: u32 =
                min_amounts_out.len().try_into().map_err(|_| Error::<T>::NarrowingConversion)?;
            ensure!(min_amounts_out_len == asset_count_u32, Error::<T>::IncorrectVecLen);
            Self::do_exit(who, market_id, pool_shares_amount_out, min_amounts_out)?;

            // TODO return weight
            Ok(().into())
        }

        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::set_early_exit_fee_account())]
        #[transactional]
        pub fn set_early_exit_fee_account(
            origin: OriginFor<T>,
            account: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(who == T::PalletAdminGetter::get_admin()?, Error::<T>::SenderNotMarketAdmin);

            <EarlyExitFeeAccount<T>>::mutate(|a| *a = Some(account.clone()));
            Self::deposit_event(Event::EarlyExitFeeAccountSet { new_account: account });

            Ok(())
        }

        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::set_additional_swap_fee())]
        #[transactional]
        pub fn set_additional_swap_fee(origin: OriginFor<T>, fee: BalanceOf<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(who == T::PalletAdminGetter::get_admin()?, Error::<T>::SenderNotMarketAdmin);

            <AdditionalSwapFee<T>>::mutate(|f| *f = Some(fee));
            Self::deposit_event(Event::AdditionalSwapFeeSet { new_fee: fee });

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        #[require_transactional]
        fn do_buy(
            who: T::AccountId,
            market_id: MarketIdOf<T>,
            asset_out: AssetOf<T>,
            amount_in: BalanceOf<T>,
            min_amount_out: BalanceOf<T>,
        ) -> Result<AmmTradeOf<T>, DispatchError> {
            ensure!(amount_in != Zero::zero(), Error::<T>::ZeroAmount);
            let market = T::MarketCommons::market(&market_id)?;
            ensure!(market.status == MarketStatus::Active, Error::<T>::MarketNotActive);
            Self::try_mutate_pool(&market_id, |pool| {
                ensure!(pool.contains(&asset_out), Error::<T>::AssetNotFound);
                T::MultiCurrency::transfer(pool.collateral, &who, &pool.account_id, amount_in)?;
                let FeeDistribution {
                    remaining: amount_in_minus_fees,
                    swap_fees: swap_fee_amount,
                    external_fees: external_fee_amount,
                } = Self::distribute_fees(market_id, pool, amount_in)?;
                ensure!(
                    amount_in_minus_fees <= pool.calculate_numerical_threshold(),
                    Error::<T>::NumericalLimits(NumericalLimitsError::MaxAmountExceeded),
                );
                ensure!(
                    pool.calculate_buy_ln_argument(asset_out, amount_in_minus_fees)? >=
                        LN_NUMERICAL_LIMIT.saturated_into(),
                    Error::<T>::NumericalLimits(NumericalLimitsError::MinAmountNotMet),
                );
                let swap_amount_out =
                    pool.calculate_swap_amount_out_for_buy(asset_out, amount_in_minus_fees)?;
                let amount_out = swap_amount_out.checked_add_res(&amount_in_minus_fees)?;
                ensure!(amount_out >= min_amount_out, Error::<T>::AmountOutBelowMin);
                // Instead of letting `who` buy the complete sets and then transfer almost all of
                // the outcomes to the pool account, we prevent `(n-1)` storage reads by using the
                // pool account to buy. Note that the fees are already in the pool at this point.
                T::CompleteSetOperations::buy_complete_set(
                    pool.account_id.clone(),
                    market_id,
                    amount_in_minus_fees,
                )?;
                T::MultiCurrency::transfer(asset_out, &pool.account_id, &who, amount_out)?;
                for asset in pool.assets().iter() {
                    pool.increase_reserve(asset, &amount_in_minus_fees)?;
                    if *asset == asset_out {
                        pool.decrease_reserve(asset, &amount_out)?;
                    }
                }
                Self::deposit_event(Event::<T>::BuyExecuted {
                    who: who.clone(),
                    market_id,
                    asset_out,
                    amount_in,
                    amount_out,
                    swap_fee_amount,
                    external_fee_amount,
                });
                Ok(AmmTrade { amount_in, amount_out, swap_fee_amount, external_fee_amount })
            })
        }

        #[require_transactional]
        fn do_sell(
            who: T::AccountId,
            market_id: MarketIdOf<T>,
            asset_in: AssetOf<T>,
            amount_in: BalanceOf<T>,
            min_amount_out: BalanceOf<T>,
        ) -> Result<AmmTradeOf<T>, DispatchError> {
            ensure!(amount_in != Zero::zero(), Error::<T>::ZeroAmount);
            let market = T::MarketCommons::market(&market_id)?;
            ensure!(market.status == MarketStatus::Active, Error::<T>::MarketNotActive);
            Self::try_mutate_pool(&market_id, |pool| {
                ensure!(pool.contains(&asset_in), Error::<T>::AssetNotFound);
                // Ensure that the price of `asset_in` is at least `exp(-EXP_NUMERICAL_LIMITS) =
                // 4.5399...e-05`.
                ensure!(
                    pool.reserve_of(&asset_in)? <= pool.calculate_numerical_threshold(),
                    Error::<T>::NumericalLimits(NumericalLimitsError::SpotPriceTooLow),
                );
                ensure!(
                    amount_in <= pool.calculate_numerical_threshold(),
                    Error::<T>::NumericalLimits(NumericalLimitsError::MaxAmountExceeded),
                );
                // Instead of first executing a swap with `(n-1)` transfers from the pool account to
                // `who` and then selling complete sets, we prevent `(n-1)` storage reads: 1)
                // Transfer `amount_in` units of `asset_in` to the pool account, 2) sell
                // `amount_out` complete sets using the pool account, 3) transfer
                // `amount_out_minus_fees` units of collateral to `who`. The fees automatically end
                // up in the pool.
                let amount_out = pool.calculate_swap_amount_out_for_sell(asset_in, amount_in)?;
                // Beware! This transfer **must** happen _after_ calculating `amount_out`:
                T::MultiCurrency::transfer(asset_in, &who, &pool.account_id, amount_in)?;
                T::CompleteSetOperations::sell_complete_set(
                    pool.account_id.clone(),
                    market_id,
                    amount_out,
                )?;
                let FeeDistribution {
                    remaining: amount_out_minus_fees,
                    swap_fees: swap_fee_amount,
                    external_fees: external_fee_amount,
                } = Self::distribute_fees(market_id, pool, amount_out)?;
                ensure!(amount_out_minus_fees >= min_amount_out, Error::<T>::AmountOutBelowMin);
                T::MultiCurrency::transfer(
                    pool.collateral,
                    &pool.account_id,
                    &who,
                    amount_out_minus_fees,
                )?;
                for asset in pool.assets().iter() {
                    if *asset == asset_in {
                        pool.increase_reserve(asset, &amount_in)?;
                    }
                    pool.decrease_reserve(asset, &amount_out)?;
                }
                // Ensure that the sell doesn't move the price below the minimum defined by
                // `EXP_NUMERICAL_LIMITS` (see comment above).
                ensure!(
                    pool.reserve_of(&asset_in)? <= pool.calculate_numerical_threshold(),
                    Error::<T>::NumericalLimits(NumericalLimitsError::SpotPriceSlippedTooLow),
                );
                Self::deposit_event(Event::<T>::SellExecuted {
                    who: who.clone(),
                    market_id,
                    asset_in,
                    amount_in,
                    amount_out: amount_out_minus_fees,
                    swap_fee_amount,
                    external_fee_amount,
                });
                Ok(AmmTrade {
                    amount_in,
                    amount_out: amount_out_minus_fees,
                    swap_fee_amount,
                    external_fee_amount,
                })
            })
        }

        #[require_transactional]
        fn do_join(
            who: T::AccountId,
            market_id: MarketIdOf<T>,
            pool_shares_amount: BalanceOf<T>,
            max_amounts_in: Vec<BalanceOf<T>>,
        ) -> DispatchResultWithPostInfo {
            ensure!(pool_shares_amount != Zero::zero(), Error::<T>::ZeroAmount);
            let market = T::MarketCommons::market(&market_id)?;
            ensure!(market.status == MarketStatus::Active, Error::<T>::MarketNotActive);
            let asset_count_u16: u16 =
                max_amounts_in.len().try_into().map_err(|_| Error::<T>::NarrowingConversion)?;
            let asset_count_u32: u32 = asset_count_u16.into();
            ensure!(asset_count_u16 == market.outcomes(), Error::<T>::IncorrectAssetCount);
            let benchmark_info = Self::try_mutate_pool(&market_id, |pool| {
                let ratio =
                    pool_shares_amount.bdiv_ceil(pool.liquidity_shares_manager.total_shares()?)?;
                // Ensure that new LPs contribute at least MIN_RELATIVE_LP_POSITION_VALUE. Note that
                // this ensures that the ratio can never be zero.
                if pool.liquidity_shares_manager.shares_of(&who).is_err() {
                    ensure!(
                        ratio >= MIN_RELATIVE_LP_POSITION_VALUE.saturated_into(),
                        Error::<T>::MinRelativeLiquidityThresholdViolated,
                    );
                }
                let mut amounts_in = vec![];
                for (&asset, &max_amount_in) in pool.assets().iter().zip(max_amounts_in.iter()) {
                    let balance_in_pool = pool.reserve_of(&asset)?;
                    let amount_in = ratio.bmul_ceil(balance_in_pool)?;
                    amounts_in.push(amount_in);
                    ensure!(amount_in <= max_amount_in, Error::<T>::AmountInAboveMax);
                    T::MultiCurrency::transfer(asset, &who, &pool.account_id, amount_in)?;
                }
                for ((_, balance), amount_in) in pool.reserves.iter_mut().zip(amounts_in.iter()) {
                    *balance = balance.checked_add_res(amount_in)?;
                }
                let benchmark_info =
                    pool.liquidity_shares_manager.join(&who, pool_shares_amount)?;
                let new_liquidity_parameter = pool
                    .liquidity_parameter
                    .checked_add_res(&ratio.bmul(pool.liquidity_parameter)?)?;
                pool.liquidity_parameter = new_liquidity_parameter;
                Self::deposit_event(Event::<T>::JoinExecuted {
                    who: who.clone(),
                    market_id,
                    pool_shares_amount,
                    amounts_in,
                    new_liquidity_parameter,
                });

                // Notify other pallets that liquidity has been provided.
                T::OnLiquidityProvided::on_liquidity_provided(&market_id, &who);

                Ok(benchmark_info)
            })?;
            let weight = match benchmark_info {
                BenchmarkInfo::InPlace => T::WeightInfo::join_in_place(asset_count_u32),
                BenchmarkInfo::Reassigned => T::WeightInfo::join_reassigned(asset_count_u32),
                BenchmarkInfo::Leaf => T::WeightInfo::join_leaf(asset_count_u32),
            };
            Ok((Some(weight)).into())
        }

        #[require_transactional]
        fn do_exit(
            who: T::AccountId,
            market_id: MarketIdOf<T>,
            pool_shares_amount: BalanceOf<T>,
            min_amounts_out: Vec<BalanceOf<T>>,
        ) -> DispatchResult {
            ensure!(pool_shares_amount != Zero::zero(), Error::<T>::ZeroAmount);
            let market = T::MarketCommons::market(&market_id)?;
            Pools::<T>::try_mutate_exists(market_id, |maybe_pool| {
                let pool =
                    maybe_pool.as_mut().ok_or::<DispatchError>(Error::<T>::PoolNotFound.into())?;
                let ratio = {
                    let mut ratio = pool_shares_amount
                        .bdiv_floor(pool.liquidity_shares_manager.total_shares()?)?;
                    if market.status == MarketStatus::Active {
                        let multiplier = PredictionMarketBase::<BalanceOf<T>>::get()?
                            .checked_sub_res(&EXIT_FEE.saturated_into())?;
                        ratio = ratio.bmul_floor(multiplier)?;
                    }
                    ratio
                };
                let mut amounts_out = vec![];
                for (&asset, &min_amount_out) in pool.assets().iter().zip(min_amounts_out.iter()) {
                    let balance_in_pool = pool.reserve_of(&asset)?;
                    let amount_out = ratio.bmul_floor(balance_in_pool)?;
                    amounts_out.push(amount_out);
                    ensure!(amount_out >= min_amount_out, Error::<T>::AmountOutBelowMin);
                    T::MultiCurrency::transfer(asset, &pool.account_id, &who, amount_out)?;
                }
                for ((_, balance), amount_out) in pool.reserves.iter_mut().zip(amounts_out.iter()) {
                    *balance = balance.checked_sub_res(amount_out)?;
                }
                pool.liquidity_shares_manager.exit(&who, pool_shares_amount)?;
                if pool.liquidity_shares_manager.total_shares()? == Zero::zero() {
                    let withdraw_remaining = |&asset| -> DispatchResult {
                        let remaining = T::MultiCurrency::free_balance(asset, &pool.account_id);
                        T::MultiCurrency::withdraw(asset, &pool.account_id, remaining)?;
                        Ok(())
                    };

                    // Transfer any remaining base assets to the designated account.
                    let remaining =
                        T::MultiCurrency::free_balance(pool.collateral, &pool.account_id);
                    T::MultiCurrency::transfer(
                        pool.collateral,
                        &pool.account_id,
                        &Self::early_exit_account()?,
                        remaining,
                    )?;

                    // Clear left-over tokens. These naturally occur in the form of exit fees.
                    for asset in pool.assets().iter() {
                        withdraw_remaining(asset)?;
                    }
                    *maybe_pool = None; // Delete the storage map entry.
                    Self::deposit_event(Event::<T>::PoolDestroyed {
                        who: who.clone(),
                        market_id,
                        amounts_out,
                    });
                } else {
                    let old_liquidity_parameter = pool.liquidity_parameter;
                    let new_liquidity_parameter = old_liquidity_parameter
                        .checked_sub_res(&ratio.bmul(old_liquidity_parameter)?)?;
                    // If `who` still holds pool shares, check that their position has at least
                    // minimum size.
                    if let Ok(remaining_pool_shares_amount) =
                        pool.liquidity_shares_manager.shares_of(&who)
                    {
                        ensure!(
                            new_liquidity_parameter >= MIN_LIQUIDITY.saturated_into(),
                            Error::<T>::LiquidityTooLow
                        );
                        let remaining_pool_shares_ratio = remaining_pool_shares_amount
                            .bdiv_floor(pool.liquidity_shares_manager.total_shares()?)?;
                        ensure!(
                            remaining_pool_shares_ratio >=
                                MIN_RELATIVE_LP_POSITION_VALUE.saturated_into(),
                            Error::<T>::MinRelativeLiquidityThresholdViolated
                        );
                    }
                    pool.liquidity_parameter = new_liquidity_parameter;
                    Self::deposit_event(Event::<T>::ExitExecuted {
                        who: who.clone(),
                        market_id,
                        pool_shares_amount,
                        amounts_out,
                        new_liquidity_parameter,
                    });
                }
                Ok(())
            })
        }

        #[require_transactional]
        fn do_withdraw_fees(who: T::AccountId, market_id: MarketIdOf<T>) -> DispatchResult {
            Self::try_mutate_pool(&market_id, |pool| {
                let amount = pool.liquidity_shares_manager.withdraw_fees(&who)?;
                T::MultiCurrency::transfer(pool.collateral, &pool.account_id, &who, amount)?; // Should never fail.
                Self::deposit_event(Event::<T>::FeesWithdrawn {
                    who: who.clone(),
                    market_id,
                    amount,
                });
                Ok(())
            })
        }

        #[require_transactional]
        fn do_deploy_pool(
            who: T::AccountId,
            market_id: MarketIdOf<T>,
            amount: BalanceOf<T>,
            spot_prices: Vec<BalanceOf<T>>,
            swap_fee: BalanceOf<T>,
        ) -> DispatchResult {
            ensure!(!Pools::<T>::contains_key(market_id), Error::<T>::DuplicatePool);
            let market = T::MarketCommons::market(&market_id)?;
            ensure!(market.status == MarketStatus::Active, Error::<T>::MarketNotActive);
            ensure!(
                market.scoring_rule == ScoringRule::AmmCdaHybrid,
                Error::<T>::InvalidTradingMechanism
            );
            let asset_count_u16: u16 =
                spot_prices.len().try_into().map_err(|_| Error::<T>::NarrowingConversion)?;
            let asset_count_u32: u32 = asset_count_u16.into();
            ensure!(asset_count_u16 == market.outcomes(), Error::<T>::IncorrectVecLen);
            ensure!(asset_count_u32 <= MaxAssets::get(), Error::<T>::AssetCountAboveMax);
            ensure!(swap_fee >= MIN_SWAP_FEE.saturated_into(), Error::<T>::SwapFeeBelowMin);
            ensure!(swap_fee <= T::MaxSwapFee::get(), Error::<T>::SwapFeeAboveMax);
            ensure!(
                spot_prices
                    .iter()
                    .fold(Zero::zero(), |acc: BalanceOf<T>, &val| acc.saturating_add(val)) ==
                    BASE.saturated_into(),
                Error::<T>::InvalidSpotPrices
            );
            for &p in spot_prices.iter() {
                ensure!(
                    p.saturated_into::<u128>() >= MIN_SPOT_PRICE,
                    Error::<T>::SpotPriceBelowMin
                );
                ensure!(
                    p.saturated_into::<u128>() <= MAX_SPOT_PRICE,
                    Error::<T>::SpotPriceAboveMax
                );
            }
            let (liquidity_parameter, amounts_in) =
                Math::<T>::calculate_reserves_from_spot_prices(amount, spot_prices)?;
            ensure!(
                liquidity_parameter >= MIN_LIQUIDITY.saturated_into(),
                Error::<T>::LiquidityTooLow
            );
            let pool_account_id = Self::pool_account_id(&market_id);
            let mut reserves = BTreeMap::new();
            for (&amount_in, &asset) in amounts_in.iter().zip(market.outcome_assets().iter()) {
                T::MultiCurrency::transfer(asset, &who, &pool_account_id, amount_in)?;
                let _ = reserves.insert(asset, amount_in);
            }
            let collateral = market.base_asset;
            let pool = Pool {
                account_id: pool_account_id.clone(),
                reserves: reserves.clone().try_into().map_err(|_| Error::<T>::Unexpected)?,
                collateral,
                liquidity_parameter,
                liquidity_shares_manager: LiquidityTree::new(who.clone(), amount)?,
                swap_fee,
            };
            // TODO(#1220): Ensure that the existential deposit doesn't kill fees. This is an ugly
            // hack and system should offer the option to whitelist accounts.
            T::MultiCurrency::transfer(
                pool.collateral,
                &who,
                &pool.account_id,
                T::MultiCurrency::minimum_balance(collateral),
            )?;
            Pools::<T>::insert(market_id, pool);

            // Notify other pallets that liquidity has been provided.
            T::OnLiquidityProvided::on_liquidity_provided(&market_id, &who);

            Self::deposit_event(Event::<T>::PoolDeployed {
                who,
                market_id,
                account_id: pool_account_id,
                reserves,
                collateral,
                liquidity_parameter,
                pool_shares_amount: amount,
                swap_fee,
            });
            Ok(())
        }

        #[inline]
        pub(crate) fn pool_account_id(market_id: &MarketIdOf<T>) -> T::AccountId {
            T::PalletId::get().into_sub_account_truncating((*market_id).saturated_into::<u128>())
        }

        pub fn early_exit_account() -> Result<T::AccountId, Error<T>> {
            Ok(<EarlyExitFeeAccount<T>>::get().ok_or(Error::<T>::EarlyExitFeeAccountNotSet)?)
        }

        pub fn additional_swap_fee() -> Result<BalanceOf<T>, Error<T>> {
            Ok(<AdditionalSwapFee<T>>::get().ok_or(Error::<T>::AdditionalSwapFeeNotSet)?)
        }

        /// Distribute swap fees and external fees and returns the remaining amount.
        ///
        /// # Arguments
        ///
        /// - `market_id`: The ID of the market to which the pool belongs.
        /// - `pool`: The pool on which the trade was executed.
        /// - `amount`: The gross amount from which the fee is deduced.
        ///
        /// Will fail if the total amount of fees is more than the gross amount. In particular, the
        /// function will fail if the external fees exceed the gross amount.
        #[require_transactional]
        fn distribute_fees(
            market_id: MarketIdOf<T>,
            pool: &mut PoolOf<T>,
            amount: BalanceOf<T>,
        ) -> Result<FeeDistribution<T>, DispatchError> {
            let swap_fees = pool.swap_fee.bmul(amount)?;
            pool.liquidity_shares_manager.deposit_fees(swap_fees)?; // Should only error unexpectedly!
            let external_fees =
                T::ExternalFees::distribute(market_id, pool.collateral, &pool.account_id, amount);
            let total_fees = external_fees.saturating_add(swap_fees);
            let remaining = amount.checked_sub(&total_fees).ok_or(Error::<T>::Unexpected)?;
            Ok(FeeDistribution { remaining, swap_fees, external_fees })
        }

        pub(crate) fn try_mutate_pool<R, F>(
            market_id: &MarketIdOf<T>,
            mutator: F,
        ) -> Result<R, DispatchError>
        where
            F: FnMut(&mut PoolOf<T>) -> Result<R, DispatchError>,
        {
            Pools::<T>::try_mutate(market_id, |maybe_pool| {
                maybe_pool.as_mut().ok_or(Error::<T>::PoolNotFound.into()).and_then(mutator)
            })
        }
    }

    impl<T: Config> DeployPoolApi for Pallet<T> {
        type AccountId = T::AccountId;
        type Balance = BalanceOf<T>;
        type MarketId = MarketIdOf<T>;

        fn deploy_pool(
            who: Self::AccountId,
            market_id: Self::MarketId,
            amount: Self::Balance,
            spot_prices: Vec<Self::Balance>,
            swap_fee: Self::Balance,
        ) -> DispatchResult {
            Self::do_deploy_pool(who, market_id, amount, spot_prices, swap_fee)
        }
    }

    impl<T: Config> Pallet<T> {
        fn amount_including_fee_surplus(
            amount: BalanceOf<T>,
            fee_fractional: BalanceOf<T>,
        ) -> Result<BalanceOf<T>, DispatchError> {
            let fee_divisor = PredictionMarketBase::<BalanceOf<T>>::get()?
                .checked_sub(&fee_fractional)
                .ok_or(Error::<T>::Unexpected)?;
            amount.bdiv(fee_divisor)
        }

        fn match_failure(error: DispatchError) -> ApiError<AmmSoftFail> {
            let spot_price_too_low: DispatchError =
                Error::<T>::NumericalLimits(NumericalLimitsError::SpotPriceTooLow).into();
            let spot_price_slipped_too_low: DispatchError =
                Error::<T>::NumericalLimits(NumericalLimitsError::SpotPriceSlippedTooLow).into();
            let max_amount_exceeded: DispatchError =
                Error::<T>::NumericalLimits(NumericalLimitsError::MaxAmountExceeded).into();
            let min_amount_not_met: DispatchError =
                Error::<T>::NumericalLimits(NumericalLimitsError::MinAmountNotMet).into();
            if spot_price_too_low == error ||
                spot_price_slipped_too_low == error ||
                max_amount_exceeded == error ||
                min_amount_not_met == error
            {
                ApiError::SoftFailure(AmmSoftFail::Numerical)
            } else {
                ApiError::HardFailure(error)
            }
        }
    }

    impl<T: Config> HybridRouterAmmApi for Pallet<T> {
        type AccountId = T::AccountId;
        type MarketId = MarketIdOf<T>;
        type Balance = BalanceOf<T>;
        type Asset = AssetOf<T>;

        fn pool_exists(market_id: Self::MarketId) -> bool {
            Pools::<T>::contains_key(market_id)
        }

        fn get_spot_price(
            market_id: Self::MarketId,
            asset: Self::Asset,
        ) -> Result<Self::Balance, DispatchError> {
            let pool = Pools::<T>::get(market_id).ok_or(Error::<T>::PoolNotFound)?;
            pool.calculate_spot_price(asset)
        }

        fn calculate_buy_amount_until(
            market_id: Self::MarketId,
            asset: Self::Asset,
            until: Self::Balance,
        ) -> Result<Self::Balance, DispatchError> {
            let pool = Pools::<T>::get(market_id).ok_or(Error::<T>::PoolNotFound)?;
            let buy_amount = pool.calculate_buy_amount_until(asset, until)?;
            let total_fee_fractional =
                pool.swap_fee.checked_add_res(&Self::additional_swap_fee()?)?;
            let buy_amount_plus_fees =
                Self::amount_including_fee_surplus(buy_amount, total_fee_fractional)?;
            Ok(buy_amount_plus_fees)
        }

        fn buy(
            who: Self::AccountId,
            market_id: Self::MarketId,
            asset_out: Self::Asset,
            amount_in: Self::Balance,
            min_amount_out: Self::Balance,
        ) -> Result<AmmTradeOf<T>, ApiError<AmmSoftFail>> {
            Self::do_buy(who, market_id, asset_out, amount_in, min_amount_out)
                .map_err(Self::match_failure)
        }

        fn calculate_sell_amount_until(
            market_id: Self::MarketId,
            asset: Self::Asset,
            until: Self::Balance,
        ) -> Result<Self::Balance, DispatchError> {
            let pool = Pools::<T>::get(market_id).ok_or(Error::<T>::PoolNotFound)?;
            pool.calculate_sell_amount_until(asset, until)
        }

        fn sell(
            who: Self::AccountId,
            market_id: Self::MarketId,
            asset_out: Self::Asset,
            amount_in: Self::Balance,
            min_amount_out: Self::Balance,
        ) -> Result<AmmTradeOf<T>, ApiError<AmmSoftFail>> {
            Self::do_sell(who, market_id, asset_out, amount_in, min_amount_out)
                .map_err(Self::match_failure)
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn encode_signed_join_params(
            relayer: &T::AccountId,
            market_id: &MarketIdOf<T>,
            pool_shares: &BalanceOf<T>,
            max_amounts_in: &Vec<BalanceOf<T>>,
            block_number: &BlockNumberFor<T>,
        ) -> Vec<u8> {
            (JOIN_CONTEXT, relayer, market_id, pool_shares, max_amounts_in, block_number).encode()
        }

        pub fn encode_signed_withdraw_fees_params(
            relayer: &T::AccountId,
            market_id: &MarketIdOf<T>,
            block_number: &BlockNumberFor<T>,
        ) -> Vec<u8> {
            (WITHDRAW_FEES_CONTEXT, relayer, market_id, block_number).encode()
        }

        pub fn encode_signed_exit_params(
            relayer: &T::AccountId,
            market_id: &MarketIdOf<T>,
            pool_shares: &BalanceOf<T>,
            min_amounts_out: &Vec<BalanceOf<T>>,
            block_number: &BlockNumberFor<T>,
        ) -> Vec<u8> {
            (EXIT_CONTEXT, relayer, market_id, pool_shares, min_amounts_out, block_number).encode()
        }

        pub fn get_encoded_call_param(
            call: &<T as Config>::RuntimeCall,
        ) -> Option<(&Proof<T::Signature, T::AccountId>, Vec<u8>)> {
            let call = match call.is_sub_type() {
                Some(call) => call,
                None => return None,
            };

            match call {
                Call::signed_join {
                    ref proof,
                    ref market_id,
                    ref pool_shares_amount,
                    ref max_amounts_in,
                    ref block_number,
                } => {
                    let encoded_data = Self::encode_signed_join_params(
                        &proof.relayer,
                        market_id,
                        pool_shares_amount,
                        max_amounts_in,
                        block_number,
                    );

                    Some((proof, encoded_data))
                },
                Call::signed_exit {
                    ref proof,
                    ref market_id,
                    ref pool_shares_amount_out,
                    ref min_amounts_out,
                    ref block_number,
                } => {
                    let encoded_data = Self::encode_signed_exit_params(
                        &proof.relayer,
                        market_id,
                        pool_shares_amount_out,
                        min_amounts_out,
                        block_number,
                    );

                    Some((proof, encoded_data))
                },
                Call::signed_withdraw_fees { ref proof, ref market_id, ref block_number } => {
                    let encoded_data = Self::encode_signed_withdraw_fees_params(
                        &proof.relayer,
                        market_id,
                        block_number,
                    );

                    Some((proof, encoded_data))
                },

                _ => None,
            }
        }
    }

    impl<T: Config> InnerCallValidator for Pallet<T> {
        type Call = <T as Config>::RuntimeCall;

        fn signature_is_valid(call: &Box<Self::Call>) -> bool {
            if let Some((proof, signed_payload)) = Self::get_encoded_call_param(call) {
                return verify_signature::<T::Signature, T::AccountId>(
                    &proof,
                    &signed_payload.as_slice(),
                )
                .is_ok();
            }

            return false;
        }
    }
}
