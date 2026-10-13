use frame_support::{
    pallet_prelude::*,
    storage::unhashed,
    traits::{Get, GetStorageVersion, OnRuntimeUpgrade},
    weights::Weight,
};
use parity_scale_codec::DecodeAll;

use crate::*;

#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;

pub(crate) mod v3 {
    use super::*;
    use frame_support::storage_alias;

    /// V3 type for [`crate::RewardPot`].
    #[storage_alias]
    pub type RewardPot<T: crate::Config> = StorageMap<
        crate::Pallet<T>,
        Blake2_128Concat,
        RewardPeriodIndex,
        RewardPotInfo<BalanceOf<T>>,
        OptionQuery,
    >;

    #[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct RewardPotInfo<Balance> {
        pub total_reward: Balance,
        pub uptime_threshold: u32,
    }

    /// V3 global reward amount, removed in v4.
    #[storage_alias]
    pub type RewardAmount<T: crate::Config> =
        StorageValue<crate::Pallet<T>, BalanceOf<T>, ValueQuery>;
}

pub(crate) mod v4_testnet {
    use super::*;
    use frame_support::storage_alias;

    /// Per-owner node count. Only ever written on the public testnet, where it was storage v4.
    #[storage_alias]
    pub type OwnedNodesCount<T: crate::Config> = StorageMap<
        crate::Pallet<T>,
        Blake2_128Concat,
        <T as frame_system::Config>::AccountId,
        u32,
        ValueQuery,
    >;
}

