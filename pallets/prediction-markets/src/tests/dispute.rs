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

use super::*;
use test_case::test_case;

use crate::{MarketAdmin, MarketIdsPerDisputeBlock, WhitelistedMarketCreators};
use prediction_market_primitives::types::{Bond, OutcomeReport};

// TODO(#1239) fails if market doesn't exist
// TODO(#1239) fails if market is trusted
// TODO(#1239) fails if user can't afford the bond

#[test]
fn it_allows_to_dispute_the_outcome_of_a_market() {
    ExtBuilder::default().build().execute_with(|| {
        let end = 2;
        simple_create_categorical_market(
            Asset::Tru,
            MarketCreation::Permissionless,
            0..end,
            ScoringRule::AmmCdaHybrid,
        );
        let market_id = 0;

        // Run to the end of the trading phase.
        let market = MarketCommons::market(&market_id).unwrap();
        let grace_period = end + market.deadlines.grace_period;
        run_to_block(grace_period + 1);

        assert_ok!(PredictionMarkets::report(
            RuntimeOrigin::signed(bob()),
            market_id,
            OutcomeReport::Categorical(1)
        ));

        let dispute_at = grace_period + 2;
        run_to_block(dispute_at);

        assert_ok!(PredictionMarkets::dispute(RuntimeOrigin::signed(charlie()), 0));
        let market = MarketCommons::market(&market_id).unwrap();
        assert_eq!(market.status, MarketStatus::Disputed);

        // Ensure that the MDM interacts correctly with auto resolution.
        assert_ok!(Authorized::authorize_market_outcome(
            RuntimeOrigin::signed(AuthorizedDisputeResolutionUser::get()),
            market_id,
            OutcomeReport::Categorical(0),
        ));
        let dispute_ends_at =
            dispute_at + <Runtime as pallet_pm_authorized::Config>::CorrectionPeriod::get();
        let market_ids = MarketIdsPerDisputeBlock::<Runtime>::get(dispute_ends_at);
        assert_eq!(market_ids.len(), 1);
        assert_eq!(market_ids[0], 0);
    });
}

#[test]
fn dispute_fails_disputed_already() {
    ExtBuilder::default().build().execute_with(|| {
        let end = 2;
        WhitelistedMarketCreators::<Runtime>::insert(&alice(), ());
        assert_ok!(PredictionMarkets::create_market(
            RuntimeOrigin::signed(alice()),
            Asset::Tru,
            Perbill::zero(),
            bob(),
            MarketPeriod::Block(0..end),
            get_deadlines(),
            gen_metadata(2),
            MarketCreation::Permissionless,
            MarketType::Categorical(<Runtime as Config>::MinCategories::get()),
            Some(MarketDisputeMechanism::Authorized),
            ScoringRule::AmmCdaHybrid,
        ));

        // Run to the end of the trading phase.
        let market = MarketCommons::market(&0).unwrap();
        let grace_period = end + market.deadlines.grace_period;
        run_to_block(grace_period + 1);

        assert_ok!(PredictionMarkets::report(
            RuntimeOrigin::signed(bob()),
            0,
            OutcomeReport::Categorical(1)
        ));

        let dispute_at = grace_period + 2;
        run_to_block(dispute_at);

        assert_ok!(PredictionMarkets::dispute(RuntimeOrigin::signed(charlie()), 0));

        assert_noop!(
            PredictionMarkets::dispute(RuntimeOrigin::signed(charlie()), 0),
            Error::<Runtime>::InvalidMarketStatus,
        );
    });
}

#[test]
fn dispute_fails_if_market_not_reported() {
    ExtBuilder::default().build().execute_with(|| {
        let end = 2;
        WhitelistedMarketCreators::<Runtime>::insert(&alice(), ());
        assert_ok!(PredictionMarkets::create_market(
            RuntimeOrigin::signed(alice()),
            Asset::Tru,
            Perbill::zero(),
            bob(),
            MarketPeriod::Block(0..end),
            get_deadlines(),
            gen_metadata(2),
            MarketCreation::Permissionless,
            MarketType::Categorical(<Runtime as Config>::MinCategories::get()),
            Some(MarketDisputeMechanism::Authorized),
            ScoringRule::AmmCdaHybrid,
        ));

        // Run to the end of the trading phase.
        let market = MarketCommons::market(&0).unwrap();
        let grace_period = end + market.deadlines.grace_period;
        run_to_block(grace_period + 1);

        // no report happening here...

        let dispute_at = grace_period + 2;
        run_to_block(dispute_at);

        assert_noop!(
            PredictionMarkets::dispute(RuntimeOrigin::signed(charlie()), 0),
            Error::<Runtime>::InvalidMarketStatus,
        );
    });
}

