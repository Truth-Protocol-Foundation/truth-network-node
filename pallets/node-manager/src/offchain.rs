// No storage mutation allowed in this file
use crate::*;
// We allow up to 5 blocks for ocw transactions
const BLOCK_INCLUSION_PERIOD: u32 = 5;
const OCW_ID: &'static [u8; 22] = b"node_manager::last_run";
const OC_HB_DB_PREFIX: &[u8] = b"tnf/node-manager-heartbeat/";

impl<T: Config> Pallet<T> {
    pub fn trigger_payment_if_required(reward_period_index: RewardPeriodIndex, author: Author<T>) {
        if Self::can_trigger_payment().unwrap_or(false) {
            log::info!("🌐 Triggering payment for period: {:?}", reward_period_index);

            let signature = author.key.sign(&(PAYOUT_REWARD_CONTEXT, reward_period_index).encode());

            match signature {
                Some(signature) => {
                    let call = Call::<T>::offchain_pay_nodes {
                        reward_period_index,
                        author: author.clone(),
                        signature,
                    };

                    if let Err(e) =
                        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
                    {
                        log::error!("💔 Error submitting transaction to trigger payment. Period: {:?}, Error: {:?}", reward_period_index, e);
                    }
                },
                None => {
                    log::error!(
                        "💔 Error signing payment transaction. Period: {:?}",
                        reward_period_index
                    );
                },
            }
        }
    }

