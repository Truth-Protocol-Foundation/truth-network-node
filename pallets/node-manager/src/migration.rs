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
        Ok(vec![])
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_input: Vec<u8>) -> Result<(), TryRuntimeError> {
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
    use sp_std::collections::btree_map::BTreeMap;

    let mut owned_nodes: BTreeMap<T::AccountId, u32> = BTreeMap::new();
    let mut reads: u64 = 0;
    let mut writes: u64 = 0;

    for (owner, _node_id) in OwnedNodes::<T>::iter_keys() {
        reads = reads.saturating_add(1);
        owned_nodes.entry(owner).and_modify(|c| *c = c.saturating_add(1)).or_insert(1);
    }

    for (owner, count) in owned_nodes.into_iter() {
        OwnedNodesCount::<T>::insert(&owner, count);
        writes = writes.saturating_add(1);
    }

    STORAGE_VERSION.put::<Pallet<T>>();
    writes = writes.saturating_add(1);

    log::info!("✅ Populated OwnedNodesCount for {} owners", writes.saturating_sub(1));

    T::DbWeight::get().reads_writes(reads, writes)
}
