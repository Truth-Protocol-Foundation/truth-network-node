// Copyright 2024 Forecasting Technologies LTD.
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
use prediction_market_primitives::types::Asset;

#[test]
fn buy_from_amm_and_then_fill_specified_order() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let pivot = _1_100;
        let spot_prices = vec![_1_2 - pivot, _1_2 + pivot];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount_in = _2;

        let order_maker_amount = _12;
        let order_taker_amount = _6;
        assert_ok!(AssetManager::deposit(asset, &charlie(), order_maker_amount));
        assert_ok!(Orderbook::place_order(
            RuntimeOrigin::signed(charlie()),
            market_id,
            asset,
            order_maker_amount,
            BASE_ASSET,
            order_taker_amount,
        ));

        let order_ids = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();

        let max_price = _3_4.saturated_into::<BalanceOf<Runtime>>();
        let strategy = Strategy::LimitOrder;
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount_in,
            max_price,
            order_ids,
            strategy,
        ));

        let amm_amount_in = 2804328542;
        System::assert_has_event(
            NeoSwapsEvent::<Runtime>::BuyExecuted {
                who: alice(),
                market_id,
                asset_out: asset,
                amount_in: amm_amount_in,
                amount_out: 5606655193,
                swap_fee_amount: 28043285,
                external_fee_amount: 1000000,
            }
            .into(),
        );

        let order_ids = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(order_ids.len(), 1);
        let order = Orders::<Runtime>::get(order_ids[0]).unwrap();
        let unfilled_base_asset_amount = 42804328542;
        assert_eq!(
            order,
            Order {
                market_id,
                maker: charlie(),
                maker_asset: Asset::CategoricalOutcome(market_id, 0),
                maker_amount: 85608657084,
                taker_asset: BASE_ASSET,
                taker_amount: unfilled_base_asset_amount,
            }
        );
        let filled_base_asset_amount = order_taker_amount - unfilled_base_asset_amount;
        assert_eq!(filled_base_asset_amount, 17195671458);
        assert_eq!(amm_amount_in + filled_base_asset_amount, amount_in);
    });
}

#[test]
fn buy_from_amm_if_specified_order_has_higher_prices_than_the_amm() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let spot_prices = vec![_1_4, _3_4];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = _2;

        let order_maker_amount = _4;
        let order_taker_amount = _2;
        assert_ok!(AssetManager::deposit(asset, &charlie(), order_maker_amount));
        assert_ok!(Orderbook::place_order(
            RuntimeOrigin::signed(charlie()),
            market_id,
            asset,
            order_maker_amount,
            BASE_ASSET,
            order_taker_amount,
        ));

        let order_ids = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();

        let max_price = _3_4.saturated_into::<BalanceOf<Runtime>>();
        let strategy = Strategy::LimitOrder;
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount,
            max_price,
            order_ids,
            strategy,
        ));

        let order_ids = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(order_ids.len(), 1);
        let order = Orders::<Runtime>::get(order_ids[0]).unwrap();
        assert_eq!(
            order,
            Order {
                market_id,
                maker: charlie(),
                maker_asset: Asset::CategoricalOutcome(market_id, 0),
                maker_amount: _4,
                taker_asset: BASE_ASSET,
                taker_amount: _2,
            }
        );
    });
}

#[test]
fn buy_fill_multiple_orders_if_amm_spot_price_higher_than_order_prices() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let spot_prices = vec![_1_2, _1_2];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount_in = _2;

        let order_maker_amount = _1;
        let order_taker_amount = _1_2;
        assert_ok!(AssetManager::deposit(asset, &charlie(), 2 * order_maker_amount));
        assert_ok!(Orderbook::place_order(
            RuntimeOrigin::signed(charlie()),
            market_id,
            asset,
            order_maker_amount,
            BASE_ASSET,
            order_taker_amount,
        ));
        assert_ok!(Orderbook::place_order(
            RuntimeOrigin::signed(charlie()),
            market_id,
            asset,
            order_maker_amount,
            BASE_ASSET,
            order_taker_amount,
        ));

        let order_ids = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();

        let max_price = _3_4.saturated_into::<BalanceOf<Runtime>>();
        let strategy = Strategy::LimitOrder;
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount_in,
            max_price,
            order_ids,
            strategy,
        ));

        let order_ids = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(order_ids.len(), 0);
    });
}