#[test]
fn dispute_reserves_dispute_bond() {
    ExtBuilder::default().build().execute_with(|| {
        let end = 2;
        WhitelistedMarketCreators::<Runtime>::insert(&alice(), ());
        assert_ok!(PredictionMarkets::create_market(
            RuntimeOrigin::signed(alice()),
            Asset::Tru,
            Perbill::zero(),
            bob(),
            MarketPeriod::Block(0..end),
            get_deadlines(),
            gen_metadata(2),
            MarketCreation::Permissionless,
            MarketType::Categorical(<Runtime as Config>::MinCategories::get()),
            Some(MarketDisputeMechanism::Authorized),
            ScoringRule::AmmCdaHybrid,
        ));

        // Run to the end of the trading phase.
        let market = MarketCommons::market(&0).unwrap();
        let grace_period = end + market.deadlines.grace_period;
        run_to_block(grace_period + 1);

        assert_ok!(PredictionMarkets::report(
            RuntimeOrigin::signed(bob()),
            0,
            OutcomeReport::Categorical(1)
        ));

        let dispute_at = grace_period + 2;
        run_to_block(dispute_at);

        let free_charlie_before = Balances::free_balance(charlie());
        let reserved_charlie = Balances::reserved_balance(charlie());
        assert_eq!(reserved_charlie, 0);

        assert_ok!(PredictionMarkets::dispute(RuntimeOrigin::signed(charlie()), 0,));

        let free_charlie_after = Balances::free_balance(charlie());
        assert_eq!(free_charlie_before - free_charlie_after, DisputeBond::get());

        let reserved_charlie = Balances::reserved_balance(charlie());
        assert_eq!(reserved_charlie, DisputeBond::get());
    });
}

#[test]
fn dispute_updates_market() {
    ExtBuilder::default().build().execute_with(|| {
        let end = 2;
        WhitelistedMarketCreators::<Runtime>::insert(&alice(), ());
        assert_ok!(PredictionMarkets::create_market(
            RuntimeOrigin::signed(alice()),
            Asset::Tru,
            Perbill::zero(),
            bob(),
            MarketPeriod::Block(0..end),
            get_deadlines(),
            gen_metadata(2),
            MarketCreation::Permissionless,
            MarketType::Categorical(<Runtime as Config>::MinCategories::get()),
            Some(MarketDisputeMechanism::Authorized),
            ScoringRule::AmmCdaHybrid,
        ));

        // Run to the end of the trading phase.
        let market = MarketCommons::market(&0).unwrap();
        let grace_period = end + market.deadlines.grace_period;
        run_to_block(grace_period + 1);

        assert_ok!(PredictionMarkets::report(
            RuntimeOrigin::signed(bob()),
            0,
            OutcomeReport::Categorical(1)
        ));

        let dispute_at = grace_period + 2;
        run_to_block(dispute_at);

        let market = MarketCommons::market(&0).unwrap();
        assert_eq!(market.status, MarketStatus::Reported);
        assert_eq!(market.bonds.dispute, None);

        assert_ok!(PredictionMarkets::dispute(RuntimeOrigin::signed(charlie()), 0,));

        let market = MarketCommons::market(&0).unwrap();
        assert_eq!(market.status, MarketStatus::Disputed);
        assert_eq!(
            market.bonds.dispute,
            Some(Bond { who: charlie(), value: DisputeBond::get(), is_settled: false })
        );
    });
}

