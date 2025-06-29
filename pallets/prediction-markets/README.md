# Prediction Markets

A module for creating, reporting, and disputing prediction markets.

## Overview

Prediction markets are speculative markets that trade on future outcomes. While
a prediction market is active, traders can permissionlessly trade on outcome
assets. Complete set of outcome assets can be generated by exchanging base
currency (TRUU) at a 1:1 exchange rate, meaning that 1 TRUU is used to generate 1
of each outcome asset of the market. A complete set of outcome assets (an equal
amount of each outcome asset for a market), can be used to do the reverse
exchange, being used to exchange 1:1 back to TRUU.

This pallet implements a two types of prediction markets:

- **Categorical market** - A market that can trade up to 8 different categories
  of outcomes. In the most simple case, when the outcomes are binary options
  (i.e. "YES" or "NO") the market has only two categories.
- **Scalar markets** - A market that trades `Long` or `Short` positions on a
  range of outcomes.

## Interface

### Dispatches

#### Public Dispatches

- `buy_complete_set` - Buys a complete set of outcome assets for a market.
- `create_categorical_market` - Creates a new categorical market.
- `create_cpmm_market_and_deploy_assets` - Creates a market using CPMM scoring
  rule, buys a complete set of the assets used and deploys the funds.
- `deploy_swap_pool_for_market` - Deploys a single "canonical" pool for a
  market.
- `deploy_swap_pool_and_additional_liquidity` - Deploys a single "canonical"
  pool for a market, buys a complete set of the assets used and deploys the
  funds as specified.
- `dispute` - Submits a disputed outcome for a market.
- `redeem_shares` - Redeems the winning shares for a market.
- `report` - Reports an outcome for a market.
- `sell_complete_set` - Sells a complete set of outcome assets for a market.
- `start_global_dispute` - Starts a global dispute for a market, when the
  `MaxDisputes` amount of disputes is reached.

#### Admin Dispatches

The administrative dispatches are used to perform admin functions on chain:

- `admin_destroy_market` - Destroys a market and all related assets, regardless
  of its state. Can only be called by the `DestroyOrigin`.
- `admin_move_market_to_closed` - Immediately moves a market that is an `Active`
  state to closed. Can only be called by `CloseOrigin`.
- `admin_move_market_to_resolved` - Immediately moves a market that is
  `Reported` or `Disputed` to resolved. Can only be called by `ResolveOrigin`.

The origins from which the admin functions are called (`CloseOrigin`,
`DestroyOrigin`, `ResolveOrigin`) are mainly minimum vote proportions from the
advisory committee, the on-chain governing body of Zeitgeist that is responsible
for maintaining a list of high quality markets and slash low quality markets.

#### `ApproveOrigin` and `RejectOrigin` Dispatches

Users can also propose markets, which are subject to approval or rejection by
the Advisory Committee. The `ApproveOrigin` calls the following dispatches:

- `approve_market` - Approves a `Proposed` market that is waiting approval from
  the Advisory Committee.

The `RejectOrigin` calls the following dispatches:

- `reject_market` - Rejects a `Proposed` market that is waiting approval from
  the Advisory Committee.
