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

#![cfg(all(feature = "mock", test))]

mod admin_move_market_to_closed;
mod admin_move_market_to_resolved;
mod approve_market;
mod buy_complete_set;
mod close_trusted_market;
mod create_market;
mod create_market_and_deploy_pool;
mod dispute;
mod dispute_early_close;
mod edit_market;
mod integration;
mod manually_close_market;
mod on_initialize;
mod on_market_close;
mod on_resolution;
mod pallet_admin_tests;
mod redeem_shares;
mod reject_early_close;
mod reject_market;
mod report;
mod request_edit;
mod schedule_early_close;
mod sell_complete_set;
mod start_global_dispute;

use crate::{
    mock::*, AccountIdOf, AssetOf, BalanceOf, Config, DeadlinesOf, Error, Event,
    MarketIdsPerDisputeBlock, WhitelistedMarketCreators,
};
use core::ops::Range;
use frame_support::{assert_noop, assert_ok, traits::NamedReservableCurrency};
use orml_traits::MultiCurrency;
use pallet_pm_court::types::VoteItem;
use pallet_pm_market_commons::MarketCommonsPalletApi;
use prediction_market_primitives::{
    constants::mock::{BASE, CENT_BASE},
    types::{
        Asset, Deadlines, MarketCreation, MarketDisputeMechanism, MarketId, MarketPeriod,
        MarketStatus, MarketType, MultiHash, OutcomeReport, ScoringRule,
    },
};
use sp_arithmetic::Perbill;
use sp_runtime::traits::{BlakeTwo256, Hash, Zero};

const SENTINEL_AMOUNT: u128 = BASE;

fn get_deadlines() -> DeadlinesOf<Runtime> {
    Deadlines {
        grace_period: 1_u32.into(),
        oracle_duration: <Runtime as Config>::MinOracleDuration::get(),
        dispute_duration: <Runtime as Config>::MinDisputeDuration::get(),
    }
}

fn gen_metadata(byte: u8) -> MultiHash {
    let mut metadata = [byte; 50];
    metadata[0] = 0x15;
    metadata[1] = 0x30;
    MultiHash::Sha3_384(metadata)
}

fn simple_create_categorical_market(
    base_asset: AssetOf<Runtime>,
    creation: MarketCreation,
    period: Range<u32>,
    scoring_rule: ScoringRule,
) {
    WhitelistedMarketCreators::<Runtime>::insert(&alice(), ());
    assert_ok!(PredictionMarkets::create_market(
        RuntimeOrigin::signed(alice()),
        base_asset,
        Perbill::zero(),
        bob(),
        MarketPeriod::Block(period),
        get_deadlines(),
        gen_metadata(2),
        creation,
        MarketType::Categorical(<Runtime as Config>::MinCategories::get()),
        Some(MarketDisputeMechanism::Authorized),
        scoring_rule
    ));
}

fn simple_create_scalar_market(
    base_asset: AssetOf<Runtime>,
    creation: MarketCreation,
    period: Range<u32>,
    scoring_rule: ScoringRule,
) {
    WhitelistedMarketCreators::<Runtime>::insert(&alice(), ());
    assert_ok!(PredictionMarkets::create_market(
        RuntimeOrigin::signed(alice()),
        base_asset,
        Perbill::zero(),
        bob(),
        MarketPeriod::Block(period),
        get_deadlines(),
        gen_metadata(2),
        creation,
        MarketType::Scalar(100..=200),
        Some(MarketDisputeMechanism::Court),
        scoring_rule
    ));
}

fn check_reserve(account: &AccountIdOf<Runtime>, expected: BalanceOf<Runtime>) {
    assert_eq!(Balances::reserved_balance(account), SENTINEL_AMOUNT + expected);
}

fn reserve_sentinel_amounts() {
    // Reserve a sentinel amount to check that we don't unreserve too much.
    assert_ok!(Balances::reserve_named(
        &PredictionMarkets::reserve_id(),
        &alice(),
        SENTINEL_AMOUNT
    ));
    assert_ok!(Balances::reserve_named(&PredictionMarkets::reserve_id(), &bob(), SENTINEL_AMOUNT));
    assert_ok!(Balances::reserve_named(
        &PredictionMarkets::reserve_id(),
        &charlie(),
        SENTINEL_AMOUNT
    ));
    assert_ok!(Balances::reserve_named(&PredictionMarkets::reserve_id(), &dave(), SENTINEL_AMOUNT));
    assert_ok!(Balances::reserve_named(&PredictionMarkets::reserve_id(), &eve(), SENTINEL_AMOUNT));
    assert_ok!(Balances::reserve_named(&PredictionMarkets::reserve_id(), &fred(), SENTINEL_AMOUNT));
    assert_eq!(Balances::reserved_balance(alice()), SENTINEL_AMOUNT);
    assert_eq!(Balances::reserved_balance(bob()), SENTINEL_AMOUNT);
    assert_eq!(Balances::reserved_balance(charlie()), SENTINEL_AMOUNT);
    assert_eq!(Balances::reserved_balance(dave()), SENTINEL_AMOUNT);
    assert_eq!(Balances::reserved_balance(eve()), SENTINEL_AMOUNT);
    assert_eq!(Balances::reserved_balance(fred()), SENTINEL_AMOUNT);
}

fn simulate_appeal_cycle(market_id: MarketId) {
    let court = pallet_pm_court::Courts::<Runtime>::get(market_id).unwrap();
    let vote_start = court.round_ends.pre_vote + 1;

    run_to_block(vote_start);

    let salt = <Runtime as frame_system::Config>::Hash::default();

    let wrong_outcome = OutcomeReport::Categorical(1);
    let wrong_vote_item = VoteItem::Outcome(wrong_outcome);

    let draws = pallet_pm_court::SelectedDraws::<Runtime>::get(market_id);
    for draw in &draws {
        let commitment =
            BlakeTwo256::hash_of(&(draw.court_participant, wrong_vote_item.clone(), salt));
        assert_ok!(Court::vote(
            RuntimeOrigin::signed(draw.court_participant),
            market_id,
            commitment
        ));
    }

    let aggregation_start = court.round_ends.vote + 1;
    run_to_block(aggregation_start);

    for draw in draws {
        assert_ok!(Court::reveal_vote(
            RuntimeOrigin::signed(draw.court_participant),
            market_id,
            wrong_vote_item.clone(),
            salt,
        ));
    }

    let resolve_at = court.round_ends.appeal;
    let market_ids = MarketIdsPerDisputeBlock::<Runtime>::get(resolve_at);
    assert_eq!(market_ids.len(), 1);

    run_to_block(resolve_at - 1);

    let market_after = MarketCommons::market(&0).unwrap();
    assert_eq!(market_after.status, MarketStatus::Disputed);
}
