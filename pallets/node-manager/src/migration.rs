use frame_support::{
    pallet_prelude::*,
    traits::{Get, GetStorageVersion, OnRuntimeUpgrade},
    weights::Weight,
};

use crate::*;

#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;

pub struct OwnedNodesUpgrade<T>(PhantomData<T>);
impl<T: Config> OnRuntimeUpgrade for OwnedNodesUpgrade<T> {
    fn on_runtime_upgrade() -> Weight {
        let current = Pallet::<T>::current_storage_version();
        let onchain = Pallet::<T>::on_chain_storage_version();

        log::info!(
            "ℹ️  Node manager invoked with current storage version {:?} / onchain {:?}",
            current,
            onchain
        );

        let mut consumed_weight = Weight::zero();
        if onchain == 3 && current == 4 {
            consumed_weight.saturating_accrue(populate_owned_nodes_count::<T>());
        }

        consumed_weight
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        let count = OwnedNodesCount::<T>::iter().count() as u32;
        assert_eq!(count, 0);
        Ok(())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(input: Vec<u8>) -> Result<(), TryRuntimeError> {
        let current = Pallet::<T>::current_storage_version();
        let onchain = Pallet::<T>::on_chain_storage_version();

        // Sum up all the values of OwnedNodesCount
        let current_count: u32 = OwnedNodesCount::<T>::iter_values().sum();
        let total_nodes = TotalRegisteredNodes::<T>::get();
        assert_eq!(total_nodes, current_count);
        assert!(onchain == 4 && current == 4);

        Ok(())
    }
}

fn populate_owned_nodes_count<T: Config>() -> Weight {
    let mut count = 0u64;
    for (owner, _node_id) in OwnedNodes::<T>::iter_keys() {
        count = count.saturating_add(1);
        OwnedNodesCount::<T>::mutate(&owner, |c| *c = c.saturating_add(1));
    }

    log::info!("✅ Populated OwnedNodesCount for {:?} node owners", count);
    return T::DbWeight::get().reads_writes(count + 1, count + 1);
}