#[test]
fn dispute_emits_event() {
    ExtBuilder::default().build().execute_with(|| {
        let end = 2;
        WhitelistedMarketCreators::<Runtime>::insert(&alice(), ());
        assert_ok!(PredictionMarkets::create_market(
            RuntimeOrigin::signed(alice()),
            Asset::Tru,
            Perbill::zero(),
            bob(),
            MarketPeriod::Block(0..end),
            get_deadlines(),
            gen_metadata(2),
            MarketCreation::Permissionless,
            MarketType::Categorical(<Runtime as Config>::MinCategories::get()),
            Some(MarketDisputeMechanism::Authorized),
            ScoringRule::AmmCdaHybrid,
        ));

        // Run to the end of the trading phase.
        let market = MarketCommons::market(&0).unwrap();
        let grace_period = end + market.deadlines.grace_period;
        run_to_block(grace_period + 1);

        assert_ok!(PredictionMarkets::report(
            RuntimeOrigin::signed(bob()),
            0,
            OutcomeReport::Categorical(1)
        ));

        let dispute_at = grace_period + 2;
        run_to_block(dispute_at);

        assert_ok!(PredictionMarkets::dispute(RuntimeOrigin::signed(charlie()), 0,));

        System::assert_last_event(
            Event::MarketDisputed(0u32.into(), MarketStatus::Disputed, charlie()).into(),
        );
    });
}

#[test_case(MarketStatus::Active; "active")]
#[test_case(MarketStatus::Closed; "closed")]
#[test_case(MarketStatus::Proposed; "proposed")]
#[test_case(MarketStatus::Resolved; "resolved")]
fn dispute_fails_unless_reported_or_disputed_market(status: MarketStatus) {
    ExtBuilder::default().build().execute_with(|| {
        // Creates a permissionless market.
        simple_create_categorical_market(
            Asset::Tru,
            MarketCreation::Permissionless,
            0..2,
            ScoringRule::AmmCdaHybrid,
        );

        assert_ok!(MarketCommons::mutate_market(&0, |market_inner| {
            market_inner.status = status;
            Ok(())
        }));

        assert_noop!(
            PredictionMarkets::dispute(RuntimeOrigin::signed(eve()), 0),
            Error::<Runtime>::InvalidMarketStatus
        );
    });
}

mod updating_oracle {
    use super::*;

    #[test]
    fn succeeds() {
        ExtBuilder::default().build().execute_with(|| {
            let end = 2;
            WhitelistedMarketCreators::<Runtime>::insert(&alice(), ());
            assert_ok!(PredictionMarkets::create_market(
                RuntimeOrigin::signed(alice()),
                Asset::Tru,
                Perbill::zero(),
                bob(),
                MarketPeriod::Block(0..end),
                Deadlines {
                    grace_period: 1_u32.into(),
                    oracle_duration: <Runtime as Config>::MinOracleDuration::get(),
                    dispute_duration: 0_u32.into(),
                },
                gen_metadata(2),
                MarketCreation::Permissionless,
                MarketType::Categorical(<Runtime as Config>::MinCategories::get()),
                None,
                ScoringRule::AmmCdaHybrid,
            ));

            // Run to the end of the trading phase.
            let market = MarketCommons::market(&0).unwrap();
            let grace_period = end + market.deadlines.grace_period;

            run_to_block(grace_period + market.deadlines.oracle_duration + 1);

            let market_admin = <MarketAdmin<Runtime>>::get().unwrap();
            let new_oracle = charlie();
            assert_ok!(PredictionMarkets::admin_update_market_oracle(
                RuntimeOrigin::signed(market_admin),
                market.market_id,
                new_oracle
            ));
            System::assert_last_event(
                Event::MarketOracleUpdated {
                    market_id: market.market_id,
                    old_oracle: bob(),
                    new_oracle: charlie(),
                }
                .into(),
            );
        });
    }

    mod fails_when {
        use super::*;

