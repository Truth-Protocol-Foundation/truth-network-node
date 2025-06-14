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
    // Auto-generated code is a no man's land
    clippy::arithmetic_side_effects
)]
#![allow(clippy::type_complexity)]
#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::signed_calls::{
    BUY_COMPLETE_SET_CONTEXT, CREATE_MARKET_AND_DEPLOY_POOL_CONTEXT, REDEEM_SHARES,
    REPORT_OUTCOME_CONTEXT, TRANSFER_TOKENS_CONTEXT, WITHDRAW_TOKENS_CONTEXT,
};

#[cfg(test)]
use crate::Pallet as PredictionMarket;
use alloc::{vec, vec::Vec};
use common_primitives::constants::{
    currency::{BASE, CENT_BASE},
    MILLISECS_PER_BLOCK,
};
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::{
    traits::{EnsureOrigin, Get, Hooks, UnfilteredDispatchable},
    BoundedVec,
};
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};
use orml_traits::{asset_registry::AssetMetadata, MultiCurrency};
use pallet_pm_authorized::Pallet as AuthorizedPallet;
use pallet_pm_global_disputes::GlobalDisputesPalletApi;
use pallet_pm_market_commons::MarketCommonsPalletApi;
use prediction_market_primitives::{
    constants::mock::{CloseEarlyProtectionTimeFramePeriod, CloseEarlyTimeFramePeriod},
    math::fixed::{BaseProvider, PredictionMarketBase},
    traits::DisputeApi,
    types::{
        Asset, Deadlines, MarketCreation, MarketDisputeMechanism, MarketPeriod, MarketStatus,
        MarketType, MultiHash, OutcomeReport, ScoringRule,
    },
};
use sp_core::{crypto::DEV_PHRASE, H160, H256};
use sp_runtime::{
    traits::{SaturatedConversion, Saturating, Zero},
    DispatchError, Perbill, RuntimeAppPublic,
};

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

const LIQUIDITY: u128 = 100 * BASE;

// Get default values for market creation. Also spawns an account with maximum
// amount of native currency
fn create_market_common_parameters<T: Config>(
    is_disputable: bool,
    maybe_caller: Option<T::AccountId>,
) -> Result<(T::AccountId, T::AccountId, Deadlines<BlockNumberFor<T>>, MultiHash), &'static str> {
    let mut caller: T::AccountId = whitelisted_caller();
    if let Some(actual_caller) = maybe_caller {
        caller = actual_caller;
    }

    T::AssetManager::deposit(Asset::Tru, &caller, (100000u128 * BASE).saturated_into()).unwrap();
    let oracle = caller.clone();
    let deadlines = Deadlines::<BlockNumberFor<T>> {
        grace_period: 1_u32.into(),
        oracle_duration: T::MinOracleDuration::get(),
        dispute_duration: if is_disputable { T::MinDisputeDuration::get() } else { Zero::zero() },
    };
    let mut metadata = [0u8; 50];
    metadata[0] = 0x15;
    metadata[1] = 0x30;
    Ok((caller, oracle, deadlines, MultiHash::Sha3_384(metadata)))
}

// Create a market based on common parameters
fn create_market_common<T: Config + pallet_timestamp::Config>(
    creation: MarketCreation,
    options: MarketType,
    scoring_rule: ScoringRule,
    period: Option<MarketPeriod<BlockNumberFor<T>, MomentOf<T>>>,
    dispute_mechanism: Option<MarketDisputeMechanism>,
    maybe_caller: Option<T::AccountId>,
) -> Result<(T::AccountId, MarketIdOf<T>), &'static str> {
    pallet_timestamp::Pallet::<T>::set_timestamp(0u32.into());
    let range_start: MomentOf<T> = 100_000u64.saturated_into();
    let range_end: MomentOf<T> = 1_000_000u64.saturated_into();
    let creator_fee: Perbill = Perbill::zero();
    let period = period.unwrap_or(MarketPeriod::Timestamp(range_start..range_end));
    let (caller, oracle, deadlines, metadata) =
        create_market_common_parameters::<T>(dispute_mechanism.is_some(), maybe_caller)?;
    WhitelistedMarketCreators::<T>::insert(&caller, ());
    Call::<T>::create_market {
        base_asset: Asset::Tru,
        creator_fee,
        oracle,
        period,
        deadlines,
        metadata,
        creation,
        market_type: options,
        dispute_mechanism,
        scoring_rule,
    }
    .dispatch_bypass_filter(RawOrigin::Signed(caller.clone()).into())?;
    let market_id = pallet_pm_market_commons::Pallet::<T>::latest_market_id()?;
    Ok((caller, market_id))
}

fn create_close_and_report_market<T: Config + pallet_timestamp::Config>(
    permission: MarketCreation,
    options: MarketType,
    outcome: OutcomeReport,
) -> Result<(T::AccountId, MarketIdOf<T>), &'static str> {
    let range_start: MomentOf<T> = 100_000u64.saturated_into();
    let range_end: MomentOf<T> = 1_000_000u64.saturated_into();
    let period = MarketPeriod::Timestamp(range_start..range_end);
    let (caller, market_id) = create_market_common::<T>(
        permission,
        options,
        ScoringRule::AmmCdaHybrid,
        Some(period),
        Some(MarketDisputeMechanism::Court),
        None,
    )?;
    Call::<T>::admin_move_market_to_closed { market_id }
        .dispatch_bypass_filter(T::CloseOrigin::try_successful_origin().unwrap())?;
    let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
    let end: u32 = match market.period {
        MarketPeriod::Timestamp(range) => range.end.saturated_into::<u32>(),
        _ => {
            return Err("MarketPeriod is block_number based");
        },
    };
    let grace_period: u32 =
        (market.deadlines.grace_period.saturated_into::<u32>() + 1) * MILLISECS_PER_BLOCK;
    pallet_timestamp::Pallet::<T>::set_timestamp((end + grace_period).into());
    Call::<T>::report { market_id, outcome }
        .dispatch_bypass_filter(RawOrigin::Signed(caller.clone()).into())?;
    Ok((caller, market_id))
}

// Setup a categorical market for fn `internal_resolve`
fn setup_redeem_shares_common<T: Config + pallet_timestamp::Config>(
    market_type: MarketType,
    caller_account_id: &Option<T::AccountId>,
) -> Result<(T::AccountId, MarketIdOf<T>), &'static str> {
    let (caller, market_id) = create_market_common::<T>(
        MarketCreation::Permissionless,
        market_type.clone(),
        ScoringRule::AmmCdaHybrid,
        None,
        Some(MarketDisputeMechanism::Court),
        caller_account_id.clone(),
    )?;
    let outcome: OutcomeReport;

    if let MarketType::Categorical(categories) = market_type {
        outcome = OutcomeReport::Categorical(categories.saturating_sub(1));
    } else if let MarketType::Scalar(range) = market_type {
        outcome = OutcomeReport::Scalar(*range.end());
    } else {
        panic!("setup_redeem_shares_common: Unsupported market type: {market_type:?}");
    }

    Call::<T>::buy_complete_set { market_id, amount: LIQUIDITY.saturated_into() }
        .dispatch_bypass_filter(RawOrigin::Signed(caller.clone()).into())?;
    let close_origin = T::CloseOrigin::try_successful_origin().unwrap();
    let resolve_origin = T::ResolveOrigin::try_successful_origin().unwrap();
    Call::<T>::admin_move_market_to_closed { market_id }.dispatch_bypass_filter(close_origin)?;
    let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
    let end: u32 = match market.period {
        MarketPeriod::Timestamp(range) => range.end.saturated_into::<u32>(),
        _ => {
            return Err("MarketPeriod is block_number based");
        },
    };
    let grace_period: u32 =
        (market.deadlines.grace_period.saturated_into::<u32>() + 1) * MILLISECS_PER_BLOCK;
    pallet_timestamp::Pallet::<T>::set_timestamp((end + grace_period).into());
    Call::<T>::report { market_id, outcome }
        .dispatch_bypass_filter(RawOrigin::Signed(caller.clone()).into())?;
    Call::<T>::admin_move_market_to_resolved { market_id }
        .dispatch_bypass_filter(resolve_origin)?;
    Ok((caller, market_id))
}