#[test]
fn buy_fill_specified_order_partially_if_amm_spot_price_higher() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let spot_prices = vec![_1_2, _1_2];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = _2;

        let order_maker_amount = _8;
        let order_taker_amount = _4;
        assert_ok!(AssetManager::deposit(asset, &charlie(), order_maker_amount));
        assert_ok!(Orderbook::place_order(
            RuntimeOrigin::signed(charlie()),
            market_id,
            asset,
            order_maker_amount,
            BASE_ASSET,
            order_taker_amount,
        ));

        let order_ids = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(order_ids.len(), 1);
        let order_id = order_ids[0];

        let max_price = _3_4.saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![order_id];
        let strategy = Strategy::LimitOrder;
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount,
            max_price,
            orders,
            strategy,
        ));

        let order = Orders::<Runtime>::get(order_id).unwrap();
        assert_eq!(
            order,
            Order {
                market_id,
                maker: charlie(),
                maker_asset: Asset::CategoricalOutcome(market_id, 0),
                maker_amount: _4,
                taker_asset: BASE_ASSET,
                taker_amount: _2,
            }
        );
    });
}

#[test]
fn buy_fails_if_asset_not_equal_to_order_book_maker_asset() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let spot_prices = vec![_1_2, _1_2];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = _2;
        let order_maker_amount = _1;
        assert_ok!(AssetManager::deposit(BASE_ASSET, &charlie(), order_maker_amount));

        assert_ok!(Orderbook::place_order(
            RuntimeOrigin::signed(charlie()),
            market_id,
            BASE_ASSET,
            order_maker_amount,
            Asset::CategoricalOutcome(market_id, 0),
            _2,
        ));

        let order_ids = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(order_ids.len(), 1);
        let order_id = order_ids[0];

        let max_price = _3_4.saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![order_id];
        let strategy = Strategy::LimitOrder;
        assert_noop!(
            HybridRouter::buy(
                RuntimeOrigin::signed(alice()),
                market_id,
                asset_count,
                asset,
                amount,
                max_price,
                orders,
                strategy,
            ),
            Error::<Runtime>::AssetNotEqualToOrderbookMakerAsset
        );
    });
}

#[test]
fn buy_fails_if_order_price_above_max_price() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let spot_prices = vec![_1_2, _1_2];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = _2;

        let order_maker_amount = _1;
        assert_ok!(AssetManager::deposit(asset, &charlie(), order_maker_amount));
        assert_ok!(Orderbook::place_order(
            RuntimeOrigin::signed(charlie()),
            market_id,
            Asset::CategoricalOutcome(market_id, 0),
            order_maker_amount,
            BASE_ASSET,
            _2,
        ));

        let order_ids = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(order_ids.len(), 1);
        let order_id = order_ids[0];

        let max_price = _3_4.saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![order_id];
        let strategy = Strategy::LimitOrder;
        assert_noop!(
            HybridRouter::buy(
                RuntimeOrigin::signed(alice()),
                market_id,
                asset_count,
                asset,
                amount,
                max_price,
                orders,
                strategy,
            ),
            Error::<Runtime>::OrderPriceAboveMaxPrice
        );
    });
}

#[test]
fn buy_from_amm() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let spot_prices = vec![_1_2, _1_2];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = _2;
        let max_price = _3_4.saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![];
        let strategy = Strategy::LimitOrder;
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount,
            max_price,
            orders,
            strategy,
        ));

        System::assert_has_event(
            NeoSwapsEvent::<Runtime>::BuyExecuted {
                who: alice(),
                market_id,
                asset_out: asset,
                amount_in: 20000000000,
                amount_out: 37205851586,
                swap_fee_amount: 200000000,
                external_fee_amount: 1000000,
            }
            .into(),
        );
    });
}

