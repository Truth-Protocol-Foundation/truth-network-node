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

pub use crate::types::*;
use frame_support::PalletId;

// Offchain DB key for registered node
pub const REGISTERED_NODE_KEY: &'static [u8; 18] = b"is_registered_node";

// Chain contant
pub const TNF_CHAIN_PREFIX: u16 = 42u16;

// Pallet identifier for node manager
pub const NODE_MANAGER_PALLET_ID: PalletId = PalletId(*b"node_mgr");

// Definitions for time
pub const MILLISECS_PER_BLOCK: u32 = 6000;
pub const BLOCKS_PER_MINUTE: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber); // 10
pub const BLOCKS_PER_HOUR: BlockNumber = BLOCKS_PER_MINUTE * 60; // 600
pub const BLOCKS_PER_DAY: BlockNumber = BLOCKS_PER_HOUR * 24; // 14_400
pub const BLOCKS_PER_YEAR: BlockNumber = (BLOCKS_PER_DAY * 36525) / 100; // 5_259_600
                                                                         // NOTE: Currently it is not possible to change the slot duration after the chain has started.
                                                                         //       Attempting to do so will brick block production.
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK as u64;

pub mod currency {
    use crate::types::Balance;

    // Definitions for currency used in Prediction market
    pub const DECIMALS: u8 = 10;
    pub const BASE: u128 = 10u128.pow(DECIMALS as u32);
    pub const CENT_BASE: Balance = BASE / 100; // 100_000_000
    pub const MILLI_BASE: Balance = CENT_BASE / 10; //  10_000_000
    pub const MICRO_BASE: Balance = MILLI_BASE / 1000; // 10_000

    pub const fn deposit(items: u32, bytes: u32) -> Balance {
        items as Balance * 50 * CENT_BASE + (bytes as Balance) * 75 * MICRO_BASE
    }

    #[cfg(test)]
    mod test_tnf_currency_constants {
        use super::*;

        /// Checks that the native token amounts are correct.
        #[test]
        fn tnfd_amounts() {
            assert_eq!(BASE, 10_000_000_000, "BASE (Full TRU) should be 10_000_000_000");
            assert_eq!(CENT_BASE, 100_000_000, "cTRU should be 100_000_000");
            assert_eq!(MILLI_BASE, 10_000_000, "mTRU should be 10_000_000");
            assert_eq!(MICRO_BASE, 10_000, "μTRU should be 10_000");
        }
    }
}