fn create_market_and_pool<T: Config + pallet_timestamp::Config>(
    caller_account_id: &Option<T::AccountId>,
    categories: u32,
) -> Result<(T::AccountId, MarketIdOf<T>), &'static str> {
    let range_start: MomentOf<T> = pallet_pm_market_commons::Pallet::<T>::now();
    let range_end: MomentOf<T> = 1_000_000u64.saturated_into();
    let (caller, market_id) = create_market_common::<T>(
        MarketCreation::Permissionless,
        MarketType::Categorical(categories.saturated_into()),
        ScoringRule::AmmCdaHybrid,
        Some(MarketPeriod::Timestamp(range_start..range_end)),
        Some(MarketDisputeMechanism::Court),
        caller_account_id.clone(),
    )?;

    Call::<T>::buy_complete_set { market_id, amount: LIQUIDITY.saturated_into() }
        .dispatch_bypass_filter(RawOrigin::Signed(caller.clone()).into())?;

    Ok((caller, market_id))
}

fn setup_reported_categorical_market<T>(
    categories: u32,
    report_outcome: OutcomeReport,
) -> Result<(T::AccountId, MarketIdOf<T>), &'static str>
where
    T: Config + pallet_timestamp::Config,
{
    let (caller, market_id) = create_market_common::<T>(
        MarketCreation::Permissionless,
        MarketType::Categorical(categories.saturated_into()),
        ScoringRule::AmmCdaHybrid,
        None,
        Some(MarketDisputeMechanism::Court),
        None,
    )?;

    Call::<T>::admin_move_market_to_closed { market_id }
        .dispatch_bypass_filter(T::CloseOrigin::try_successful_origin().unwrap())?;
    let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
    let end: u32 = match market.period {
        MarketPeriod::Timestamp(range) => range.end.saturated_into::<u32>(),
        _ => {
            return Err("MarketPeriod is block_number based");
        },
    };
    let grace_period: u32 =
        (market.deadlines.grace_period.saturated_into::<u32>() + 1) * MILLISECS_PER_BLOCK;
    pallet_timestamp::Pallet::<T>::set_timestamp((end + grace_period).into());
    Call::<T>::report { market_id, outcome: report_outcome }
        .dispatch_bypass_filter(RawOrigin::Signed(caller.clone()).into())?;

    Ok((caller, market_id))
}

fn create_spot_prices<T: Config>(asset_count: u16) -> Vec<BalanceOf<T>> {
    let mut result = vec![CENT_BASE.saturated_into(); (asset_count - 1) as usize];
    let remaining_u128 =
        PredictionMarketBase::<u128>::get().unwrap() - (asset_count - 1) as u128 * CENT_BASE;
    result.push(remaining_u128.saturated_into());
    result
}

fn into_bytes<T: Config>(account: &<T as pallet_avn::Config>::AuthorityId) -> [u8; 32]
where
    T: Config + pallet_avn::Config,
{
    let bytes = account.encode();
    let mut vector: [u8; 32] = Default::default();
    vector.copy_from_slice(&bytes[0..32]);
    return vector;
}

fn get_user_account<T: Config>() -> (<T as pallet_avn::Config>::AuthorityId, T::AccountId)
where
    T: Config + pallet_avn::Config,
{
    let mnemonic: &str = DEV_PHRASE;
    let key_pair =
        <T as pallet_avn::Config>::AuthorityId::generate_pair(Some(mnemonic.as_bytes().to_vec()));
    let account_bytes = into_bytes::<T>(&key_pair);
    let account_id = T::AccountId::decode(&mut &account_bytes.encode()[..]).unwrap();
    return (key_pair, account_id);
}

fn get_relayer<T: Config>() -> T::AccountId {
    let relayer_account: H256 = H256::repeat_byte(1);
    return T::AccountId::decode(&mut relayer_account.as_bytes()).expect("valid relayer account id");
}

fn get_proof<T: Config>(
    signer: T::AccountId,
    relayer: T::AccountId,
    signature: &[u8],
) -> Proof<T::Signature, T::AccountId> {
    return Proof {
        signer: signer.clone(),
        relayer: relayer.clone(),
        signature: sp_core::sr25519::Signature::from_slice(signature).unwrap().into(),
    };
}

fn do_report_trusted_market<T: Config>(
    caller_account_id: &Option<T::AccountId>,
) -> Result<MarketIdOf<T>, DispatchError>
where
    T: Config + pallet_timestamp::Config,
{
    pallet_timestamp::Pallet::<T>::set_timestamp(0u32.into());
    let start: MomentOf<T> = pallet_pm_market_commons::Pallet::<T>::now();
    let end: MomentOf<T> = 1_000_000u64.saturated_into();
    let (caller, _oracle, _, metadata) =
        create_market_common_parameters::<T>(false, caller_account_id.clone())?;
    WhitelistedMarketCreators::<T>::insert(&caller, ());
    Call::<T>::create_market {
        base_asset: Asset::Tru,
        creator_fee: Perbill::zero(),
        oracle: caller.clone(),
        period: MarketPeriod::Timestamp(start..end),
        deadlines: Deadlines::<BlockNumberFor<T>> {
            grace_period: 0u8.into(),
            oracle_duration: T::MinOracleDuration::get(),
            dispute_duration: 0u8.into(),
        },
        metadata,
        creation: MarketCreation::Permissionless,
        market_type: MarketType::Categorical(3),
        dispute_mechanism: None,
        scoring_rule: ScoringRule::AmmCdaHybrid,
    }
    .dispatch_bypass_filter(RawOrigin::Signed(caller.clone()).into())
    .map_err(|e| e.error)?;
    let market_id = pallet_pm_market_commons::Pallet::<T>::latest_market_id()?;
    let close_origin = T::CloseOrigin::try_successful_origin().unwrap();
    Pallet::<T>::admin_move_market_to_closed(close_origin, market_id).map_err(|e| e.error)?;
    return Ok(market_id);
}

fn do_report_market_with_dispute_mechanism<T: Config>(
    m: u32,
    caller_account_id: &Option<T::AccountId>,
    dispute_mechanism: Option<MarketDisputeMechanism>,
    expire_reporting_period: bool,
) -> Result<MarketIdOf<T>, DispatchError>
where
    T: Config + pallet_timestamp::Config,
{
    // ensure range.start is now to get the heaviest path
    let range_start: MomentOf<T> = pallet_pm_market_commons::Pallet::<T>::now();
    let range_end: MomentOf<T> = 1_000_000u64.saturated_into();
    let (caller, market_id) = create_market_common::<T>(
        MarketCreation::Permissionless,
        MarketType::Categorical(T::MaxCategories::get()),
        ScoringRule::AmmCdaHybrid,
        Some(MarketPeriod::Timestamp(range_start..range_end)),
        dispute_mechanism,
        caller_account_id.clone(),
    )?;

    pallet_pm_market_commons::Pallet::<T>::mutate_market(&market_id, |market| {
        // ensure sender is oracle to succeed extrinsic call
        market.oracle = caller.clone();
        Ok(())
    })?;

    let close_origin = T::CloseOrigin::try_successful_origin().unwrap();
    Pallet::<T>::admin_move_market_to_closed(close_origin, market_id).map_err(|e| e.error)?;
    let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
    let end: u32 = match market.period {
        MarketPeriod::Timestamp(range) => range.end.saturated_into::<u32>(),
        _ => {
            return Err(DispatchError::Other("foo"));
        },
    };
    let mut end_period: u32 =
        (market.deadlines.grace_period.saturated_into::<u32>() + 1) * MILLISECS_PER_BLOCK;

    if expire_reporting_period {
        end_period +=
            (market.deadlines.oracle_duration.saturated_into::<u32>() + 1) * MILLISECS_PER_BLOCK;
    }

    pallet_timestamp::Pallet::<T>::set_timestamp((end + end_period).into());

    let report_at = frame_system::Pallet::<T>::block_number();
    let resolves_at = report_at.saturating_add(market.deadlines.dispute_duration);
    for i in 0..m {
        MarketIdsPerReportBlock::<T>::try_mutate(resolves_at, |ids| ids.try_push(i.into()))
            .unwrap();
    }

    return Ok(market_id);
}