#[test]
fn buy_max_price_lower_than_amm_spot_price_results_in_place_order() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let spot_prices = vec![_1_2 + 1u128, _1_2 - 1u128];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );
        let market = Markets::<Runtime>::get(market_id).unwrap();
        let base_asset = market.base_asset;

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = _2;
        //*  max_price is just 1 smaller than the spot price of the AMM
        //*  this results in no buy on the AMM, but places an order on the order book
        let max_price = (_1_2).saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![];
        let strategy = Strategy::LimitOrder;
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount,
            max_price,
            orders,
            strategy,
        ));

        let order_keys = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(order_keys.len(), 1);
        let order_id = order_keys[0];
        let order = Orders::<Runtime>::get(order_id).unwrap();
        assert_eq!(
            order,
            Order {
                market_id,
                maker: alice(),
                maker_asset: base_asset,
                maker_amount: _2,
                taker_asset: asset,
                taker_amount: _4,
            }
        );
    });
}

#[test]
fn buy_from_amm_but_low_amount() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let spot_prices = vec![_1_2, _1_2];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );
        let market = Markets::<Runtime>::get(market_id).unwrap();
        let base_asset = market.base_asset;

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount_in = _2;
        //*  max_price is just 1 larger than the spot price of the AMM
        //*  this results in a low buy amount_in on the AMM
        let max_price = (_1_2 + 1u128).saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![];
        let strategy = Strategy::LimitOrder;
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount_in,
            max_price,
            orders,
            strategy,
        ));

        System::assert_has_event(
            NeoSwapsEvent::<Runtime>::BuyExecuted {
                who: alice(),
                market_id,
                asset_out: asset,
                amount_in: 29,
                amount_out: 58,
                swap_fee_amount: 0,
                external_fee_amount: 0,
            }
            .into(),
        );

        let order_keys = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(order_keys.len(), 1);
        let order_id = order_keys[0];
        let order = Orders::<Runtime>::get(order_id).unwrap();
        assert_eq!(
            order,
            Order {
                market_id,
                maker: alice(),
                maker_asset: base_asset,
                maker_amount: 19999999971,
                taker_asset: asset,
                taker_amount: 39999999935,
            }
        );
    });
}

#[test]
fn buy_from_amm_only() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let spot_prices = vec![_1_2, _1_2];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = _2;
        let max_price = _3_4.saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![];
        let strategy = Strategy::ImmediateOrCancel;
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount,
            max_price,
            orders,
            strategy,
        ));

        System::assert_has_event(
            NeoSwapsEvent::<Runtime>::BuyExecuted {
                who: alice(),
                market_id,
                asset_out: asset,
                amount_in: 20000000000,
                amount_out: 37205851586,
                swap_fee_amount: 200000000,
                external_fee_amount: 1000000,
            }
            .into(),
        );

        let order_keys = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(order_keys.len(), 0);
    });
}

#[test]
fn buy_places_limit_order_no_pool() {
    ExtBuilder::default().build().execute_with(|| {
        let market_id = 0;
        let mut market = market_mock::<Runtime>(market_creator());
        let base_asset = market.base_asset;
        let required_asset_count = match &market.market_type {
            MarketType::Scalar(_) => panic!("Categorical market type is expected!"),
            MarketType::Categorical(categories) => *categories,
        };
        market.status = MarketStatus::Active;
        Markets::<Runtime>::insert(market_id, market);

        let asset_count = required_asset_count;
        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = 10 * BASE;
        let max_price = (BASE / 2).saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![];
        let strategy = Strategy::LimitOrder;
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount,
            max_price,
            orders,
            strategy,
        ));

        let order_keys = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(order_keys.len(), 1);
        let order_id = order_keys[0];
        let order = Orders::<Runtime>::get(order_id).unwrap();
        assert_eq!(
            order,
            Order {
                market_id,
                maker: alice(),
                maker_asset: base_asset,
                maker_amount: 10 * BASE,
                taker_asset: asset,
                taker_amount: 20 * BASE,
            }
        );
    });
}

