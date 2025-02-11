//! # Node manager benchmarks
// Copyright 2025 Truth Network.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::{EventRecord, RawOrigin};
use prediction_market_primitives::math::fixed::FixedMulDiv;
use sp_runtime::SaturatedConversion;