benchmarks! {
    where_clause {
        where
            T: pallet_timestamp::Config + pallet_pm_authorized::Config + pallet_pm_court::Config + pallet_avn::Config + pallet_pm_eth_asset_registry::Config,
            <<T as pallet_pm_authorized::Config>::MarketCommons as MarketCommonsPalletApi>::MarketId:
                From<<T as pallet_pm_market_commons::Config>::MarketId>,
    }

    admin_move_market_to_closed {
        let c in 0..63;

        let range_start: MomentOf<T> = 100_000u64.saturated_into();
        let range_end: MomentOf<T> = 1_000_000u64.saturated_into();
        let (caller, market_id) = create_market_common::<T>(
            MarketCreation::Permissionless,
            MarketType::Categorical(T::MaxCategories::get()),
            ScoringRule::AmmCdaHybrid,
            Some(MarketPeriod::Timestamp(range_start..range_end)),
            Some(MarketDisputeMechanism::Court),
            None,
        )?;

        for i in 0..c {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        let close_origin = T::CloseOrigin::try_successful_origin().unwrap();
        let call = Call::<T>::admin_move_market_to_closed { market_id };
    }: { call.dispatch_bypass_filter(close_origin)? }

    admin_move_market_to_resolved_scalar_reported {
        let r in 0..63;

        let (_, market_id) = create_close_and_report_market::<T>(
            MarketCreation::Permissionless,
            MarketType::Scalar(0u128..=u128::MAX),
            OutcomeReport::Scalar(u128::MAX),
        )?;

        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;

        let report_at = market.report.unwrap().at;
        let resolves_at = report_at.saturating_add(market.deadlines.dispute_duration);
        for i in 0..r {
            MarketIdsPerReportBlock::<T>::try_mutate(
                resolves_at,
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        let resolve_origin = T::ResolveOrigin::try_successful_origin().unwrap();
        let call = Call::<T>::admin_move_market_to_resolved { market_id };
    }: {
        call.dispatch_bypass_filter(resolve_origin)?
    } verify {
        assert_last_event::<T>(Event::MarketResolved::<T>(
            market_id,
            MarketStatus::Resolved,
            OutcomeReport::Scalar(u128::MAX),
        ).into());
    }

    admin_move_market_to_resolved_categorical_reported {
        let r in 0..63;

        let categories = T::MaxCategories::get();
        let (_, market_id) = setup_reported_categorical_market::<T>(
            categories.into(),
            OutcomeReport::Categorical(0u16),
        )?;
        pallet_pm_market_commons::Pallet::<T>::mutate_market(&market_id, |market| {
            market.dispute_mechanism = Some(MarketDisputeMechanism::Authorized);
            Ok(())
        })?;

        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;

        let report_at = market.report.unwrap().at;
        let resolves_at = report_at.saturating_add(market.deadlines.dispute_duration);
        for i in 0..r {
            MarketIdsPerReportBlock::<T>::try_mutate(
                resolves_at,
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        let resolve_origin = T::ResolveOrigin::try_successful_origin().unwrap();
        let call = Call::<T>::admin_move_market_to_resolved { market_id };
    }: {
        call.dispatch_bypass_filter(resolve_origin)?
    } verify {
        assert_last_event::<T>(Event::MarketResolved::<T>(
            market_id,
            MarketStatus::Resolved,
            OutcomeReport::Categorical(0u16),
        ).into());
    }

    admin_move_market_to_resolved_scalar_disputed {
        let r in 0..63;

        let (_, market_id) = create_close_and_report_market::<T>(
            MarketCreation::Permissionless,
            MarketType::Scalar(0u128..=u128::MAX),
            OutcomeReport::Scalar(u128::MAX),
        )?;

        pallet_pm_market_commons::Pallet::<T>::mutate_market(&market_id, |market| {
            market.dispute_mechanism = Some(MarketDisputeMechanism::Authorized);
            Ok(())
        })?;

        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;

        let outcome = OutcomeReport::Scalar(0);
        let disputor = account("disputor", 1, 0);
        <T as pallet::Config>::AssetManager::deposit(
            Asset::Tru,
            &disputor,
            u128::MAX.saturated_into(),
        ).unwrap();
        Pallet::<T>::dispute(RawOrigin::Signed(disputor).into(), market_id)?;

        let now = frame_system::Pallet::<T>::block_number();
        AuthorizedPallet::<T>::authorize_market_outcome(
            T::AuthorizedDisputeResolutionOrigin::try_successful_origin().unwrap(),
            market_id.into(),
            OutcomeReport::Scalar(0),
        )?;

        let resolves_at = now.saturating_add(<T as pallet_pm_authorized::Config>::CorrectionPeriod::get());
        for i in 0..r {
            MarketIdsPerDisputeBlock::<T>::try_mutate(
                resolves_at,
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        let resolve_origin = T::ResolveOrigin::try_successful_origin().unwrap();
        let call = Call::<T>::admin_move_market_to_resolved { market_id };
    }: {
        call.dispatch_bypass_filter(resolve_origin)?
    } verify {
        assert_last_event::<T>(Event::MarketResolved::<T>(
            market_id,
            MarketStatus::Resolved,
            OutcomeReport::Scalar(0),
        ).into());
    }

    admin_move_market_to_resolved_categorical_disputed {
        let r in 0..63;

        let categories = T::MaxCategories::get();
        let (caller, market_id) =
            setup_reported_categorical_market::<T>(
                categories.into(),
                OutcomeReport::Categorical(2)
            )?;

        pallet_pm_market_commons::Pallet::<T>::mutate_market(&market_id, |market| {
            market.dispute_mechanism = Some(MarketDisputeMechanism::Authorized);
            Ok(())
        })?;

        let disputor = account("disputor", 1, 0);
        <T as pallet::Config>::AssetManager::deposit(
            Asset::Tru,
            &disputor,
            u128::MAX.saturated_into(),
        ).unwrap();
        Pallet::<T>::dispute(RawOrigin::Signed(disputor).into(), market_id)?;

        // Authorize the outcome with the highest number of correct reporters to maximize the
        // number of transfers required (0 has (d+1)//2 reports, 1 has d//2 reports).
        AuthorizedPallet::<T>::authorize_market_outcome(
            T::AuthorizedDisputeResolutionOrigin::try_successful_origin().unwrap(),
            market_id.into(),
            OutcomeReport::Categorical(0),
        )?;

        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
        let now = frame_system::Pallet::<T>::block_number();
        let resolves_at = now.saturating_add(<T as pallet_pm_authorized::Config>::CorrectionPeriod::get());
        for i in 0..r {
            MarketIdsPerDisputeBlock::<T>::try_mutate(
                resolves_at,
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        let resolve_origin = T::ResolveOrigin::try_successful_origin().unwrap();
        let call = Call::<T>::admin_move_market_to_resolved { market_id };
    }: {
        call.dispatch_bypass_filter(resolve_origin)?
    } verify {
        assert_last_event::<T>(Event::MarketResolved::<T>(
            market_id,
            MarketStatus::Resolved,
            OutcomeReport::Categorical(0u16),
        ).into());
    }

    approve_market {
        let (_, market_id) = create_market_common::<T>(
            MarketCreation::Advised,
            MarketType::Categorical(T::MaxCategories::get()),
            ScoringRule::AmmCdaHybrid,
            None,
            Some(MarketDisputeMechanism::Court),
            None,
        )?;

        let approve_origin = T::ApproveOrigin::try_successful_origin().unwrap();
        let call = Call::<T>::approve_market { market_id };
    }: { call.dispatch_bypass_filter(approve_origin)? }

    request_edit {
        let r in 0..<T as Config>::MaxEditReasonLen::get();
        let (_, market_id) = create_market_common::<T>(
            MarketCreation::Advised,
            MarketType::Categorical(T::MaxCategories::get()),
            ScoringRule::AmmCdaHybrid,
            None,
            Some(MarketDisputeMechanism::Court),
            None,
        )?;

        let request_edit_origin = T::RequestEditOrigin::try_successful_origin().unwrap();
        let edit_reason = vec![0_u8; r as usize];
        let call = Call::<T>::request_edit{ market_id, edit_reason };
    }: { call.dispatch_bypass_filter(request_edit_origin)? } verify {}

    buy_complete_set {
        let a in (T::MinCategories::get().into())..T::MaxCategories::get().into();
        let (caller, market_id) = create_market_common::<T>(
            MarketCreation::Permissionless,
            MarketType::Categorical(a.saturated_into()),
            ScoringRule::AmmCdaHybrid,
            None,
            Some(MarketDisputeMechanism::Court),
            None,
        )?;
        let amount = BASE * 1_000;
    }: _(RawOrigin::Signed(caller), market_id, amount.saturated_into())

    // Beware! We're only benchmarking categorical markets (scalar market creation is essentially
    // the same).
    create_market {
        let m in 0..63;

        let (caller, oracle, deadlines, metadata) = create_market_common_parameters::<T>(true, None)?;
        WhitelistedMarketCreators::<T>::insert(&caller, ());

        let range_end = 200_000;
        let period = MarketPeriod::Timestamp(100_000..range_end);

        for i in 0..m {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }
    }: _(
            RawOrigin::Signed(caller),
            Asset::Tru,
            Perbill::zero(),
            oracle,
            period,
            deadlines,
            metadata,
            MarketCreation::Permissionless,
            MarketType::Categorical(T::MaxCategories::get()),
            Some(MarketDisputeMechanism::Court),
            ScoringRule::AmmCdaHybrid
    )

    edit_market {
        let m in 0..63;

        let market_type = MarketType::Categorical(T::MaxCategories::get());
        let dispute_mechanism = Some(MarketDisputeMechanism::Court);
        let scoring_rule = ScoringRule::AmmCdaHybrid;
        let range_start: MomentOf<T> = 100_000u64.saturated_into();
        let range_end: MomentOf<T> = 1_000_000u64.saturated_into();
        let period = MarketPeriod::Timestamp(range_start..range_end);
        let (caller, oracle, deadlines, metadata) =
            create_market_common_parameters::<T>(true, None)?;
        WhitelistedMarketCreators::<T>::insert(&caller, ());
        Call::<T>::create_market {
            base_asset: Asset::Tru,
            creator_fee: Perbill::zero(),
            oracle: oracle.clone(),
            period: period.clone(),
            deadlines,
            metadata: metadata.clone(),
            creation: MarketCreation::Advised,
            market_type: market_type.clone(),
            dispute_mechanism: dispute_mechanism.clone(),
            scoring_rule,
        }
        .dispatch_bypass_filter(RawOrigin::Signed(caller.clone()).into())?;
        let market_id = pallet_pm_market_commons::Pallet::<T>::latest_market_id()?;

        let request_edit_origin = T::RequestEditOrigin::try_successful_origin().unwrap();
        let edit_reason = vec![0_u8; 1024];
        Call::<T>::request_edit{ market_id, edit_reason }
        .dispatch_bypass_filter(request_edit_origin)?;

        for i in 0..m {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }
        let new_deadlines = Deadlines::<BlockNumberFor<T>> {
            grace_period: 2_u32.into(),
            oracle_duration: T::MinOracleDuration::get(),
            dispute_duration: T::MinDisputeDuration::get(),
        };
    }: _(
            RawOrigin::Signed(caller),
            Asset::Tru,
            market_id,
            oracle,
            period,
            new_deadlines,
            metadata,
            market_type,
            dispute_mechanism,
            scoring_rule
    )

    start_global_dispute {
        let m in 1..CacheSize::get();
        let n in 1..CacheSize::get();

        // no benchmarking component for max disputes here,
        // because MaxDisputes is enforced for the extrinsic
        let (caller, market_id) = create_close_and_report_market::<T>(
            MarketCreation::Permissionless,
            MarketType::Scalar(0u128..=u128::MAX),
            OutcomeReport::Scalar(u128::MAX),
        )?;

        pallet_pm_market_commons::Pallet::<T>::mutate_market(&market_id, |market| {
            market.dispute_mechanism = Some(MarketDisputeMechanism::Court);
            Ok(())
        })?;

        // first element is the market id from above
        let mut market_ids_1: BoundedVec<MarketIdOf<T>, CacheSize> = Default::default();
        assert_eq!(market_id, 0u128.saturated_into());
        for i in 1..m {
            market_ids_1.try_push(i.saturated_into()).unwrap();
        }

        pallet_pm_court::Pallet::<T>::on_initialize(1u32.into());
        frame_system::Pallet::<T>::set_block_number(1u32.into());

        let min_amount = <T as pallet_pm_court::Config>::MinJurorStake::get();
        for i in 0..pallet_pm_court::Pallet::<T>::necessary_draws_weight(0usize) {
            let juror: T::AccountId = account("Jurori", i.try_into().unwrap(), 0);
            <T as pallet::Config>::AssetManager::deposit(
                Asset::Tru,
                &juror,
                (u128::MAX / 2).saturated_into(),
            ).unwrap();
            pallet_pm_court::Pallet::<T>::join_court(
                RawOrigin::Signed(juror.clone()).into(),
                min_amount + i.saturated_into(),
            )?;
        }

        let disputor: T::AccountId = account("Disputor", 1, 0);
        <T as pallet::Config>::AssetManager::deposit(
            Asset::Tru,
            &disputor,
            u128::MAX.saturated_into(),
        ).unwrap();
        let _ = Call::<T>::dispute {
            market_id,
        }
        .dispatch_bypass_filter(RawOrigin::Signed(disputor).into())?;

        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id.saturated_into()).unwrap();
        let appeal_end = T::Court::get_auto_resolve(&market_id, &market).result.unwrap();
        let mut market_ids_2: BoundedVec<MarketIdOf<T>, CacheSize> = BoundedVec::try_from(
            vec![market_id],
        ).unwrap();
        for i in 1..n {
            market_ids_2.try_push(i.saturated_into()).unwrap();
        }
        MarketIdsPerDisputeBlock::<T>::insert(appeal_end, market_ids_2);

        frame_system::Pallet::<T>::set_block_number(appeal_end - 1u64.saturated_into::<BlockNumberFor<T>>());

        let now = frame_system::Pallet::<T>::block_number();

        let add_outcome_end = now +
            <T as Config>::GlobalDisputes::get_add_outcome_period();
        let vote_end = add_outcome_end + <T as Config>::GlobalDisputes::get_vote_period();
        // the complexity depends on MarketIdsPerDisputeBlock at the current block
        // this is because a variable number of market ids need to be decoded from the storage
        MarketIdsPerDisputeBlock::<T>::insert(vote_end, market_ids_1);

        let call = Call::<T>::start_global_dispute { market_id };
    }: {
        call.dispatch_bypass_filter(RawOrigin::Signed(caller).into())?;
    }

    dispute_authorized {
        let report_outcome = OutcomeReport::Scalar(u128::MAX);
        let (caller, market_id) = create_close_and_report_market::<T>(
            MarketCreation::Permissionless,
            MarketType::Scalar(0u128..=u128::MAX),
            report_outcome,
        )?;

        pallet_pm_market_commons::Pallet::<T>::mutate_market(&market_id, |market| {
            market.dispute_mechanism = Some(MarketDisputeMechanism::Authorized);
            Ok(())
        })?;

        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;

        let call = Call::<T>::dispute { market_id };
    }: {
        call.dispatch_bypass_filter(RawOrigin::Signed(caller).into())?;
    }

    handle_expired_advised_market {
        let (_, market_id) = create_market_common::<T>(
            MarketCreation::Advised,
            MarketType::Categorical(T::MaxCategories::get()),
            ScoringRule::AmmCdaHybrid,
            Some(MarketPeriod::Timestamp(100_000..200_000)),
            Some(MarketDisputeMechanism::Court),
            None,
        )?;
        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id.saturated_into())?;
    }: { Pallet::<T>::handle_expired_advised_market(&market_id, market)? }

    internal_resolve_categorical_reported {
        let categories = T::MaxCategories::get();
        let (_, market_id) = setup_reported_categorical_market::<T>(
            categories.into(),
            OutcomeReport::Categorical(1u16),
        )?;
        pallet_pm_market_commons::Pallet::<T>::mutate_market(&market_id, |market| {
            market.dispute_mechanism = Some(MarketDisputeMechanism::Authorized);
            Ok(())
        })?;
        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
    }: {
        Pallet::<T>::on_resolution(&market_id, &market)?;
    } verify {
        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
        assert_eq!(market.status, MarketStatus::Resolved);
    }

    internal_resolve_categorical_disputed {
        let categories = T::MaxCategories::get();
        let (caller, market_id) =
            setup_reported_categorical_market::<T>(
                categories.into(),
                OutcomeReport::Categorical(1u16)
            )?;
        pallet_pm_market_commons::Pallet::<T>::mutate_market(&market_id, |market| {
            market.dispute_mechanism = Some(MarketDisputeMechanism::Authorized);
            Ok(())
        })?;

        Pallet::<T>::dispute(
            RawOrigin::Signed(caller).into(),
            market_id,
        )?;

        AuthorizedPallet::<T>::authorize_market_outcome(
            T::AuthorizedDisputeResolutionOrigin::try_successful_origin().unwrap(),
            market_id.into(),
            OutcomeReport::Categorical(0),
        )?;
        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
    }: {
        Pallet::<T>::on_resolution(&market_id, &market)?;
    } verify {
        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
        assert_eq!(market.status, MarketStatus::Resolved);
    }

    internal_resolve_scalar_reported {
        let (caller, market_id) = create_close_and_report_market::<T>(
            MarketCreation::Permissionless,
            MarketType::Scalar(0u128..=u128::MAX),
            OutcomeReport::Scalar(u128::MAX),
        )?;
        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
    }: {
        Pallet::<T>::on_resolution(&market_id, &market)?;
    } verify {
        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
        assert_eq!(market.status, MarketStatus::Resolved);
    }

    internal_resolve_scalar_disputed {
        let (caller, market_id) = create_close_and_report_market::<T>(
            MarketCreation::Permissionless,
            MarketType::Scalar(0u128..=u128::MAX),
            OutcomeReport::Scalar(u128::MAX),
        )?;
        pallet_pm_market_commons::Pallet::<T>::mutate_market(&market_id, |market| {
            market.dispute_mechanism = Some(MarketDisputeMechanism::Authorized);
            Ok(())
        })?;
        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
        Pallet::<T>::dispute(
            RawOrigin::Signed(caller).into(),
            market_id,
        )?;

        AuthorizedPallet::<T>::authorize_market_outcome(
            T::AuthorizedDisputeResolutionOrigin::try_successful_origin().unwrap(),
            market_id.into(),
            OutcomeReport::Scalar(0),
        )?;
        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
    }: {
        Pallet::<T>::on_resolution(&market_id, &market)?;
    } verify {
        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
        assert_eq!(market.status, MarketStatus::Resolved);
    }

    on_initialize_resolve_overhead {
        // wait for timestamp to get initialized (that's why block 2)
        let now = 2u64.saturated_into::<BlockNumberFor<T>>();
    }: { Pallet::<T>::on_initialize(now) }

    redeem_shares_categorical {
        let (caller, market_id) = setup_redeem_shares_common::<T>(
            MarketType::Categorical(T::MaxCategories::get()), &None
        )?;
    }: redeem_shares(RawOrigin::Signed(caller), market_id)

    redeem_shares_scalar {
        let (caller, market_id) = setup_redeem_shares_common::<T>(
            MarketType::Scalar(0u128..=u128::MAX), &None
        )?;
    }: redeem_shares(RawOrigin::Signed(caller), market_id)

    reject_market {
        let c in 0..63;
        let r in 0..<T as Config>::MaxRejectReasonLen::get();

        let range_start: MomentOf<T> = 100_000u64.saturated_into();
        let range_end: MomentOf<T> = 1_000_000u64.saturated_into();
        let (_, market_id) = create_market_common::<T>(
            MarketCreation::Advised,
            MarketType::Categorical(T::MaxCategories::get()),
            ScoringRule::AmmCdaHybrid,
            Some(MarketPeriod::Timestamp(range_start..range_end)),
            Some(MarketDisputeMechanism::Court),
            None,
        )?;

        for i in 0..c {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        let reject_origin = T::RejectOrigin::try_successful_origin().unwrap();
        let reject_reason: Vec<u8> = vec![0; r as usize];
        let call = Call::<T>::reject_market { market_id, reject_reason };
    }: { call.dispatch_bypass_filter(reject_origin)? }

    report_market_with_dispute_mechanism {
        let m in 0..63;
        let outcome = OutcomeReport::Categorical(0);
        let caller: T::AccountId = whitelisted_caller();
        let market_id = do_report_market_with_dispute_mechanism::<T>(m, &None, Some(MarketDisputeMechanism::Court), false)?;
        let call = Call::<T>::report { market_id, outcome };
    }: {
        call.dispatch_bypass_filter(RawOrigin::Signed(caller).into())?;
    }

    report_trusted_market {
        let market_id = do_report_trusted_market::<T>(&None)?;
        let outcome = OutcomeReport::Categorical(0);
        let call = Call::<T>::report { market_id, outcome };
        let caller: T::AccountId = whitelisted_caller();
    }: {
        call.dispatch_bypass_filter(RawOrigin::Signed(caller).into())?;
    }

    sell_complete_set {
        let a in (T::MinCategories::get().into())..T::MaxCategories::get().into();
        let (caller, market_id) = create_market_common::<T>(
            MarketCreation::Permissionless,
            MarketType::Categorical(a.saturated_into()),
            ScoringRule::AmmCdaHybrid,
            None,
            Some(MarketDisputeMechanism::Court),
            None,
        )?;
        let amount: BalanceOf<T> = LIQUIDITY.saturated_into();
        Pallet::<T>::buy_complete_set(
            RawOrigin::Signed(caller.clone()).into(),
            market_id,
            amount,
        )?;
    }: _(RawOrigin::Signed(caller), market_id, amount)

    // Benchmarks `market_status_manager` for any type of cache by using `MarketIdsPerClose*` as
    // sample. If `MarketIdsPerClose*` ever gets removed and we want to keep using
    // `market_status_manager`, we need to benchmark it with a different cache.
    market_status_manager {
        let b in 1..31;
        let f in 1..31;

        // ensure markets exist
        let start_block: BlockNumberFor<T> = 100_000u64.saturated_into();
        let end_block: BlockNumberFor<T> = 1_000_000u64.saturated_into();
        for _ in 0..31 {
            create_market_common::<T>(
                MarketCreation::Permissionless,
                MarketType::Categorical(T::MaxCategories::get()),
                ScoringRule::AmmCdaHybrid,
                Some(MarketPeriod::Block(start_block..end_block)),
                Some(MarketDisputeMechanism::Court),
                None,
            ).unwrap();
        }

        let range_start: MomentOf<T> = 100_000u64.saturated_into();
        let range_end: MomentOf<T> = 1_000_000u64.saturated_into();
        for _ in 31..64 {
            create_market_common::<T>(
                MarketCreation::Permissionless,
                MarketType::Categorical(T::MaxCategories::get()),
                ScoringRule::AmmCdaHybrid,
                Some(MarketPeriod::Timestamp(range_start..range_end)),
                Some(MarketDisputeMechanism::Court),
                None,
            ).unwrap();
        }

        let block_number: BlockNumberFor<T> = Zero::zero();
        for i in 1..=b {
            MarketIdsPerCloseBlock::<T>::try_mutate(block_number, |ids| {
                ids.try_push(i.into())
            }).unwrap();
        }

        let last_time_frame: TimeFrame = Zero::zero();
        let last_offset: TimeFrame = last_time_frame + 1.saturated_into::<u64>();
        //* quadratic complexity should not be allowed in substrate blockchains
        //* assume at first that the last time frame is one block before the current time frame
        let t = 0;
        let current_time_frame: TimeFrame = last_offset + t.saturated_into::<u64>();
        for i in 1..=f {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(current_time_frame, |ids| {
                // + 31 to not conflict with the markets of MarketIdsPerCloseBlock
                ids.try_push((i + 31).into())
            }).unwrap();
        }
    }: {
        Pallet::<T>::market_status_manager::<
            _,
            MarketIdsPerCloseBlock<T>,
            MarketIdsPerCloseTimeFrame<T>,
        >(
            block_number,
            last_time_frame,
            current_time_frame,
            // noop, because weight is already measured somewhere else
            |market_id, market| Ok(()),
        )
        .unwrap();
    }

    market_resolution_manager {
        let r in 1..31;
        let d in 1..31;

        let range_start: MomentOf<T> = 100_000u64.saturated_into();
        let range_end: MomentOf<T> = 1_000_000u64.saturated_into();
        // ensure markets exist
        for _ in 0..64 {
            let (_, market_id) = create_market_common::<T>(
                MarketCreation::Permissionless,
                MarketType::Categorical(T::MaxCategories::get()),
                ScoringRule::AmmCdaHybrid,
                Some(MarketPeriod::Timestamp(range_start..range_end)),
                Some(MarketDisputeMechanism::Court),
                None,
            )?;
            // ensure market is reported
            pallet_pm_market_commons::Pallet::<T>::mutate_market(&market_id, |market| {
                market.status = MarketStatus::Reported;
                Ok(())
            })?;
        }

        let block_number: BlockNumberFor<T> = Zero::zero();

        let mut r_ids_vec = Vec::new();
        for i in 1..=r {
           r_ids_vec.push(i.into());
        }
        MarketIdsPerReportBlock::<T>::mutate(block_number, |ids| {
            *ids = BoundedVec::try_from(r_ids_vec).unwrap();
        });

        // + 31 to not conflict with the markets of MarketIdsPerReportBlock
        let d_ids_vec = (1..=d).map(|i| (i + 31).into()).collect::<Vec<_>>();
        MarketIdsPerDisputeBlock::<T>::mutate(block_number, |ids| {
            *ids = BoundedVec::try_from(d_ids_vec).unwrap();
        });
    }: {
        Pallet::<T>::resolution_manager(
            block_number,
            |market_id, market| {
                // noop, because weight is already measured somewhere else
                Ok(())
            },
        ).unwrap();
    }

    schedule_early_close_as_authority {
        let o in 0..63;
        let n in 0..63;

        let range_start: MomentOf<T> = 0u64.saturated_into();
        let old_range_end: MomentOf<T> = 100_000_000_000u64.saturated_into();
        let (caller, market_id) = create_market_common::<T>(
            MarketCreation::Permissionless,
            MarketType::Categorical(T::MaxCategories::get()),
            ScoringRule::AmmCdaHybrid,
            Some(MarketPeriod::Timestamp(range_start..old_range_end)),
            Some(MarketDisputeMechanism::Court),
            None,
        )?;

        for i in 0..o {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(old_range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        let now_time = pallet_pm_market_commons::Pallet::<T>::now();
        let new_range_end: MomentOf<T> = now_time + CloseEarlyProtectionTimeFramePeriod::get();

        for i in 0..n {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(new_range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        let close_origin = T::CloseMarketEarlyOrigin::try_successful_origin().unwrap();
        let call = Call::<T>::schedule_early_close { market_id };
    }: { call.dispatch_bypass_filter(close_origin)? }

    schedule_early_close_after_dispute {
        let o in 0..63;
        let n in 0..63;

        let range_start: MomentOf<T> = 0u64.saturated_into();
        let old_range_end: MomentOf<T> = 100_000_000_000u64.saturated_into();
        let (caller, market_id) = create_market_common::<T>(
            MarketCreation::Permissionless,
            MarketType::Categorical(T::MaxCategories::get()),
            ScoringRule::AmmCdaHybrid,
            Some(MarketPeriod::Timestamp(range_start..old_range_end)),
            Some(MarketDisputeMechanism::Court),
            None,
        )?;

        for i in 0..o {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(old_range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        let now_time = pallet_pm_market_commons::Pallet::<T>::now();
        let new_range_end: MomentOf<T> = now_time + CloseEarlyProtectionTimeFramePeriod::get();

        for i in 0..n {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(new_range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        Pallet::<T>::schedule_early_close(
            RawOrigin::Signed(caller.clone()).into(),
            market_id,
        )?;

        Pallet::<T>::dispute_early_close(
            RawOrigin::Signed(caller.clone()).into(),
            market_id,
        )?;

        let close_origin = T::CloseMarketEarlyOrigin::try_successful_origin().unwrap();
        let call = Call::<T>::schedule_early_close { market_id };
    }: { call.dispatch_bypass_filter(close_origin)? }

    schedule_early_close_as_market_creator {
        let o in 0..63;
        let n in 0..63;

        let range_start: MomentOf<T> = 0u64.saturated_into();
        let old_range_end: MomentOf<T> = 100_000_000_000u64.saturated_into();
        let (caller, market_id) = create_market_common::<T>(
            MarketCreation::Permissionless,
            MarketType::Categorical(T::MaxCategories::get()),
            ScoringRule::AmmCdaHybrid,
            Some(MarketPeriod::Timestamp(range_start..old_range_end)),
            Some(MarketDisputeMechanism::Court),
            None,
        )?;

        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
        let market_creator = market.creator.clone();

        for i in 0..o {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(old_range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        let now_time = pallet_pm_market_commons::Pallet::<T>::now();
        let new_range_end: MomentOf<T> = now_time + CloseEarlyTimeFramePeriod::get();

        for i in 0..n {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(new_range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        let origin = RawOrigin::Signed(market_creator).into();
        let call = Call::<T>::schedule_early_close { market_id };
    }: { call.dispatch_bypass_filter(origin)? }

    dispute_early_close {
        let o in 0..63;
        let n in 0..63;

        let range_start: MomentOf<T> = 0u64.saturated_into();
        let old_range_end: MomentOf<T> = 100_000_000_000u64.saturated_into();
        let (caller, market_id) = create_market_common::<T>(
            MarketCreation::Permissionless,
            MarketType::Categorical(T::MaxCategories::get()),
            ScoringRule::AmmCdaHybrid,
            Some(MarketPeriod::Timestamp(range_start..old_range_end)),
            Some(MarketDisputeMechanism::Court),
            None,
        )?;

        let market_creator = caller.clone();

        Pallet::<T>::schedule_early_close(
            RawOrigin::Signed(market_creator.clone()).into(),
            market_id,
        )?;

        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
        let new_range_end = match market.period {
            MarketPeriod::Timestamp(range) => {
                range.end
            },
            _ => {
                return Err(frame_benchmarking::BenchmarkError::Stop(
                          "MarketPeriod is block_number based"
                        ));
            },
        };

        for i in 0..o {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(old_range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        for i in 0..n {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(new_range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        let origin = RawOrigin::Signed(market_creator).into();
        let call = Call::<T>::dispute_early_close { market_id };
    }: { call.dispatch_bypass_filter(origin)? }

    reject_early_close_after_authority {
        let o in 0..63;
        let n in 0..63;

        let range_start: MomentOf<T> = 0u64.saturated_into();
        let old_range_end: MomentOf<T> = 100_000_000_000u64.saturated_into();
        let (caller, market_id) = create_market_common::<T>(
            MarketCreation::Permissionless,
            MarketType::Categorical(T::MaxCategories::get()),
            ScoringRule::AmmCdaHybrid,
            Some(MarketPeriod::Timestamp(range_start..old_range_end)),
            Some(MarketDisputeMechanism::Court),
            None,
        )?;

        let market_creator = caller.clone();

        let close_origin = T::CloseMarketEarlyOrigin::try_successful_origin().unwrap();
        Pallet::<T>::schedule_early_close(
            close_origin.clone(),
            market_id,
        )?;

        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
        let new_range_end = match market.period {
            MarketPeriod::Timestamp(range) => {
                range.end
            },
            _ => {
                return Err(frame_benchmarking::BenchmarkError::Stop(
                          "MarketPeriod is block_number based"
                        ));
            },
        };

        for i in 0..o {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(old_range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        for i in 0..n {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(new_range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        let call = Call::<T>::reject_early_close { market_id };
    }: { call.dispatch_bypass_filter(close_origin)? }

    reject_early_close_after_dispute {
        let range_start: MomentOf<T> = 0u64.saturated_into();
        let old_range_end: MomentOf<T> = 100_000_000_000u64.saturated_into();
        let (caller, market_id) = create_market_common::<T>(
            MarketCreation::Permissionless,
            MarketType::Categorical(T::MaxCategories::get()),
            ScoringRule::AmmCdaHybrid,
            Some(MarketPeriod::Timestamp(range_start..old_range_end)),
            Some(MarketDisputeMechanism::Court),
            None,
        )?;

        let market_creator = caller.clone();

        Pallet::<T>::schedule_early_close(
            RawOrigin::Signed(market_creator.clone()).into(),
            market_id,
        )?;

        Pallet::<T>::dispute_early_close(
            RawOrigin::Signed(caller.clone()).into(),
            market_id,
        )?;

        let close_origin = T::CloseMarketEarlyOrigin::try_successful_origin().unwrap();

        let call = Call::<T>::reject_early_close { market_id };
    }: { call.dispatch_bypass_filter(close_origin)? }

    close_trusted_market {
        let c in 0..63;

        let range_start: MomentOf<T> = 100_000u64.saturated_into();
        let range_end: MomentOf<T> = 1_000_000u64.saturated_into();
        let (caller, market_id) = create_market_common::<T>(
            MarketCreation::Permissionless,
            MarketType::Categorical(T::MaxCategories::get()),
            ScoringRule::AmmCdaHybrid,
            Some(MarketPeriod::Timestamp(range_start..range_end)),
            None,
            None,
        )?;

        for i in 0..c {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        let call = Call::<T>::close_trusted_market { market_id };
    }: { call.dispatch_bypass_filter(RawOrigin::Signed(caller).into())? }

    create_market_and_deploy_pool {
        // Beware! This benchmark expects the `DeployPool` implementation to accept spot prices as
        // low as `BASE / MaxCategories::get()`!
        let m in 0..63; // Number of markets closing on the same block.
        let n in 2..T::MaxCategories::get() as u32; // Number of assets in the market.

        let base_asset = Asset::Tru;
        let range_start = (5 * MILLISECS_PER_BLOCK) as u64;
        let range_end = (100 * MILLISECS_PER_BLOCK) as u64;
        let period = MarketPeriod::Timestamp(range_start..range_end);
        let asset_count = n.try_into().unwrap();
        let market_type = MarketType::Categorical(asset_count);
        let (caller, oracle, deadlines, metadata) = create_market_common_parameters::<T>(true, None)?;
        let amount = (10u128 * BASE).saturated_into();

        <T as pallet::Config>::AssetManager::deposit(
            base_asset,
            &caller,
            amount,
        )?;
        WhitelistedMarketCreators::<T>::insert(&caller, ());
        for i in 0..m {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }
    }: _(
            RawOrigin::Signed(caller),
            base_asset,
            Perbill::zero(),
            oracle,
            period,
            deadlines,
            metadata,
            MarketType::Categorical(asset_count),
            Some(MarketDisputeMechanism::Court),
            amount,
            create_spot_prices::<T>(asset_count),
            CENT_BASE.saturated_into()
    )

    signed_create_market_and_deploy_pool {
        // Beware! This benchmark expects the `DeployPool` implementation to accept spot prices as
        // low as `BASE / MaxCategories::get()`!
        let m in 0..63; // Number of markets closing on the same block.
        let n in 2..T::MaxCategories::get() as u32; // Number of assets in the market.

        let relayer_account_id = get_relayer::<T>();
        let (caller_key_pair, caller_account_id) = get_user_account::<T>();

        let base_asset = Asset::Tru;
        let range_start = (5 * MILLISECS_PER_BLOCK) as u64;
        let range_end = (100 * MILLISECS_PER_BLOCK) as u64;
        let period = MarketPeriod::Timestamp(range_start..range_end);
        let asset_count = n.try_into().unwrap();
        let market_type = MarketType::Categorical(asset_count);
        let (caller, oracle, deadlines, metadata) = create_market_common_parameters::<T>(true, Some(caller_account_id.clone()))?;
        let amount = (10u128 * BASE).saturated_into();

        <T as pallet::Config>::AssetManager::deposit(
            base_asset,
            &caller,
            amount,
        )?;
        WhitelistedMarketCreators::<T>::insert(&caller, ());
        for i in 0..m {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(
                Pallet::<T>::calculate_time_frame_of_moment(range_end),
                |ids| ids.try_push(i.into()),
            ).unwrap();
        }

        let spot_prices = create_spot_prices::<T>(asset_count);
        let swap_fee: BalanceOf<T> = CENT_BASE.saturated_into();
        let dispute_resolution = MarketDisputeMechanism::Court;
        let creator_fee = Perbill::zero();
        let signed_payload = (
            CREATE_MARKET_AND_DEPLOY_POOL_CONTEXT,
            relayer_account_id.clone(),
            0u64,
            base_asset,
            creator_fee.clone(),
            oracle.clone(),
            period.clone(),
            deadlines,
            metadata.clone(),
            market_type.clone(),
            Some(MarketDisputeMechanism::Court),
            amount,
            spot_prices.clone(),
            swap_fee.clone(),
        );

        let signature = caller_key_pair.sign(&signed_payload.encode().as_slice()).unwrap().encode();
        let proof: Proof<T::Signature, T::AccountId> = get_proof::<T>(caller_account_id.clone(), relayer_account_id, &signature);
    }: _(
            RawOrigin::Signed(caller),
            proof,
            base_asset,
            creator_fee,
            oracle,
            period,
            deadlines,
            metadata,
            market_type,
            Some(dispute_resolution),
            amount,
            spot_prices,
            swap_fee
    )

    manually_close_market {
        let o in 1..63;

        let range_start: MomentOf<T> = 100_000u64.saturated_into();
        let range_end: MomentOf<T> = 1_000_000u64.saturated_into();

        let (caller, market_id) = create_market_common::<T>(
            MarketCreation::Permissionless,
            MarketType::Categorical(T::MaxCategories::get()),
            ScoringRule::AmmCdaHybrid,
            Some(MarketPeriod::Timestamp(range_start..range_end)),
            Some(MarketDisputeMechanism::Court),
            None,
        )?;

        let now = 1_500_000u32;
        assert!(range_end < now as u64);

        let range_end_time_frame = Pallet::<T>::calculate_time_frame_of_moment(range_end);
        let range_end_time_frame = Pallet::<T>::calculate_time_frame_of_moment(range_end);
        for i in 1..o {
            MarketIdsPerCloseTimeFrame::<T>::try_mutate(range_end_time_frame, |ids| {
                ids.try_push((i + 1).into())
            }).unwrap();
        }

        pallet_pm_market_commons::Pallet::<T>::mutate_market(&market_id, |market| {
            market.status = MarketStatus::Active;
            Ok(())
        })?;

        pallet_timestamp::Pallet::<T>::set_timestamp(now.into());
    }: manually_close_market(RawOrigin::Signed(caller), market_id)

    set_config_option {
        let market_admin: T::AccountId = whitelisted_caller();
    }: _(RawOrigin::Root, AdminConfig::MarketAdmin(market_admin.clone()))
    verify {
        assert_eq!(MarketAdmin::<T>::get(), Some(market_admin.clone()));
        assert_last_event::<T>(
            Event::MarketAdminSet { new_admin: market_admin }
        .into());
    }

    whitelist_market_creator {
        let market_admin: T::AccountId = whitelisted_caller();
        MarketAdmin::<T>::set(Some(market_admin.clone()));
        let whitelisted_account: T::AccountId = account("WhitelistedAcc", 0, 0);
    }: _(RawOrigin::Signed(market_admin), whitelisted_account.clone())
    verify {
        assert!(WhitelistedMarketCreators::<T>::contains_key(&whitelisted_account));
        assert_last_event::<T>(
            Event::MarketCreatorAdded { whitelisted_account }
        .into());
    }

    set_winnings_fee_account {
        let market_admin: T::AccountId = whitelisted_caller();
        MarketAdmin::<T>::set(Some(market_admin.clone()));
        let whitelisted_account: T::AccountId = account("WhitelistedAcc", 0, 0);
    }: _(RawOrigin::Signed(market_admin), whitelisted_account.clone())
    verify {
        assert_eq!(WinningsFeeAccount::<T>::get(), Some(whitelisted_account.clone()));
        assert_last_event::<T>(
            Event::WinningsFeeAccountSet { new_account: whitelisted_account }
        .into());
    }

    set_additional_swap_fee_account {
        let market_admin: T::AccountId = whitelisted_caller();
        MarketAdmin::<T>::set(Some(market_admin.clone()));
        let whitelisted_account: T::AccountId = account("WhitelistedAcc", 0, 0);
    }: _(RawOrigin::Signed(market_admin), whitelisted_account.clone())
    verify {
        assert_eq!(AdditionalSwapFeeAccount::<T>::get(), Some(whitelisted_account.clone()));
        assert_last_event::<T>(
            Event::AdditionalSwapFeeAccountSet { new_account: whitelisted_account }
        .into());
    }

    remove_market_creator {
        let market_admin: T::AccountId = whitelisted_caller();
        MarketAdmin::<T>::set(Some(market_admin.clone()));
        let removed_account: T::AccountId = account("WhitelistedAccToRemove", 0, 0);
        WhitelistedMarketCreators::<T>::insert(removed_account.clone(), ());
    }: _(RawOrigin::Signed(market_admin), removed_account.clone())
    verify {
        assert!(!WhitelistedMarketCreators::<T>::contains_key(&removed_account));
        assert_last_event::<T>(
            Event::MarketCreatorRemoved::<T> { removed_account }
        .into());
    }

    signed_transfer_asset {
        let relayer_account_id = get_relayer::<T>();
        let (caller_key_pair, caller_account_id) = get_user_account::<T>();

        let token: EthAddress = H160::from([1u8; 20]);
        let asset = T::AssetRegistry::asset_id(&token).unwrap();
        let asset_metadata: AssetMetadata<
        BalanceOf<T>,
        CustomMetadata,
        <T as pallet_pm_eth_asset_registry::Config>::StringLimit> = AssetMetadata {
            decimals: 18,
            name: BoundedVec::truncate_from("dummy token".as_bytes().to_vec()),
            symbol: BoundedVec::truncate_from("DMY".as_bytes().to_vec()),
            existential_deposit: 0u32.into(),
            location: None,
            additional: CustomMetadata { eth_address: token, allow_as_base_asset: true },
        };

        T::AssetManager::deposit(asset, &caller_account_id, (10000u128 * LIQUIDITY).saturated_into()).unwrap();
        let recipient: T::AccountId = account("Recipient", 0, 0);
        let amount: BalanceOf<T> = (1000u128 * LIQUIDITY).saturated_into();

        let signed_payload = (
            TRANSFER_TOKENS_CONTEXT,
            &relayer_account_id,
            0u64,
            token,
            caller_account_id.clone(),
            recipient.clone(),
            amount
        );

        let signature = caller_key_pair.sign(&signed_payload.encode().as_slice()).unwrap().encode();

        let proof: Proof<T::Signature, T::AccountId> = get_proof::<T>(caller_account_id.clone(), relayer_account_id, &signature);
    }: _(RawOrigin::Signed(caller_account_id.clone()), proof, token, recipient.clone(), amount)
    verify {
        let recipient_balance = T::AssetManager::free_balance(asset, &recipient);
        assert_eq!(recipient_balance, amount);
    }

    signed_withdraw_tokens {
        let relayer_account_id = get_relayer::<T>();
        let (caller_key_pair, caller_account_id) = get_user_account::<T>();

        let token: EthAddress = H160::from([1u8; 20]);
        let asset = T::AssetRegistry::asset_id(&token).unwrap();
        let asset_metadata: AssetMetadata<
        BalanceOf<T>,
        CustomMetadata,
        <T as pallet_pm_eth_asset_registry::Config>::StringLimit> = AssetMetadata {
            decimals: 18,
            name: BoundedVec::truncate_from("dummy token".as_bytes().to_vec()),
            symbol: BoundedVec::truncate_from("DMY".as_bytes().to_vec()),
            existential_deposit: 0u32.into(),
            location: None,
            additional: CustomMetadata { eth_address: token, allow_as_base_asset: true },
        };
        let initial_balance: BalanceOf<T> = (10000u128 * LIQUIDITY).saturated_into();
        T::AssetManager::deposit(asset, &caller_account_id, initial_balance).unwrap();
        let amount: BalanceOf<T> = (1000u128 * LIQUIDITY).saturated_into();

        let signed_payload =
            (WITHDRAW_TOKENS_CONTEXT, relayer_account_id.clone(), 0u64, token, caller_account_id.clone(), amount);
        let signature = caller_key_pair.sign(&signed_payload.encode().as_slice()).unwrap().encode();

        let proof: Proof<T::Signature, T::AccountId> = get_proof::<T>(caller_account_id.clone(), relayer_account_id, &signature);
    }: _(RawOrigin::Signed(caller_account_id.clone()), proof, token, amount)
    verify {
        let owner_balance = T::AssetManager::free_balance(asset, &caller_account_id);
        assert_eq!(owner_balance, initial_balance - amount);
    }

    //signed_report
    signed_report_market_with_dispute_mechanism {
        let m in 0..63;

        let relayer_account_id = get_relayer::<T>();
        let (caller_key_pair, caller_account_id) = get_user_account::<T>();

        let outcome = OutcomeReport::Categorical(0);
        let market_id = do_report_market_with_dispute_mechanism::<T>(m, &Some(caller_account_id.clone()), Some(MarketDisputeMechanism::Court), false)?;

        let signed_payload =
            (REPORT_OUTCOME_CONTEXT, relayer_account_id.clone(), 0u64, market_id, outcome.clone());
        let signature = caller_key_pair.sign(&signed_payload.encode().as_slice()).unwrap().encode();

        let proof: Proof<T::Signature, T::AccountId> = get_proof::<T>(caller_account_id.clone(), relayer_account_id, &signature);
        let call = Call::<T>::signed_report { proof, market_id, outcome };
    }: {
        call.dispatch_bypass_filter(RawOrigin::Signed(caller_account_id).into())?;
    }

    signed_report_trusted_market {
        let relayer_account_id = get_relayer::<T>();
        let (caller_key_pair, caller_account_id) = get_user_account::<T>();

        let market_id = do_report_trusted_market::<T>(&Some(caller_account_id.clone()))?;
        let outcome = OutcomeReport::Categorical(0);

        let signed_payload =
            (REPORT_OUTCOME_CONTEXT, relayer_account_id.clone(), 0u64, market_id, outcome.clone());
        let market_nonce = MarketNonces::<T>::get(caller_account_id.clone(), market_id);
        let signature = caller_key_pair.sign(&signed_payload.encode().as_slice()).unwrap().encode();
        let proof: Proof<T::Signature, T::AccountId> = get_proof::<T>(caller_account_id.clone(), relayer_account_id, &signature);
        let call = Call::<T>::signed_report { proof, market_id, outcome };
    }: {
        call.dispatch_bypass_filter(RawOrigin::Signed(caller_account_id.clone()).into())?;
    }
    verify {
        let new_nonce = MarketNonces::<T>::get(caller_account_id.clone(), market_id);
        assert_eq!(new_nonce, market_nonce + 1);
    }

    signed_redeem_shares_categorical {
        let relayer_account_id = get_relayer::<T>();
        let (caller_key_pair, caller_account_id) = get_user_account::<T>();
        let (caller, market_id) = setup_redeem_shares_common::<T>(
            MarketType::Categorical(T::MaxCategories::get()), &Some(caller_account_id.clone())
        )?;
        let market_nonce = MarketNonces::<T>::get(caller_account_id.clone(), market_id);
        let signed_payload =
            (REDEEM_SHARES, relayer_account_id.clone(), 0u64, market_id);

        let signature = caller_key_pair.sign(&signed_payload.encode().as_slice()).unwrap().encode();
        let proof: Proof<T::Signature, T::AccountId> = get_proof::<T>(caller_account_id.clone(), relayer_account_id, &signature);

    }: signed_redeem_shares(RawOrigin::Signed(caller.clone()), proof, market_id)
    verify {
        let new_nonce = MarketNonces::<T>::get(caller_account_id.clone(), market_id);
        assert_eq!(new_nonce, market_nonce + 1);
    }

    signed_redeem_shares_scalar {
        let relayer_account_id = get_relayer::<T>();
        let (caller_key_pair, caller_account_id) = get_user_account::<T>();
        let (caller, market_id) = setup_redeem_shares_common::<T>(
            MarketType::Scalar(0u128..=u128::MAX), &Some(caller_account_id.clone())
        )?;
        let market_nonce = MarketNonces::<T>::get(caller_account_id.clone(), market_id);
        let signed_payload =
            (REDEEM_SHARES, relayer_account_id.clone(), 0u64, market_id);

        let signature = caller_key_pair.sign(&signed_payload.encode().as_slice()).unwrap().encode();
        let proof: Proof<T::Signature, T::AccountId> = get_proof::<T>(caller_account_id.clone(), relayer_account_id, &signature);

    }: signed_redeem_shares(RawOrigin::Signed(caller.clone()), proof, market_id)
    verify {
        let new_nonce = MarketNonces::<T>::get(caller_account_id.clone(), market_id);
        assert_eq!(new_nonce, market_nonce + 1);
    }

    signed_buy_complete_set {
        let a in (T::MinCategories::get().into())..T::MaxCategories::get().into();
        let relayer_account_id = get_relayer::<T>();
        let (caller_key_pair, caller_account_id) = get_user_account::<T>();

        let (caller, market_id) = create_market_and_pool::<T>(
            &Some(caller_account_id.clone()), a
        )?;

        let market_nonce = MarketNonces::<T>::get(caller_account_id.clone(), market_id);
        let amount = (10u128 * BASE).saturated_into();
        let signed_payload =
            (BUY_COMPLETE_SET_CONTEXT, relayer_account_id.clone(), 0u64, market_id, amount);

        let signature = caller_key_pair.sign(&signed_payload.encode().as_slice()).unwrap().encode();
        let proof: Proof<T::Signature, T::AccountId> = get_proof::<T>(caller_account_id.clone(), relayer_account_id, &signature);

    }: signed_buy_complete_set(RawOrigin::Signed(caller.clone()), proof, market_id, amount)
    verify {
        let new_nonce = MarketNonces::<T>::get(caller_account_id.clone(), market_id);
        assert_eq!(new_nonce, market_nonce + 1);
    }

    admin_update_market_oracle {
        let market_admin: T::AccountId = whitelisted_caller();
        MarketAdmin::<T>::set(Some(market_admin.clone()));
        let old_oracle: T::AccountId = account("oldOracle", 0, 0);
        let new_oracle: T::AccountId = account("newOracle", 1, 1);
        let market_id = do_report_market_with_dispute_mechanism::<T>(0, &Some(old_oracle.clone()), None, true)?;
    }: admin_update_market_oracle(RawOrigin::Signed(market_admin), market_id, new_oracle.clone())
    verify {
        let market = pallet_pm_market_commons::Pallet::<T>::market(&market_id)?;
        assert_eq!(market.oracle, new_oracle);
         assert_last_event::<T>(
            Event::MarketOracleUpdated::<T> { market_id, old_oracle, new_oracle }
        .into());
    }

    impl_benchmark_test_suite!(
        PredictionMarket,
        crate::mock::ExtBuilder::default().build(),
        crate::mock::Runtime,
    );
}