        #[test]
        fn bad_sender() {
            ExtBuilder::default().build().execute_with(|| {
                let end = 2;
                WhitelistedMarketCreators::<Runtime>::insert(&alice(), ());
                assert_ok!(PredictionMarkets::create_market(
                    RuntimeOrigin::signed(alice()),
                    Asset::Tru,
                    Perbill::zero(),
                    bob(),
                    MarketPeriod::Block(0..end),
                    Deadlines {
                        grace_period: 1_u32.into(),
                        oracle_duration: <Runtime as Config>::MinOracleDuration::get(),
                        dispute_duration: 0_u32.into(),
                    },
                    gen_metadata(2),
                    MarketCreation::Permissionless,
                    MarketType::Categorical(<Runtime as Config>::MinCategories::get()),
                    None,
                    ScoringRule::AmmCdaHybrid,
                ));

                // Run to the end of the trading phase.
                let market = MarketCommons::market(&0).unwrap();
                let grace_period = end + market.deadlines.grace_period;
                run_to_block(grace_period + market.deadlines.oracle_duration + 1);

                let new_oracle = charlie();
                assert_noop!(
                    PredictionMarkets::admin_update_market_oracle(
                        RuntimeOrigin::signed(new_oracle),
                        market.market_id,
                        new_oracle
                    ),
                    Error::<Runtime>::SenderNotMarketAdmin
                );
            });
        }

        #[test]
        fn market_is_not_closed() {
            ExtBuilder::default().build().execute_with(|| {
                let end = 2;
                WhitelistedMarketCreators::<Runtime>::insert(&alice(), ());
                assert_ok!(PredictionMarkets::create_market(
                    RuntimeOrigin::signed(alice()),
                    Asset::Tru,
                    Perbill::zero(),
                    bob(),
                    MarketPeriod::Block(0..end),
                    Deadlines {
                        grace_period: 1_u32.into(),
                        oracle_duration: <Runtime as Config>::MinOracleDuration::get(),
                        dispute_duration: 0_u32.into(),
                    },
                    gen_metadata(2),
                    MarketCreation::Permissionless,
                    MarketType::Categorical(<Runtime as Config>::MinCategories::get()),
                    None,
                    ScoringRule::AmmCdaHybrid,
                ));

                let market = MarketCommons::market(&0).unwrap();
                let market_admin = <MarketAdmin<Runtime>>::get().unwrap();
                let new_oracle = charlie();
                assert_noop!(
                    PredictionMarkets::admin_update_market_oracle(
                        RuntimeOrigin::signed(market_admin),
                        market.market_id,
                        new_oracle
                    ),
                    Error::<Runtime>::MarketIsNotClosed
                );
            });
        }

        #[test]
        fn market_can_be_disputed() {
            ExtBuilder::default().build().execute_with(|| {
                let end = 2;
                WhitelistedMarketCreators::<Runtime>::insert(&alice(), ());
                assert_ok!(PredictionMarkets::create_market(
                    RuntimeOrigin::signed(alice()),
                    Asset::Tru,
                    Perbill::zero(),
                    bob(),
                    MarketPeriod::Block(0..end),
                    Deadlines {
                        grace_period: 1_u32.into(),
                        oracle_duration: <Runtime as Config>::MinOracleDuration::get(),
                        dispute_duration: <Runtime as Config>::MinDisputeDuration::get(),
                    },
                    gen_metadata(2),
                    MarketCreation::Permissionless,
                    MarketType::Categorical(<Runtime as Config>::MinCategories::get()),
                    Some(MarketDisputeMechanism::Authorized),
                    ScoringRule::AmmCdaHybrid,
                ));

                let market = MarketCommons::market(&0).unwrap();
                let grace_period = end + market.deadlines.grace_period;
                run_to_block(grace_period + market.deadlines.oracle_duration + 1);

                let market_admin = <MarketAdmin<Runtime>>::get().unwrap();
                let new_oracle = charlie();
                assert_noop!(
                    PredictionMarkets::admin_update_market_oracle(
                        RuntimeOrigin::signed(market_admin),
                        market.market_id,
                        new_oracle
                    ),
                    Error::<Runtime>::MarketCanBeDisputed
                );
            });
        }

