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

//! Autogenerated weights for pallet_pm_global_disputes
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: `2024-08-27`, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `zeitgeist-benchmark`, CPU: `AMD EPYC 7601 32-Core Processor`
//! EXECUTION: ``, WASM-EXECUTION: `Compiled`, CHAIN: `Some("dev")`, DB CACHE: `1024`

// Executed Command:
// ./target/production/zeitgeist
// benchmark
// pallet
// --chain=dev
// --steps=50
// --repeat=20
// --pallet=pallet_pm_global_disputes
// --extrinsic=*
// --execution=wasm
// --wasm-execution=compiled
// --heap-pages=4096
// --template=./misc/weight_template.hbs
// --header=./HEADER_GPL3
// --output=./pallets/global-disputes/src/weights.rs

#![allow(unused_parens)]
#![allow(unused_imports)]

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::Weight};

///  Trait containing the required functions for weight retrival within
/// pallet_pm_global_disputes (automatically generated)
pub trait WeightInfoZeitgeist {
    fn vote_on_outcome(o: u32, v: u32) -> Weight;
    fn unlock_vote_balance_set(l: u32, o: u32) -> Weight;
    fn unlock_vote_balance_remove(l: u32, o: u32) -> Weight;
    fn add_vote_outcome(w: u32) -> Weight;
    fn reward_outcome_owner_shared_possession(o: u32) -> Weight;
    fn reward_outcome_owner_paid_possession() -> Weight;
    fn purge_outcomes(k: u32, o: u32) -> Weight;
    fn refund_vote_fees(k: u32, o: u32) -> Weight;
}

