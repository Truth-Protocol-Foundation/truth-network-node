//! One-off runtime migrations for realigning the public testnet with mainnet.

use crate::Runtime;
use frame_support::{
    pallet_prelude::*, storage_alias, traits::OnRuntimeUpgrade, weights::Weight, Blake2_128Concat,
};
use pallet_summary::types::ExternalValidationEnum;
use sp_avn_common::watchtower::{ProposalId, ProposalStatusEnum};
use sp_core::H256;
use sp_std::{marker::PhantomData, vec::Vec};

#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;

frame_support::parameter_types! {
    /// Storage prefix of the removed watchtower pallet (was index 29).
    pub const WatchtowerPalletName: &'static str = "Watchtower";
    /// Storage prefix of the removed summary watchtower pallet (was index 49).
    pub const SummaryWatchtowerPalletName: &'static str = "SummaryWatchtower";
}

/// Clears all storage of the removed watchtower pallet.
pub type RemoveWatchtower = frame_support::migrations::RemovePallet<
    WatchtowerPalletName,
    <Runtime as frame_system::Config>::DbWeight,
>;

/// Clears all storage of the removed summary watchtower pallet.
pub type RemoveSummaryWatchtower = frame_support::migrations::RemovePallet<
    SummaryWatchtowerPalletName,
    <Runtime as frame_system::Config>::DbWeight,
>;

/// Storage of the removed watchtower pallet, read before it is cleared.
mod watchtower {
    use super::*;

    /// External reference to proposal id map of the removed `Watchtower` pallet.
    #[storage_alias(verbatim)]
    pub type ExternalRef = StorageMap<Watchtower, Blake2_128Concat, H256, ProposalId, ValueQuery>;
}

/// Summary roots that were waiting on a watchtower vote can no longer be resolved once the
/// watchtower is gone. Hand them to admin review, so they can be accepted or rejected with
/// `admin_resolve_challenge`.
pub struct MoveInFlightSummaryValidationsToAdminReview<I>(PhantomData<I>);

#[cfg(feature = "try-runtime")]
impl<I: 'static> MoveInFlightSummaryValidationsToAdminReview<I>
where
    Runtime: pallet_summary::Config<I>,
{
    fn in_flight_count() -> u32 {
        pallet_summary::ExternalValidationStatus::<Runtime, I>::iter_values()
            .filter(|status| *status == ExternalValidationEnum::ValidationInProgress)
            .count() as u32
    }
}

impl<I: 'static> OnRuntimeUpgrade for MoveInFlightSummaryValidationsToAdminReview<I>
where
    Runtime: pallet_summary::Config<I>,
{
    fn on_runtime_upgrade() -> Weight {
        let mut reads: u64 = 0;
        let mut writes: u64 = 0;

        let refs: Vec<_> = pallet_summary::ExternalValidationRef::<Runtime, I>::iter().collect();
        reads = reads.saturating_add(refs.len() as u64);

        for (external_ref, root_id) in refs {
            reads = reads.saturating_add(2);
            let status = pallet_summary::ExternalValidationStatus::<Runtime, I>::get(&root_id);
            if status != Some(ExternalValidationEnum::ValidationInProgress) {
                continue
            }

            let proposal_id = watchtower::ExternalRef::get(external_ref);
            pallet_summary::Pallet::<Runtime, I>::setup_root_for_admin_review(
                root_id,
                proposal_id,
                external_ref,
                ProposalStatusEnum::Cancelled,
            );
            writes = writes.saturating_add(3);

            log::info!(
                "✅ Summary root {:?} moved from watchtower validation to admin review",
                root_id
            );
        }

        <Runtime as frame_system::Config>::DbWeight::get().reads_writes(reads, writes)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        let in_flight = Self::in_flight_count();
        let pending = pallet_summary::PendingAdminReviews::<Runtime, I>::iter_keys().count() as u32;
        log::info!("Summary roots in watchtower validation: {:?}", in_flight);
        Ok((in_flight, pending).encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
        let (in_flight, pending_before): (u32, u32) =
            Decode::decode(&mut state.as_slice()).map_err(|_| "Invalid pre_upgrade state")?;

        ensure!(Self::in_flight_count() == 0, "Summary roots still waiting on the watchtower");
        let pending_after =
            pallet_summary::PendingAdminReviews::<Runtime, I>::iter_keys().count() as u32;
        ensure!(
            pending_after == pending_before.saturating_add(in_flight),
            "Not every in-flight summary root was moved to admin review"
        );
        Ok(())
    }
}