    pub fn send_heartbeat_if_required(block_number: BlockNumberFor<T>) {
        let maybe_node_key = Self::get_node_from_signing_key();
        if let Some((node, signing_key)) = maybe_node_key {
            let current_reward_period = RewardPeriod::<T>::get().current;
            let uptime_info = <NodeUptime<T>>::get(current_reward_period, &node);
            let heartbeat_count = uptime_info.map(|info| info.count).unwrap_or(0);

            if Self::should_send_heartbeat(
                block_number,
                uptime_info,
                current_reward_period,
                heartbeat_count,
            ) {
                log::info!(
                    "🌐 Sending heartbeat for reward period: {:?}, block number: {:?}",
                    block_number,
                    current_reward_period
                );

                let signature = signing_key
                    .sign(&(HEARTBEAT_CONTEXT, heartbeat_count, current_reward_period).encode());

                match signature {
                    Some(signature) => {
                        let call = Call::<T>::offchain_submit_heartbeat {
                            node,
                            reward_period_index: current_reward_period,
                            heartbeat_count,
                            signature,
                        };

                        match SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()) {
                            Ok(_) => {
                                // If this fails, the extrinsic will still reject duplicates
                                let _ = Self::record_heartbeat_ocw_submission(
                                    block_number,
                                    current_reward_period,
                                    heartbeat_count
                                );
                            },
                            Err(e) => log::error!(
                                "💔 Error submitting heartbeat transaction. Period: {:?}, Heartbeat count: {:?}, Error: {:?}",
                                current_reward_period, heartbeat_count, e),
                        }

                        log::info!(
                            "🌐 heartbeat transaction sent. Reward period: {:?}, Block number: {:?}",
                            block_number, current_reward_period);
                    },
                    None => {
                        log::error!(
                            "💔 Error signing heartbeat transaction. Reward period: {:?}, Block number: {:?}",
                            block_number, current_reward_period);
                    },
                }
            }
        }
    }

    pub fn try_get_node_author(block_number: BlockNumberFor<T>) -> Option<Author<T>> {
        let setup_result = AVN::<T>::pre_run_setup(block_number, OCW_ID.to_vec());
        if let Err(_) = setup_result {
            return None;
        }

        let (this_author, _) = setup_result.expect("We have an author");
        let is_primary = AVN::<T>::is_primary_for_block(block_number, &this_author.account_id);

        if is_primary.is_err() {
            log::error!("💔 Error checking if author is Primary");
            return None;
        }

        return Some(this_author);
    }

    pub fn can_trigger_payment() -> Result<bool, ()> {
        let oldest_period = OldestUnpaidRewardPeriodIndex::<T>::get();
        let current_period = RewardPeriod::<T>::get().current;
        let last_paid_pointer = LastPaidPointer::<T>::get();

        if last_paid_pointer.is_some() {
            log::info!("👷 Resuming payment for period: {:?}", oldest_period);
            return Ok(true);
        }

        if oldest_period < current_period && last_paid_pointer.is_none() {
            log::info!(
                "👷 Triggering payment for period: {:?}. Current period: {:?}",
                oldest_period,
                current_period
            );

            return Ok(true);
        }

        return Ok(false);
    }

    pub fn get_node_from_signing_key() -> Option<(T::AccountId, T::SignerId)> {
        let mut local_keys = T::SignerId::all();
        local_keys.sort();

        if let Some(registered_node_id) = StorageValueRef::persistent(REGISTERED_NODE_KEY)
            .get::<T::AccountId>()
            .ok()
            .flatten()
        {
            if let Some(node_info) = NodeRegistry::<T>::get(&registered_node_id) {
                // If the registered node’s signing key is present locally, return it.
                if local_keys.binary_search(&node_info.signing_key).is_ok() {
                    return Some((registered_node_id, node_info.signing_key));
                }
            }
        }

        log::warn!("🔐 Signing key not found for registered node, searching all nodes.");
        // we couldn't find a signing key for the registered_node_id, so check all registered nodes
        // This is potentially very slow so only do it as a last resort
        return NodeRegistry::<T>::iter()
            .filter_map(move |(node_id, info)| {
                local_keys
                    .binary_search(&info.signing_key)
                    .ok()
                    .map(|_| (node_id, info.signing_key))
            })
            .nth(0)
    }

    pub fn should_send_heartbeat(
        block_number: BlockNumberFor<T>,
        uptime_info: Option<UptimeInfo<BlockNumberFor<T>>>,
        reward_period_index: RewardPeriodIndex,
        heartbeat_count: u64,
    ) -> bool {
        let submission_in_progress = Self::heartbeat_submission_in_progress(
            reward_period_index,
            heartbeat_count,
            block_number,
        );
        if submission_in_progress {
            return false;
        }

        let heartbeat_period = HeartbeatPeriod::<T>::get();
        if let Some(uptime_info) = uptime_info {
            let last_submission = uptime_info.last_reported;
            // Send heartbeat if the current block is at or past the next allowed block.
            return block_number >= last_submission + BlockNumberFor::<T>::from(heartbeat_period);
        } else {
            // First heartbeat
            return true;
        }
    }

    fn record_heartbeat_ocw_submission(
        now: BlockNumberFor<T>,
        reward_period_index: RewardPeriodIndex,
        heartbeat_count: u64,
    ) -> Result<(), Error<T>> {
        let mut key = OC_HB_DB_PREFIX.to_vec();
        key.extend((reward_period_index, heartbeat_count).encode());

        let storage = StorageValueRef::persistent(&key);
        let result =
            storage.mutate(|_: Result<Option<BlockNumberFor<T>>, StorageRetrievalError>| Ok(now));
        match result {
            Err(MutateStorageError::ValueFunctionFailed(e)) => Err(e),
            Err(MutateStorageError::ConcurrentModification(_)) =>
                Err(Error::<T>::FailedToAcquireOcwDbLock),
            Ok(_) => return Ok(()),
        }
    }

    fn heartbeat_submission_in_progress(
        reward_period_index: RewardPeriodIndex,
        heartbeat_count: u64,
        current_block: BlockNumberFor<T>,
    ) -> bool {
        let mut key = OC_HB_DB_PREFIX.to_vec();
        key.extend((reward_period_index, heartbeat_count).encode());

        match StorageValueRef::persistent(&key).get::<BlockNumberFor<T>>().ok().flatten() {
            Some(last_submission) => {
                // Allow BLOCK_INCLUSION_PERIOD blocks for the transaction to be included
                return current_block <=
                    last_submission
                        .saturating_add(BlockNumberFor::<T>::from(BLOCK_INCLUSION_PERIOD))
            },
            _ => false,
        }
    }
}