/// Weight functions for pallet_pm_global_disputes (automatically generated)
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfoZeitgeist for WeightInfo<T> {
    /// Storage: `GlobalDisputes::GlobalDisputesInfo` (r:1 w:1)
    /// Proof: `GlobalDisputes::GlobalDisputesInfo` (`max_values`: None, `max_size`: Some(396),
    /// added: 2871, mode: `MaxEncodedLen`) Storage: `GlobalDisputes::Outcomes` (r:1 w:1)
    /// Proof: `GlobalDisputes::Outcomes` (`max_values`: None, `max_size`: Some(395), added: 2870,
    /// mode: `MaxEncodedLen`) Storage: `GlobalDisputes::Locks` (r:1 w:1)
    /// Proof: `GlobalDisputes::Locks` (`max_values`: None, `max_size`: Some(1641), added: 4116,
    /// mode: `MaxEncodedLen`) Storage: `Balances::Locks` (r:1 w:1)
    /// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode:
    /// `MaxEncodedLen`) Storage: `Balances::Freezes` (r:1 w:0)
    /// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(65), added: 2540, mode:
    /// `MaxEncodedLen`) The range of component `o` is `[2, 10]`.
    /// The range of component `v` is `[0, 49]`.
    fn vote_on_outcome(_o: u32, v: u32) -> Weight {
        // Proof Size summary in bytes:
        //  Measured:  `452 + o * (33 ±0) + v * (32 ±0)`
        //  Estimated: `5106`
        // Minimum execution time: 61_281 nanoseconds.
        Weight::from_parts(65_221_662, 5106)
            // Standard Error: 4_175
            .saturating_add(Weight::from_parts(66_837, 0).saturating_mul(v.into()))
            .saturating_add(T::DbWeight::get().reads(5))
            .saturating_add(T::DbWeight::get().writes(4))
    }
    /// Storage: `GlobalDisputes::Locks` (r:1 w:1)
    /// Proof: `GlobalDisputes::Locks` (`max_values`: None, `max_size`: Some(1641), added: 4116,
    /// mode: `MaxEncodedLen`) Storage: `GlobalDisputes::GlobalDisputesInfo` (r:50 w:0)
    /// Proof: `GlobalDisputes::GlobalDisputesInfo` (`max_values`: None, `max_size`: Some(396),
    /// added: 2871, mode: `MaxEncodedLen`) Storage: `Balances::Locks` (r:1 w:1)
    /// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode:
    /// `MaxEncodedLen`) Storage: `Balances::Freezes` (r:1 w:0)
    /// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(65), added: 2540, mode:
    /// `MaxEncodedLen`) Storage: `System::Account` (r:1 w:0)
    /// Proof: `System::Account` (`max_values`: None, `max_size`: Some(132), added: 2607, mode:
    /// `MaxEncodedLen`) The range of component `l` is `[0, 50]`.
    /// The range of component `o` is `[1, 10]`.
    fn unlock_vote_balance_set(l: u32, o: u32) -> Weight {
        // Proof Size summary in bytes:
        //  Measured:  `0 + l * (435 ±0) + o * (1600 ±0)`
        //  Estimated: `5106 + l * (2871 ±0)`
        // Minimum execution time: 34_631 nanoseconds.
        Weight::from_parts(31_327_635, 5106)
            // Standard Error: 9_052
            .saturating_add(Weight::from_parts(3_571_866, 0).saturating_mul(l.into()))
            // Standard Error: 47_513
            .saturating_add(Weight::from_parts(818_545, 0).saturating_mul(o.into()))
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(l.into())))
            .saturating_add(T::DbWeight::get().writes(2))
            .saturating_add(Weight::from_parts(0, 2871).saturating_mul(l.into()))
    }
    /// Storage: `GlobalDisputes::Locks` (r:1 w:1)
    /// Proof: `GlobalDisputes::Locks` (`max_values`: None, `max_size`: Some(1641), added: 4116,
    /// mode: `MaxEncodedLen`) Storage: `GlobalDisputes::GlobalDisputesInfo` (r:50 w:0)
    /// Proof: `GlobalDisputes::GlobalDisputesInfo` (`max_values`: None, `max_size`: Some(396),
    /// added: 2871, mode: `MaxEncodedLen`) Storage: `Balances::Locks` (r:1 w:1)
    /// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode:
    /// `MaxEncodedLen`) Storage: `Balances::Freezes` (r:1 w:0)
    /// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(65), added: 2540, mode:
    /// `MaxEncodedLen`) Storage: `System::Account` (r:1 w:1)
    /// Proof: `System::Account` (`max_values`: None, `max_size`: Some(132), added: 2607, mode:
    /// `MaxEncodedLen`) The range of component `l` is `[0, 50]`.
    /// The range of component `o` is `[1, 10]`.
    fn unlock_vote_balance_remove(l: u32, o: u32) -> Weight {
        // Proof Size summary in bytes:
        //  Measured:  `0 + l * (419 ±0) + o * (1600 ±0)`
        //  Estimated: `5106 + l * (2871 ±0)`
        // Minimum execution time: 35_101 nanoseconds.
        Weight::from_parts(30_900_200, 5106)
            // Standard Error: 9_052
            .saturating_add(Weight::from_parts(3_495_887, 0).saturating_mul(l.into()))
            // Standard Error: 47_512
            .saturating_add(Weight::from_parts(661_905, 0).saturating_mul(o.into()))
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(l.into())))
            .saturating_add(T::DbWeight::get().writes(3))
            .saturating_add(Weight::from_parts(0, 2871).saturating_mul(l.into()))
    }
    /// Storage: `MarketCommons::Markets` (r:1 w:0)
    /// Proof: `MarketCommons::Markets` (`max_values`: None, `max_size`: Some(694), added: 3169,
    /// mode: `MaxEncodedLen`) Storage: `GlobalDisputes::GlobalDisputesInfo` (r:1 w:1)
    /// Proof: `GlobalDisputes::GlobalDisputesInfo` (`max_values`: None, `max_size`: Some(396),
    /// added: 2871, mode: `MaxEncodedLen`) Storage: `GlobalDisputes::Outcomes` (r:1 w:1)
    /// Proof: `GlobalDisputes::Outcomes` (`max_values`: None, `max_size`: Some(395), added: 2870,
    /// mode: `MaxEncodedLen`) Storage: `System::Account` (r:1 w:1)
    /// Proof: `System::Account` (`max_values`: None, `max_size`: Some(132), added: 2607, mode:
    /// `MaxEncodedLen`) The range of component `w` is `[1, 10]`.
    fn add_vote_outcome(w: u32) -> Weight {
        // Proof Size summary in bytes:
        //  Measured:  `680 + w * (32 ±0)`
        //  Estimated: `4159`
        // Minimum execution time: 78_902 nanoseconds.
        Weight::from_parts(82_020_410, 4159)
            // Standard Error: 18_347
            .saturating_add(Weight::from_parts(141_445, 0).saturating_mul(w.into()))
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Storage: `GlobalDisputes::Outcomes` (r:1 w:0)
    /// Proof: `GlobalDisputes::Outcomes` (`max_values`: None, `max_size`: Some(395), added: 2870,
    /// mode: `MaxEncodedLen`) Storage: `GlobalDisputes::GlobalDisputesInfo` (r:1 w:0)
    /// Proof: `GlobalDisputes::GlobalDisputesInfo` (`max_values`: None, `max_size`: Some(396),
    /// added: 2871, mode: `MaxEncodedLen`) Storage: `System::Account` (r:11 w:11)
    /// Proof: `System::Account` (`max_values`: None, `max_size`: Some(132), added: 2607, mode:
    /// `MaxEncodedLen`) The range of component `o` is `[1, 10]`.
    fn reward_outcome_owner_shared_possession(o: u32) -> Weight {
        // Proof Size summary in bytes:
        //  Measured:  `464 + o * (41 ±0)`
        //  Estimated: `3861 + o * (2790 ±20)`
        // Minimum execution time: 82_822 nanoseconds.
        Weight::from_parts(45_008_105, 3861)
            // Standard Error: 216_175
            .saturating_add(Weight::from_parts(47_290_206, 0).saturating_mul(o.into()))
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(o.into())))
            .saturating_add(T::DbWeight::get().writes(1))
            .saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(o.into())))
            .saturating_add(Weight::from_parts(0, 2790).saturating_mul(o.into()))
    }
    /// Storage: `GlobalDisputes::Outcomes` (r:1 w:0)
    /// Proof: `GlobalDisputes::Outcomes` (`max_values`: None, `max_size`: Some(395), added: 2870,
    /// mode: `MaxEncodedLen`) Storage: `GlobalDisputes::GlobalDisputesInfo` (r:1 w:0)
    /// Proof: `GlobalDisputes::GlobalDisputesInfo` (`max_values`: None, `max_size`: Some(396),
    /// added: 2871, mode: `MaxEncodedLen`) Storage: `System::Account` (r:2 w:2)
    /// Proof: `System::Account` (`max_values`: None, `max_size`: Some(132), added: 2607, mode:
    /// `MaxEncodedLen`)
    fn reward_outcome_owner_paid_possession() -> Weight {
        // Proof Size summary in bytes:
        //  Measured:  `511`
        //  Estimated: `6204`
        // Minimum execution time: 81_543 nanoseconds.
        Weight::from_parts(83_572_000, 6204)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    /// Storage: `GlobalDisputes::GlobalDisputesInfo` (r:1 w:1)
    /// Proof: `GlobalDisputes::GlobalDisputesInfo` (`max_values`: None, `max_size`: Some(396),
    /// added: 2871, mode: `MaxEncodedLen`) Storage: `GlobalDisputes::Outcomes` (r:250 w:249)
    /// Proof: `GlobalDisputes::Outcomes` (`max_values`: None, `max_size`: Some(395), added: 2870,
    /// mode: `MaxEncodedLen`) The range of component `k` is `[2, 248]`.
    /// The range of component `o` is `[1, 10]`.
    fn purge_outcomes(k: u32, _o: u32) -> Weight {
        // Proof Size summary in bytes:
        //  Measured:  `385 + k * (90 ±0) + o * (32 ±0)`
        //  Estimated: `6730 + k * (2870 ±0)`
        // Minimum execution time: 48_611 nanoseconds.
        Weight::from_parts(50_185_138, 6730)
            // Standard Error: 11_693
            .saturating_add(Weight::from_parts(6_965_478, 0).saturating_mul(k.into()))
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(k.into())))
            .saturating_add(T::DbWeight::get().writes(2))
            .saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(k.into())))
            .saturating_add(Weight::from_parts(0, 2870).saturating_mul(k.into()))
    }
    /// Storage: `GlobalDisputes::GlobalDisputesInfo` (r:1 w:0)
    /// Proof: `GlobalDisputes::GlobalDisputesInfo` (`max_values`: None, `max_size`: Some(396),
    /// added: 2871, mode: `MaxEncodedLen`) Storage: `GlobalDisputes::Outcomes` (r:250 w:249)
    /// Proof: `GlobalDisputes::Outcomes` (`max_values`: None, `max_size`: Some(395), added: 2870,
    /// mode: `MaxEncodedLen`) The range of component `k` is `[2, 248]`.
    /// The range of component `o` is `[1, 10]`.
    fn refund_vote_fees(k: u32, _o: u32) -> Weight {
        // Proof Size summary in bytes:
        //  Measured:  `385 + k * (90 ±0) + o * (32 ±0)`
        //  Estimated: `6730 + k * (2870 ±0)`
        // Minimum execution time: 45_662 nanoseconds.
        Weight::from_parts(58_765_984, 6730)
            // Standard Error: 12_445
            .saturating_add(Weight::from_parts(6_889_728, 0).saturating_mul(k.into()))
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(k.into())))
            .saturating_add(T::DbWeight::get().writes(1))
            .saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(k.into())))
            .saturating_add(Weight::from_parts(0, 2870).saturating_mul(k.into()))
    }
}