        #[test]
        fn oracle_grace_period_not_ended() {
            ExtBuilder::default().build().execute_with(|| {
                let end = 2;
                WhitelistedMarketCreators::<Runtime>::insert(&alice(), ());
                assert_ok!(PredictionMarkets::create_market(
                    RuntimeOrigin::signed(alice()),
                    Asset::Tru,
                    Perbill::zero(),
                    bob(),
                    MarketPeriod::Block(0..end),
                    Deadlines {
                        grace_period: 1_u32.into(),
                        oracle_duration: <Runtime as Config>::MinOracleDuration::get(),
                        dispute_duration: 0_u32.into(),
                    },
                    gen_metadata(2),
                    MarketCreation::Permissionless,
                    MarketType::Categorical(<Runtime as Config>::MinCategories::get()),
                    None,
                    ScoringRule::AmmCdaHybrid,
                ));

                let market = MarketCommons::market(&0).unwrap();
                let grace_period = end + market.deadlines.grace_period;
                // Make sure we leave room for the oracle to report.
                run_to_block(grace_period - 1);

                let market_admin = <MarketAdmin<Runtime>>::get().unwrap();
                let new_oracle = charlie();
                assert_noop!(
                    PredictionMarkets::admin_update_market_oracle(
                        RuntimeOrigin::signed(market_admin),
                        market.market_id,
                        new_oracle
                    ),
                    Error::<Runtime>::NotAllowedToReportYet
                );
            });
        }

        #[test]
        fn reporting_window_not_expired() {
            ExtBuilder::default().build().execute_with(|| {
                let end = 2;
                WhitelistedMarketCreators::<Runtime>::insert(&alice(), ());
                assert_ok!(PredictionMarkets::create_market(
                    RuntimeOrigin::signed(alice()),
                    Asset::Tru,
                    Perbill::zero(),
                    bob(),
                    MarketPeriod::Block(0..end),
                    Deadlines {
                        grace_period: 1_u32.into(),
                        oracle_duration: <Runtime as Config>::MinOracleDuration::get(),
                        dispute_duration: 0_u32.into(),
                    },
                    gen_metadata(2),
                    MarketCreation::Permissionless,
                    MarketType::Categorical(<Runtime as Config>::MinCategories::get()),
                    None,
                    ScoringRule::AmmCdaHybrid,
                ));

                let market = MarketCommons::market(&0).unwrap();
                let grace_period = end + market.deadlines.grace_period;
                // Make sure we leave room for the oracle to report.
                run_to_block(grace_period + market.deadlines.oracle_duration - 1);

                let market_admin = <MarketAdmin<Runtime>>::get().unwrap();
                let new_oracle = charlie();
                assert_noop!(
                    PredictionMarkets::admin_update_market_oracle(
                        RuntimeOrigin::signed(market_admin),
                        market.market_id,
                        new_oracle
                    ),
                    Error::<Runtime>::OracleReportingWindowNotExpired
                );
            });
        }

        #[test]
        fn market_already_reported() {
            ExtBuilder::default().build().execute_with(|| {
                let end = 2;
                WhitelistedMarketCreators::<Runtime>::insert(&alice(), ());
                assert_ok!(PredictionMarkets::create_market(
                    RuntimeOrigin::signed(alice()),
                    Asset::Tru,
                    Perbill::zero(),
                    bob(),
                    MarketPeriod::Block(0..end),
                    get_deadlines(),
                    gen_metadata(2),
                    MarketCreation::Permissionless,
                    MarketType::Categorical(<Runtime as Config>::MinCategories::get()),
                    Some(MarketDisputeMechanism::Authorized),
                    ScoringRule::AmmCdaHybrid,
                ));

                // Run to the end of the trading phase.
                let market = MarketCommons::market(&0).unwrap();
                let grace_period = end + market.deadlines.grace_period;
                run_to_block(grace_period + 1);

                assert_ok!(PredictionMarkets::report(
                    RuntimeOrigin::signed(bob()),
                    0,
                    OutcomeReport::Categorical(1)
                ));

                let market_admin = <MarketAdmin<Runtime>>::get().unwrap();
                let new_oracle = charlie();
                assert_noop!(
                    PredictionMarkets::admin_update_market_oracle(
                        RuntimeOrigin::signed(market_admin),
                        market.market_id,
                        new_oracle
                    ),
                    Error::<Runtime>::MarketAlreadyReported
                );
            });
        }
    }
}
