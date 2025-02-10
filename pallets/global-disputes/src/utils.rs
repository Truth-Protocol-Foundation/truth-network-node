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

#![cfg(any(feature = "runtime-benchmarks", test))]

use crate::*;
use frame_system::pallet_prelude::BlockNumberFor;

type MarketOf<T> = prediction_market_primitives::types::Market<
    <T as frame_system::Config>::AccountId,
    BalanceOf<T>,
    BlockNumberFor<T>,
    MomentOf<T>,
    MarketIdOf<T>,
>;

pub(crate) fn market_mock<T>() -> MarketOf<T>
where
    T: crate::Config,
{
    use frame_support::traits::Get;
    use prediction_market_primitives::types::ScoringRule;
    use sp_runtime::traits::AccountIdConversion;

    prediction_market_primitives::types::Market {
        market_id: Default::default(),
        base_asset: prediction_market_primitives::types::Asset::Tru,
        creation: prediction_market_primitives::types::MarketCreation::Permissionless,
        creator_fee: sp_runtime::Perbill::zero(),
        creator: T::GlobalDisputesPalletId::get().into_account_truncating(),
        market_type: prediction_market_primitives::types::MarketType::Scalar(0..=u128::MAX),
        dispute_mechanism: Some(
            prediction_market_primitives::types::MarketDisputeMechanism::Authorized,
        ),
        metadata: Default::default(),
        oracle: T::GlobalDisputesPalletId::get().into_account_truncating(),
        period: prediction_market_primitives::types::MarketPeriod::Block(Default::default()),
        deadlines: prediction_market_primitives::types::Deadlines {
            grace_period: 1_u32.into(),
            oracle_duration: 1_u32.into(),
            dispute_duration: 1_u32.into(),
        },
        report: None,
        resolved_outcome: None,
        scoring_rule: ScoringRule::AmmCdaHybrid,
        status: prediction_market_primitives::types::MarketStatus::Disputed,
        bonds: Default::default(),
        early_close: None,
    }
}
