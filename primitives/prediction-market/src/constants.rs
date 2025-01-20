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
    // Constants parameters inside `parameter_types!` already check
    // arithmetic operations at compile time
    clippy::arithmetic_side_effects
)]

// #[cfg(any(feature = "mock", feature = "runtime-benchmarks"))]
pub mod base_multiples;
#[cfg(any(feature = "mock", feature = "runtime-benchmarks"))]
pub mod mock;

use common_primitives::{
    constants::{BLOCKS_PER_DAY, BLOCKS_PER_HOUR, BLOCKS_PER_YEAR},
    types::{Balance, BlockNumber},
};
use frame_support::PalletId;

// Definitions for currency used in Prediction market
pub const DECIMALS: u8 = 10;
pub const BASE: u128 = 10u128.pow(DECIMALS as u32);
pub const CENT_BASE: Balance = BASE / 100; // 100_000_000
pub const MILLI_BASE: Balance = CENT_BASE / 10; //  10_000_000
pub const MICRO_BASE: Balance = MILLI_BASE / 1000; // 10_000

// Authorized
/// Pallet identifier, mainly used for named balance reserves.
pub const AUTHORIZED_PALLET_ID: PalletId = PalletId(*b"tnf/atzd");

// Court
/// Pallet identifier, mainly used for named balance reserves.
pub const COURT_PALLET_ID: PalletId = PalletId(*b"tnf/cout");
/// Lock identifier, mainly used for the locks on the accounts.
pub const COURT_LOCK_ID: [u8; 8] = *b"tnf/colk";

// Orderbook
pub const ORDERBOOK_PALLET_ID: PalletId = PalletId(*b"tnf/ordb");

// Hybrid Router
/// Pallet identifier, mainly used for named balance reserves.
pub const HYBRID_ROUTER_PALLET_ID: PalletId = PalletId(*b"tnf/hybr");

// NeoSwaps
pub const NS_PALLET_ID: PalletId = PalletId(*b"tnf/neos");

/// Treasury - Pallet identifier, used to derive treasury account
pub const TREASURY_PALLET_ID: PalletId = PalletId(*b"Treasury");

/// Prediction market - Pallet identifier, mainly used for named balance reserves.
pub const PM_PALLET_ID: PalletId = PalletId(*b"tnf/pred");
/// Max. categories in a prediction market.
pub const MAX_CATEGORIES: u16 = 64;
/// The dispute_duration is time where users can dispute the outcome.
/// Minimum block period for a dispute.
pub const MIN_DISPUTE_DURATION: BlockNumber = 12 * BLOCKS_PER_HOUR;
/// Maximum block period for a dispute.
pub const MAX_DISPUTE_DURATION: BlockNumber = 30 * BLOCKS_PER_DAY;
/// Maximum block period for an grace_period.
/// The grace_period is a delay between the point where the market closes and the point where the
/// oracle may report.
pub const MAX_GRACE_PERIOD: BlockNumber = BLOCKS_PER_YEAR;
/// The maximum allowed market life time, measured in blocks.
pub const MAX_MARKET_LIFETIME: BlockNumber = 4 * BLOCKS_PER_YEAR;
/// Maximum block period for an oracle_duration.
/// The oracle_duration is a duration where the oracle has to submit its report.
pub const MAX_ORACLE_DURATION: BlockNumber = 14 * BLOCKS_PER_DAY;
/// Minimum block period for oracle_duration.
pub const MIN_ORACLE_DURATION: BlockNumber = BLOCKS_PER_HOUR;

// Global Disputes
pub const GLOBAL_DISPUTES_PALLET_ID: PalletId = PalletId(*b"tnf/gldp");
/// Lock identifier, mainly used for the locks on the accounts.
pub const GLOBAL_DISPUTES_LOCK_ID: [u8; 8] = *b"tnf/gdlk";

// Swaps
/// Max. assets in a swap pool.
pub const MAX_ASSETS: u16 = MAX_CATEGORIES + 1;
