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

// Chain contant
pub const TNF_CHAIN_PREFIX: u16 = 42u16;

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

    pub const PICO_TNF: Balance = 1_000_000;
    pub const NANO_TNF: Balance = 1_000 * PICO_TNF;
    pub const MICRO_TNF: Balance = 1_000 * NANO_TNF;
    pub const MILLI_TNF: Balance = 1_000 * MICRO_TNF;
    pub const TNF: Balance = 1_000 * MILLI_TNF;

    pub const fn deposit(items: u32, bytes: u32) -> Balance {
        items as Balance * 10 * MILLI_TNF + (bytes as Balance) * 1 * MILLI_TNF
    }

    #[cfg(test)]
    mod test_tnf_currency_constants {
        use super::*;

        /// Checks that the native token amounts are correct.
        #[test]
        fn tnfd_amounts() {
            assert_eq!(TNF, 1_000_000_000_000_000_000, "TNF should be 1_000_000_000_000_000_000");
            assert_eq!(MILLI_TNF, 1_000_000_000_000_000, "mTNF should be 1_000_000_000_000_000");
            assert_eq!(MICRO_TNF, 1_000_000_000_000, "μTNF should be 1_000_000_000_000");
            assert_eq!(NANO_TNF, 1_000_000_000, "nTNF should be 1_000_000_000");
            assert_eq!(PICO_TNF, 1_000_000, "pTNF should be 1_000_000");
        }
    }
}