#[test]
fn buy_fails_if_balance_too_low() {
    ExtBuilder::default().build().execute_with(|| {
        let market_id = 0;
        let mut market = market_mock::<Runtime>(market_creator());
        let required_asset_count = match &market.market_type {
            MarketType::Scalar(_) => panic!("Categorical market type is expected!"),
            MarketType::Categorical(categories) => *categories,
        };
        market.status = MarketStatus::Active;
        Markets::<Runtime>::insert(market_id, market);

        let asset_count = required_asset_count;
        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = 10 * BASE;

        assert_eq!(Balances::set_balance(&alice(), amount - 1), amount - 1);
        let max_price = (BASE / 2).saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![];
        let strategy = Strategy::LimitOrder;
        assert_noop!(
            HybridRouter::buy(
                RuntimeOrigin::signed(alice()),
                market_id,
                asset_count,
                asset,
                amount,
                max_price,
                orders,
                strategy,
            ),
            CurrenciesError::<Runtime>::BalanceTooLow
        );
    });
}

// This test is failing because the decimal is set to 18 instead of 10
// Keep this failing until we resolve this issue
#[test]
fn buy_emits_event() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let pivot = _1_100;
        let spot_prices = vec![_1_2 + pivot, _1_2 - pivot];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount_in = _1000 * 100;

        assert_ok!(AssetManager::deposit(BASE_ASSET, &alice(), amount_in));

        let max_price = _9_10.saturated_into::<BalanceOf<Runtime>>();
        let orders = (0u128..10u128).collect::<Vec<_>>();
        let maker_asset = asset;
        let maker_amount: BalanceOf<Runtime> = _20.saturated_into();
        let taker_asset = BASE_ASSET;
        let taker_amount = _11.saturated_into::<BalanceOf<Runtime>>();
        for (i, _) in orders.iter().enumerate() {
            // Do not use get_account_with_seed because it changes the account addresses
            let order_creator = get_account(i.try_into().unwrap());
            let surplus = ((i + 1) as u128) * _1_2;
            let taker_amount = taker_amount + surplus.saturated_into::<BalanceOf<Runtime>>();
            assert_ok!(AssetManager::deposit(maker_asset, &order_creator, maker_amount));
            assert_ok!(Orderbook::place_order(
                RuntimeOrigin::signed(order_creator),
                market_id,
                maker_asset,
                maker_amount,
                taker_asset,
                taker_amount,
            ));
        }

        let strategy = Strategy::LimitOrder;
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount_in,
            max_price,
            orders,
            strategy,
        ));

        System::assert_last_event(
            Event::<Runtime>::HybridRouterExecuted {
                tx_type: TxType::Buy,
                who: alice(),
                market_id,
                price_limit: max_price,
                asset_in: BASE_ASSET,
                amount_in,
                asset_out: asset,
                amount_out: 2302415689824,
                external_fee_amount: 12000000,
                swap_fee_amount: 2250551794,
            }
            .into(),
        );
    });
}

#[test]
fn buy_fails_if_asset_count_mismatch() {
    ExtBuilder::default().build().execute_with(|| {
        let market_id = 0;
        let mut market = market_mock::<Runtime>(market_creator());
        let required_asset_count = match &market.market_type {
            MarketType::Scalar(_) => panic!("Categorical market type is expected!"),
            MarketType::Categorical(categories) => *categories,
        };
        market.status = MarketStatus::Active;
        Markets::<Runtime>::insert(market_id, market);

        let asset_count = 2;
        assert_ne!(required_asset_count, asset_count);
        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = 2 * BASE;
        let max_price = (BASE / 2).saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![];
        let strategy = Strategy::ImmediateOrCancel;
        assert_noop!(
            HybridRouter::buy(
                RuntimeOrigin::signed(alice()),
                market_id,
                asset_count,
                asset,
                amount,
                max_price,
                orders,
                strategy,
            ),
            Error::<Runtime>::AssetCountMismatch
        );
    });
}

#[test]
fn buy_fails_if_price_limit_too_high() {
    ExtBuilder::default().build().execute_with(|| {
        let market_id = 0;
        let mut market = market_mock::<Runtime>(market_creator());
        let required_asset_count = match &market.market_type {
            MarketType::Scalar(_) => panic!("Categorical market type is expected!"),
            MarketType::Categorical(categories) => *categories,
        };
        market.status = MarketStatus::Active;
        Markets::<Runtime>::insert(market_id, market);

        let asset_count = required_asset_count;
        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = 10 * BASE;
        let max_price = (BASE + 1).saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![];
        let strategy = Strategy::LimitOrder;
        assert_noop!(
            HybridRouter::buy(
                RuntimeOrigin::signed(alice()),
                market_id,
                asset_count,
                asset,
                amount,
                max_price,
                orders,
                strategy,
            ),
            Error::<Runtime>::PriceLimitTooHigh
        );
    });
}

