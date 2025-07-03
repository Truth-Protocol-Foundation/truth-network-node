#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

pub trait WeightInfo {
    fn vote() -> Weight;
    fn set_voting_period() -> Weight;
    fn ocw_vote() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn vote() -> Weight {
        Weight::from_parts(1000, 0)
    }
    fn set_voting_period() -> Weight {
        Weight::from_parts(1000, 0)
    }
    fn ocw_vote() -> Weight {
        Weight::from_parts(1000, 0)
    }
}

impl WeightInfo for () {
    fn vote() -> Weight {
        Weight::from_parts(1000, 0)
    }
    fn set_voting_period() -> Weight {
        Weight::from_parts(1000, 0)
    }
    fn ocw_vote() -> Weight {
        Weight::from_parts(1000, 0)
    }
}