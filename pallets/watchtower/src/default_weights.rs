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
    fn cleanup_expired_votes() -> Weight;

    fn set_challenge_resolution_admin() -> Weight;
    fn resolve_challenge() -> Weight;
    fn submit_challenge() -> Weight;

}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {

    fn vote() -> Weight {
        Weight::from_parts(1000, 0)
    }
    
    fn set_voting_period() -> Weight {
        Weight::from_parts(10_000_000, 0)
            .saturating_add(T::DbWeight::get().writes(1))
    }

    fn ocw_vote() -> Weight {
        Weight::from_parts(1000, 0)
    }
    fn cleanup_expired_votes() -> Weight {
        Weight::from_parts(1000, 0)
    }
    
    fn set_challenge_resolution_admin() -> Weight {
        Weight::from_parts(10_000_000, 0)
            .saturating_add(T::DbWeight::get().writes(1))
    }
    
    fn resolve_challenge() -> Weight {
        Weight::from_parts(30_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    
    fn submit_challenge() -> Weight {
        Weight::from_parts(40_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(3))

    }
}

impl WeightInfo for () {

    fn vote() -> Weight {
        Weight::from_parts(1000, 0);
    }
    
    fn set_voting_period() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }

    fn ocw_vote() -> Weight {
        Weight::from_parts(1000, 0)
    }
    fn cleanup_expired_votes() -> Weight {
        Weight::from_parts(1000, 0)
    }
    
    fn set_challenge_resolution_admin() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
    
    fn resolve_challenge() -> Weight {
        Weight::from_parts(30_000_000, 0)
    }
    
    fn submit_challenge() -> Weight {
        Weight::from_parts(40_000_000, 0)

    }
}