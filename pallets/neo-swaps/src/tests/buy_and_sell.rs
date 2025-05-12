// Copyright 2023 Forecasting Technologies LTD.
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
use common_primitives::constants::currency::BASE;

#[test]
fn buy_and_sell() {
    ExtBuilder::default().build().execute_with(|| {
        let asset_count = 3;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(asset_count),
            _100,
            vec![_1_2, _1_4, _1_4],
            CENT_BASE,
        );
        assert_ok!(AssetManager::deposit(BASE_ASSET, &alice(), _1000));
        assert_ok!(AssetManager::deposit(BASE_ASSET, &bob(), _1000));
        assert_ok!(AssetManager::deposit(BASE_ASSET, &charlie(), _1000));

        assert_ok!(NeoSwaps::buy(
            RuntimeOrigin::signed(alice()),
            market_id,
            asset_count,
            Asset::CategoricalOutcome(market_id, 2),
            _10,
            0,
        ));
        assert_pool_state!(
            market_id,
            vec![598999000000, 1098999000000, 765201996010],
            [4358797240, 2179398620, 3461804140],
            721_347_520_444,
            create_b_tree_map!({ alice() => _100 }),
            1_000_000_000,
        );

        assert_ok!(NeoSwaps::buy(
            RuntimeOrigin::signed(bob()),
            market_id,
            asset_count,
            Asset::CategoricalOutcome(market_id, 1),
            1234567898765,
            0,
        ));
        assert_pool_state!(
            market_id,
            vec![1821220219777, 111887179152, 1987423215787],
            [800785444, 8563222054, 635992503],
            721_347_520_444,
            create_b_tree_map!({ alice() => _100 }),
            13_345_678_988,
        );

        assert_ok!(NeoSwaps::buy(
            RuntimeOrigin::signed(charlie()),
            market_id,
            asset_count,
            Asset::CategoricalOutcome(market_id, 0),
            667 * BASE,
            0,
        ));
        assert_pool_state!(
            market_id,
            vec![70199543, 6715186179152, 8590722215787],
            [9999026875, 905847, 67278],
            721_347_520_444,
            create_b_tree_map!({ alice() => _100 }),
            80045678988,
        );

        // Selling asset 2 is illegal due to low spot price.
        assert_noop!(
            NeoSwaps::sell(
                RuntimeOrigin::signed(alice()),
                market_id,
                asset_count,
                Asset::CategoricalOutcome(market_id, 2),
                123_456,
                0,
            ),
            Error::<Runtime>::NumericalLimits(NumericalLimitsError::SpotPriceTooLow),
        );

        assert_ok!(NeoSwaps::sell(
            RuntimeOrigin::signed(charlie()),
            market_id,
            asset_count,
            Asset::CategoricalOutcome(market_id, 0),
            _1,
            0,
        ));
        assert_pool_state!(
            market_id,
            vec![71179444, 6705187159053, 8580723195688],
            [9999013292, 918491, 68217],
            721_347_520_444,
            create_b_tree_map!({ alice() => _100 }),
            80145669189,
        );

        // Selling asset 1 is allowed, but selling too much will raise an error.
        assert_noop!(
            NeoSwaps::sell(
                RuntimeOrigin::signed(bob()),
                market_id,
                asset_count,
                Asset::CategoricalOutcome(market_id, 1),
                _100,
                0,
            ),
            Error::<Runtime>::NumericalLimits(NumericalLimitsError::SpotPriceSlippedTooLow),
        );

        // Try to sell more than the maximum amount.
        assert_noop!(
            NeoSwaps::sell(
                RuntimeOrigin::signed(bob()),
                market_id,
                asset_count,
                Asset::CategoricalOutcome(market_id, 1),
                _1000,
                0,
            ),
            Error::<Runtime>::NumericalLimits(NumericalLimitsError::MaxAmountExceeded),
        );

        // Buying a small amount from an asset with a low price fails...
        assert_noop!(
            NeoSwaps::buy(
                RuntimeOrigin::signed(charlie()),
                market_id,
                asset_count,
                Asset::CategoricalOutcome(market_id, 2),
                _1,
                0,
            ),
            Error::<Runtime>::NumericalLimits(NumericalLimitsError::MinAmountNotMet),
        );

        // ...but buying a large amount is fine.
        assert_ok!(NeoSwaps::buy(
            RuntimeOrigin::signed(charlie()),
            market_id,
            asset_count,
            Asset::CategoricalOutcome(market_id, 2),
            _100,
            0,
        ));
        assert_pool_state!(
            market_id,
            vec![990070179444, 7695186159053, 210881797230],
            [2534652093, 232829, 7465115079],
            721_347_520_444,
            create_b_tree_map!({ alice() => _100 }),
            90145669189,
        );
    });
}