#[test]
fn buy_succeeds_for_place_order_below_minimum_balance_soft_failure() {
    ExtBuilder::default().build().execute_with(|| {
        let market_id = 0;
        let mut market = market_mock::<Runtime>(market_creator());
        let required_asset_count = match &market.market_type {
            MarketType::Scalar(_) => panic!("Categorical market type is expected!"),
            MarketType::Categorical(categories) => *categories,
        };
        market.status = MarketStatus::Active;
        Markets::<Runtime>::insert(market_id, market);

        let asset_count = required_asset_count;
        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = 1;
        let max_price = (BASE / 2).saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![];
        let strategy = Strategy::LimitOrder;
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount,
            max_price,
            orders,
            strategy,
        ));

        // no order was placed since the amount is below the minimum balance
        let order_keys = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(order_keys.len(), 0);
    });
}

#[test]
fn buy_succeeds_for_numerical_soft_failure() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let min_spot_price = CENT_BASE / 2;
        let spot_prices = vec![min_spot_price, _1 - min_spot_price];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = _1_100;

        let max_price = (_3_4).saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![];
        let strategy = Strategy::LimitOrder;
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount,
            max_price,
            orders,
            strategy,
        ));

        let order = Orders::<Runtime>::get(0).unwrap();
        assert_eq!(
            order,
            Order {
                market_id,
                maker: alice(),
                maker_asset: BASE_ASSET,
                maker_amount: _1_100,
                taker_asset: Asset::CategoricalOutcome(market_id, 0),
                taker_amount: 133333334,
            }
        );
    });
}

#[test]
fn buy_just_one_unit_from_amm() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let spot_prices = vec![_1_2, _1_2];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = 1;

        let order_maker_amount = _2;
        assert_ok!(AssetManager::deposit(asset, &charlie(), order_maker_amount));
        assert_ok!(Orderbook::place_order(
            RuntimeOrigin::signed(charlie()),
            market_id,
            Asset::CategoricalOutcome(market_id, 0),
            order_maker_amount,
            BASE_ASSET,
            _1,
        ));

        let order_ids = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(order_ids.len(), 1);
        let order_id = order_ids[0];

        let max_price = (_1_2 + 1).saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![order_id];
        let strategy = Strategy::LimitOrder;
        // Prevent ED error by giving Alice some extra tokens.
        assert_ok!(AssetManager::deposit(asset, &alice(), _1));
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount,
            max_price,
            orders,
            strategy,
        ));

        let order = Orders::<Runtime>::get(order_id).unwrap();
        assert_eq!(
            order,
            Order {
                market_id,
                maker: charlie(),
                maker_asset: Asset::CategoricalOutcome(market_id, 0),
                maker_amount: _2,
                taker_asset: BASE_ASSET,
                taker_amount: _1,
            }
        );
    });
}

#[test]
fn buy_succeeds_for_fill_order_below_minimum_balance_soft_failure() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let spot_prices = vec![_1_2, _1_2];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = 1;

        let order_maker_amount = _2;
        assert_ok!(AssetManager::deposit(asset, &charlie(), order_maker_amount));
        assert_ok!(Orderbook::place_order(
            RuntimeOrigin::signed(charlie()),
            market_id,
            Asset::CategoricalOutcome(market_id, 0),
            order_maker_amount,
            BASE_ASSET,
            _1,
        ));

        let order_ids = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(order_ids.len(), 1);
        let order_id = order_ids[0];

        let max_price = _3_4.saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![order_id];
        let strategy = Strategy::LimitOrder;
        // Prevent ED error by giving Alice some extra tokens.
        assert_ok!(AssetManager::deposit(asset, &alice(), _1));
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount,
            max_price,
            orders,
            strategy,
        ));

        let order = Orders::<Runtime>::get(order_id).unwrap();
        assert_eq!(
            order,
            Order {
                market_id,
                maker: charlie(),
                maker_asset: Asset::CategoricalOutcome(market_id, 0),
                maker_amount: _2,
                taker_asset: BASE_ASSET,
                taker_amount: _1,
            }
        );
    });
}