/// Brings node manager storage to the mainnet layout, at storage v5.
///
/// Mainnet and the public testnet both report storage v4, with different layouts:
/// - Mainnet: `RewardPot` entries carry the funding state (29 bytes each), `OutstandingRewardToPay`
///   is tracked and there is no global `RewardAmount`.
/// - Testnet: `RewardPot` entries are amounts snapshotted at the end of each period (20 bytes
///   each), with a global `RewardAmount` and an `OwnedNodesCount` map.
///
/// Storage v3 has the testnet reward layout without `OwnedNodesCount`.
///
/// The legacy layout is detected from the data, not the storage version. Each `RewardPot` entry
/// is converted only if its raw length matches the legacy encoding, because the legacy type also
/// decodes, wrongly, from a mainnet entry.
///
/// Legacy, ended and unpaid periods keep their snapshotted amount (or get the old `RewardAmount`
/// if they have no pot) as funded with a closed update window, so they are paid as before.
/// `OutstandingRewardToPay` is seeded from them, and `RewardAmount` and `OwnedNodesCount` are
/// removed. On mainnet only the storage version changes.
pub struct MainnetAlignmentUpgrade<T>(PhantomData<T>);
impl<T: Config> OnRuntimeUpgrade for MainnetAlignmentUpgrade<T> {
    fn on_runtime_upgrade() -> Weight {
        let current = Pallet::<T>::current_storage_version();
        let onchain = Pallet::<T>::on_chain_storage_version();

        if onchain >= current || onchain < 3 {
            log::info!(
                "ℹ️  Node manager MainnetAlignmentUpgrade skipped. Current storage version {:?} / onchain {:?}",
                current,
                onchain
            );
            return T::DbWeight::get().reads(1)
        }

        let (has_legacy_layout, detection_reads) = has_legacy_layout::<T>();
        if onchain == 3 || has_legacy_layout {
            return migrate_legacy_layout::<T>()
                .saturating_add(T::DbWeight::get().reads(detection_reads.saturating_add(1)))
        }

        STORAGE_VERSION.put::<Pallet<T>>();
        log::info!(
            "✅ Node manager already on the mainnet layout. Storage version set to {:?}",
            current
        );
        T::DbWeight::get().reads_writes(detection_reads.saturating_add(1), 1)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        let onchain = Pallet::<T>::on_chain_storage_version();
        let will_migrate = onchain < Pallet::<T>::current_storage_version() && onchain >= 3;
        let legacy = will_migrate && (onchain == 3 || has_legacy_layout::<T>().0);

        let raw_pots: Vec<(RewardPeriodIndex, Vec<u8>)> = RewardPot::<T>::iter_keys()
            .filter_map(|period| raw_pot::<T>(period).map(|raw| (period, raw)))
            .collect();

        log::info!(
            "Node manager pre-upgrade: onchain {:?}, legacy layout {:?}, {:?} reward pots",
            onchain,
            legacy,
            raw_pots.len()
        );

        Ok((
            will_migrate,
            legacy,
            raw_pots,
            OutstandingRewardToPay::<T>::get(),
            v3::RewardAmount::<T>::get(),
        )
            .encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(input: Vec<u8>) -> Result<(), TryRuntimeError> {
        let (will_migrate, legacy, raw_pots, outstanding_before, old_reward_amount): (
            bool,
            bool,
            Vec<(RewardPeriodIndex, Vec<u8>)>,
            BalanceOf<T>,
            BalanceOf<T>,
        ) = Decode::decode(&mut input.as_slice()).map_err(|_| "Invalid pre_upgrade state")?;

        ensure!(
            Pallet::<T>::on_chain_storage_version() == Pallet::<T>::current_storage_version(),
            "Storage version not updated"
        );
        if !will_migrate {
            return Ok(())
        }

        // Every reward pot must decode with the current type
        let pot_keys = RewardPot::<T>::iter_keys().count();
        let pot_values = RewardPot::<T>::iter_values().count();
        ensure!(pot_keys == pot_values, "A RewardPot entry does not decode");

        for (period, raw) in raw_pots.iter() {
            let pot = RewardPot::<T>::get(period).ok_or("RewardPot entry lost")?;
            match decode_legacy_pot::<T>(raw) {
                Some(old_pot) => {
                    ensure!(pot.total_reward == old_pot.total_reward, "RewardPot amount changed");
                    ensure!(
                        pot.uptime_threshold == old_pot.uptime_threshold,
                        "Uptime threshold changed"
                    );
                    ensure!(pot.funded, "Migrated RewardPot not funded");
                    ensure!(
                        pot.reward_end_time == 0,
                        "Migrated RewardPot update window not closed"
                    );
                },
                None => ensure!(&pot.encode() == raw, "Mainnet RewardPot entry changed"),
            }
        }

        ensure!(!v3::RewardAmount::<T>::exists(), "RewardAmount not removed");
        ensure!(
            v4_testnet::OwnedNodesCount::<T>::iter_keys().next().is_none(),
            "OwnedNodesCount not removed"
        );

        if legacy {
            let oldest = OldestUnpaidRewardPeriodIndex::<T>::get();
            let current_period = RewardPeriod::<T>::get().current;
            for period in oldest..current_period {
                let pot = RewardPot::<T>::get(period).ok_or("Unpaid period without RewardPot")?;
                // Existing pots were checked above. A mainnet pot may legitimately be unfunded.
                if raw_pots.iter().any(|(existing, _)| *existing == period) {
                    continue
                }
                ensure!(
                    pot.total_reward == old_reward_amount,
                    "Backfilled RewardPot amount is not the old RewardAmount"
                );
                ensure!(pot.funded, "Backfilled RewardPot not funded");
                ensure!(pot.reward_end_time == 0, "Backfilled RewardPot update window not closed");
            }

            ensure!(
                OutstandingRewardToPay::<T>::get() == funded_total::<T>(),
                "OutstandingRewardToPay does not match the funded reward pots"
            );
        } else {
            ensure!(
                OutstandingRewardToPay::<T>::get() == outstanding_before,
                "OutstandingRewardToPay changed on the mainnet layout"
            );
        }

        Ok(())
    }
}

fn raw_pot<T: Config>(period: RewardPeriodIndex) -> Option<Vec<u8>> {
    unhashed::get_raw(&RewardPot::<T>::hashed_key_for(period))
}

/// Decodes `raw` as a legacy reward pot, only if it has exactly the legacy encoded length.
fn decode_legacy_pot<T: Config>(raw: &[u8]) -> Option<v3::RewardPotInfo<BalanceOf<T>>> {
    if raw.len() != v3::RewardPotInfo::<BalanceOf<T>>::max_encoded_len() {
        return None
    }
    v3::RewardPotInfo::<BalanceOf<T>>::decode_all(&mut &raw[..]).ok()
}

/// Whether storage still holds any testnet (or v3) data. Returns the number of reads made.
fn has_legacy_layout<T: Config>() -> (bool, u64) {
    if v3::RewardAmount::<T>::exists() {
        return (true, 1)
    }
    if v4_testnet::OwnedNodesCount::<T>::iter_keys().next().is_some() {
        return (true, 2)
    }

    let mut reads = 2u64;
    for period in RewardPot::<T>::iter_keys() {
        reads.saturating_inc();
        if raw_pot::<T>(period).map_or(false, |raw| decode_legacy_pot::<T>(&raw).is_some()) {
            return (true, reads)
        }
    }
    (false, reads)
}

/// Sum of the rewards of all funded reward pots.
fn funded_total<T: Config>() -> BalanceOf<T> {
    RewardPot::<T>::iter_values()
        .filter(|pot| pot.funded)
        .fold(BalanceOf::<T>::zero(), |total, pot| total.saturating_add(pot.total_reward))
}

fn migrate_legacy_layout<T: Config>() -> Weight {
    let old_reward_amount = v3::RewardAmount::<T>::take();
    let owned_nodes_count = v4_testnet::OwnedNodesCount::<T>::clear(u32::MAX, None).unique;

    let periods: Vec<RewardPeriodIndex> = RewardPot::<T>::iter_keys().collect();
    let mut translated = 0u64;
    let mut unexpected = 0u64;
    let new_pot_len = RewardPotInfo::<BalanceOf<T>>::max_encoded_len();
    for period in periods.iter() {
        let Some(raw) = raw_pot::<T>(*period) else { continue };
        match decode_legacy_pot::<T>(&raw) {
            Some(old) => {
                translated.saturating_inc();
                RewardPot::<T>::insert(
                    period,
                    RewardPotInfo::new(old.total_reward, old.uptime_threshold, 0, true),
                );
            },
            None if raw.len() == new_pot_len => {},
            None => {
                unexpected.saturating_inc();
                log::error!(
                    "💔 Node manager RewardPot for period {:?} has an unexpected length {:?}. Left as is.",
                    period,
                    raw.len()
                );
            },
        }
    }

    let reward_period = RewardPeriod::<T>::get();
    let oldest = OldestUnpaidRewardPeriodIndex::<T>::get();
    let mut backfilled = 0u64;
    for period in oldest..reward_period.current {
        if !RewardPot::<T>::contains_key(period) {
            backfilled.saturating_inc();
            RewardPot::<T>::insert(
                period,
                RewardPotInfo::new(
                    old_reward_amount,
                    Pallet::<T>::calculate_uptime_threshold(reward_period.length),
                    0,
                    true,
                ),
            );
        }
    }

    let outstanding = funded_total::<T>();
    OutstandingRewardToPay::<T>::put(outstanding);
    STORAGE_VERSION.put::<Pallet<T>>();

    log::info!(
        "✅ Node manager migrated to the mainnet layout. Translated {:?} reward pots, backfilled {:?}, unexpected {:?}, cleared {:?} owned node counts, outstanding {:?}",
        translated,
        backfilled,
        unexpected,
        owned_nodes_count,
        outstanding
    );

    let pots = periods.len() as u64;
    let period_reads = reward_period.current.saturating_sub(oldest);
    T::DbWeight::get().reads_writes(
        pots.saturating_mul(2)
            .saturating_add(backfilled)
            .saturating_add(period_reads)
            .saturating_add(owned_nodes_count as u64)
            .saturating_add(4),
        translated
            .saturating_add(backfilled)
            .saturating_add(owned_nodes_count as u64)
            .saturating_add(3),
    )
}