#[test]
fn buy_succeeds_for_place_order_partial_fill_near_full_fill_not_allowed() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let spot_prices = vec![_1_2, _1_2];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = _1 - 1;

        let order_maker_amount = _2;
        assert_ok!(AssetManager::deposit(asset, &charlie(), order_maker_amount));
        assert_ok!(Orderbook::place_order(
            RuntimeOrigin::signed(charlie()),
            market_id,
            Asset::CategoricalOutcome(market_id, 0),
            order_maker_amount,
            BASE_ASSET,
            _1,
        ));

        let order_ids = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(order_ids.len(), 1);
        let order_id = order_ids[0];

        let max_price = _3_4.saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![order_id];
        let strategy = Strategy::LimitOrder;
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount,
            max_price,
            orders,
            strategy,
        ));

        let order = Orders::<Runtime>::get(order_id).unwrap();
        assert_eq!(
            order,
            Order {
                market_id,
                maker: charlie(),
                maker_asset: Asset::CategoricalOutcome(market_id, 0),
                maker_amount: _2,
                taker_asset: BASE_ASSET,
                taker_amount: _1,
            }
        );
    });
}

#[test]
fn buy_only_executes_first_order_from_orders_vector() {
    ExtBuilder::default().build().execute_with(|| {
        let liquidity = _10;
        let spot_prices = vec![_1_2, _1_2];
        let swap_fee = CENT_BASE;
        let asset_count = 2u16;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            liquidity,
            spot_prices.clone(),
            swap_fee,
        );

        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = _1;

        let order_maker_amount = _2;
        assert_ok!(AssetManager::deposit(asset, &charlie(), order_maker_amount));
        assert_ok!(Orderbook::place_order(
            RuntimeOrigin::signed(charlie()),
            market_id,
            Asset::CategoricalOutcome(market_id, 0),
            order_maker_amount,
            BASE_ASSET,
            _1,
        ));

        let order_maker_amount = _2;
        assert_ok!(AssetManager::deposit(asset, &dave(), order_maker_amount));
        assert_ok!(Orderbook::place_order(
            RuntimeOrigin::signed(dave()),
            market_id,
            Asset::CategoricalOutcome(market_id, 0),
            order_maker_amount,
            BASE_ASSET,
            _1,
        ));

        let order_ids = Orders::<Runtime>::iter().map(|(k, _)| k).collect::<Vec<_>>();
        assert_eq!(order_ids.len(), 2);
        let order_id_0 = order_ids[0];
        let order_id_1 = order_ids[1];

        let max_price = _3_4.saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![order_id_0, order_id_1];
        let strategy = Strategy::LimitOrder;
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount,
            max_price,
            orders,
            strategy,
        ));

        assert!(Orders::<Runtime>::get(order_id_0).is_none());

        let order = Orders::<Runtime>::get(order_id_1).unwrap();
        assert_eq!(
            order,
            Order {
                market_id,
                maker: charlie(),
                maker_asset: Asset::CategoricalOutcome(market_id, 0),
                maker_amount: _2,
                taker_asset: BASE_ASSET,
                taker_amount: _1,
            }
        );
    });
}

#[test]
fn buy_skips_fill_order_if_order_not_present_and_places_new_order() {
    ExtBuilder::default().build().execute_with(|| {
        let market_id = 0;
        let mut market = market_mock::<Runtime>(market_creator());
        market.base_asset = BASE_ASSET;
        let required_asset_count = match &market.market_type {
            MarketType::Scalar(_) => panic!("Categorical market type is expected!"),
            MarketType::Categorical(categories) => *categories,
        };
        market.status = MarketStatus::Active;
        Markets::<Runtime>::insert(market_id, market);

        let asset_count = required_asset_count;
        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = 10 * BASE;
        let max_price = (BASE / 2).saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![42];
        let strategy = Strategy::LimitOrder;
        assert_ok!(HybridRouter::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            asset,
            amount,
            max_price,
            orders,
            strategy,
        ));

        let order = Orders::<Runtime>::get(0).unwrap();
        assert_eq!(
            order,
            Order {
                market_id,
                maker: alice(),
                maker_asset: BASE_ASSET,
                maker_amount: 10 * BASE,
                taker_asset: Asset::CategoricalOutcome(market_id, 0),
                taker_amount: 20 * BASE,
            }
        );
    });
}

#[test]
fn buy_fails_if_max_orders_exceeded() {
    ExtBuilder::default().build().execute_with(|| {
        let market_id = 0;
        let mut market = market_mock::<Runtime>(market_creator());
        let required_asset_count = match &market.market_type {
            MarketType::Scalar(_) => panic!("Categorical market type is expected!"),
            MarketType::Categorical(categories) => *categories,
        };
        market.status = MarketStatus::Active;
        Markets::<Runtime>::insert(market_id, market);

        let asset_count = required_asset_count;
        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = 10 * BASE;
        let max_price = (BASE / 2).saturated_into::<BalanceOf<Runtime>>();
        let orders = (0u128..100u128 + 1u128).collect::<Vec<_>>();
        let strategy = Strategy::LimitOrder;
        assert_noop!(
            HybridRouter::buy(
                RuntimeOrigin::signed(alice()),
                market_id,
                asset_count,
                asset,
                amount,
                max_price,
                orders,
                strategy,
            ),
            Error::<Runtime>::MaxOrdersExceeded
        );
    });
}

#[test]
fn buy_fails_if_amount_is_zero() {
    ExtBuilder::default().build().execute_with(|| {
        let market_id = 0;
        let mut market = market_mock::<Runtime>(market_creator());
        let required_asset_count = match &market.market_type {
            MarketType::Scalar(_) => panic!("Categorical market type is expected!"),
            MarketType::Categorical(categories) => *categories,
        };
        market.status = MarketStatus::Active;
        Markets::<Runtime>::insert(market_id, market);

        let asset_count = required_asset_count;
        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = 0;
        let max_price = (BASE / 2).saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![];
        let strategy = Strategy::LimitOrder;
        assert_noop!(
            HybridRouter::buy(
                RuntimeOrigin::signed(alice()),
                market_id,
                asset_count,
                asset,
                amount,
                max_price,
                orders,
                strategy,
            ),
            Error::<Runtime>::AmountIsZero
        );
    });
}

#[test]
fn buy_fails_if_cancel_strategy_applied() {
    ExtBuilder::default().build().execute_with(|| {
        let market_id = 0;
        let mut market = market_mock::<Runtime>(market_creator());
        let required_asset_count = match &market.market_type {
            MarketType::Scalar(_) => panic!("Categorical market type is expected!"),
            MarketType::Categorical(categories) => *categories,
        };
        market.status = MarketStatus::Active;
        Markets::<Runtime>::insert(market_id, market);

        let asset_count = required_asset_count;
        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = 10 * BASE;
        let max_price = (BASE / 2).saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![];
        let strategy = Strategy::ImmediateOrCancel;
        assert_noop!(
            HybridRouter::buy(
                RuntimeOrigin::signed(alice()),
                market_id,
                asset_count,
                asset,
                amount,
                max_price,
                orders,
                strategy,
            ),
            Error::<Runtime>::CancelStrategyApplied
        );
    });
}

#[test]
fn buy_fails_if_market_does_not_exist() {
    ExtBuilder::default().build().execute_with(|| {
        let market_id = 0;
        let asset_count = 2;
        let asset = Asset::CategoricalOutcome(market_id, 0);
        let amount = 10 * BASE;
        let max_price = (BASE / 2).saturated_into::<BalanceOf<Runtime>>();
        let orders = vec![];
        let strategy = Strategy::ImmediateOrCancel;
        assert_noop!(
            HybridRouter::buy(
                RuntimeOrigin::signed(alice()),
                market_id,
                asset_count,
                asset,
                amount,
                max_price,
                orders,
                strategy,
            ),
            MError::<Runtime>::MarketDoesNotExist
        );
    });
}
