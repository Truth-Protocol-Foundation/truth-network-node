use crate::*;
use sp_runtime::{ArithmeticError, Perquintill, SaturatedConversion};
impl<T: Config> Pallet<T> {
    // Nodes should not be able to submit over the min uptime required.
    // but we still check it here to be sure.
    pub fn calculate_node_uptime(
        node_id: &NodeId<T>,
        actual_uptime: u64,
        uptime_threshold: u32,
    ) -> u64 {
        let uptime_threshold_u64 = uptime_threshold as u64;
        if actual_uptime >= uptime_threshold_u64 {
            if actual_uptime > uptime_threshold_u64 {
                log::warn!("✋ Node ({:?}) has been up for more than the expected uptime. Actual: {:?}, Expected: {:?}", node_id, actual_uptime, uptime_threshold);
            }
            uptime_threshold_u64
        } else {
            actual_uptime
        }
    }

    /// The node's share of `total_reward`, rounded down
    pub fn calculate_reward(
        uptime: u64,
        total_uptime: &u64,
        total_reward: &BalanceOf<T>,
    ) -> Result<BalanceOf<T>, DispatchError> {
        if total_uptime.is_zero() {
            return Err(DispatchError::Arithmetic(ArithmeticError::DivisionByZero));
        }

        let ratio = Perquintill::from_rational(uptime, *total_uptime);
        let total_reward_u128: u128 = (*total_reward).saturated_into();
        Ok(ratio.mul_floor(total_reward_u128).saturated_into())
    }

    pub fn pay_reward(
        period: &RewardPeriodIndex,
        node: NodeId<T>,
        amount: BalanceOf<T>,
    ) -> DispatchResult {
        let node_owner = match <NodeRegistry<T>>::get(&node) {
            Some(info) => info.owner,
            None => {
                log::warn!("⚠️ Error paying reward. Node not found in registry. Reward period: {:?}, Node {:?}, Amount: {:?}",
                  period, node, amount
                );

                Self::deposit_event(Event::ErrorPayingReward {
                    reward_period: *period,
                    node: node.clone(),
                    amount,
                    error: Error::<T>::NodeNotRegistered.into(),
                });
                // We skip paying rewards for this node and continue without erroring
                return Ok(());
            },
        };

        let reward_pot_account_id = Self::compute_reward_account_id();

        T::Currency::transfer(
            &reward_pot_account_id,
            &node_owner,
            amount,
            ExistenceRequirement::KeepAlive,
        )?;

        Self::deposit_event(Event::RewardPaid {
            reward_period: *period,
            owner: node_owner,
            node,
            amount,
        });

        Ok(())
    }

    pub fn remove_paid_nodes(
        period_index: RewardPeriodIndex,
        paid_nodes_to_remove: &Vec<T::AccountId>,
    ) {
        // Remove the paid nodes. We do this separatly to avoid changing the map while iterating
        // it
        for node in paid_nodes_to_remove {
            NodeUptime::<T>::remove(period_index, node);
        }
    }

    pub fn complete_reward_payout(period_index: RewardPeriodIndex) {
        // Anything left unpaid stays in the pot for later periods
        if let Some(reward_pot) = RewardPot::<T>::get(period_index) {
            OutstandingRewardToPay::<T>::mutate(|outstanding| {
                *outstanding = outstanding.saturating_sub(reward_pot.total_reward);
            });
        }

        // We finished paying all nodes for this period
        OldestUnpaidRewardPeriodIndex::<T>::put(period_index.saturating_add(1));
        LastPaidPointer::<T>::kill();
        <TotalUptime<T>>::remove(period_index);
        <RewardPot<T>>::remove(period_index);
        Self::deposit_event(Event::RewardPayoutCompleted { reward_period_index: period_index });
    }

    pub fn update_last_paid_pointer(
        period_index: RewardPeriodIndex,
        last_node_paid: Option<T::AccountId>,
    ) {
        if let Some(node) = last_node_paid {
            LastPaidPointer::<T>::put(PaymentPointer { period_index, node });
        }
    }

    /// The account ID of the reward pot.
    pub fn compute_reward_account_id() -> T::AccountId {
        T::RewardPotId::get().into_account_truncating()
    }

    /// The total amount of funds stored in this pallet
    pub fn reward_pot_balance() -> BalanceOf<T> {
        // Must never be less than 0 but better be safe.
        <T as pallet::Config>::Currency::free_balance(&Self::compute_reward_account_id())
            .saturating_sub(<T as pallet::Config>::Currency::minimum_balance())
    }

    pub fn get_iterator_from_last_paid(
        oldest_period: RewardPeriodIndex,
        last_paid_pointer: PaymentPointer<T::AccountId>,
    ) -> Result<PrefixIterator<(T::AccountId, UptimeInfo<BlockNumberFor<T>>)>, DispatchError> {
        ensure!(last_paid_pointer.period_index == oldest_period, Error::<T>::InvalidPeriodPointer);
        // Make sure the last paid node has been remove, to be extra sure we won't double pay
        ensure!(
            !NodeUptime::<T>::contains_key(oldest_period, &last_paid_pointer.node),
            Error::<T>::InvalidNodePointer
        );

        // Start iteration just after `(oldest_period, last_paid_pointer.node)`.
        let final_key = last_paid_pointer.get_final_key::<T>();
        Ok(NodeUptime::<T>::iter_prefix_from(oldest_period, final_key))
    }

    /// Get the current time in seconds
    pub fn time_now_sec() -> Duration {
        T::TimeProvider::now().as_secs()
    }

    /// `period`'s reward pot, if it is funded and its update window has closed
    pub fn get_payable_reward_pot(
        period: RewardPeriodIndex,
    ) -> Result<RewardPotInfo<BalanceOf<T>>, Error<T>> {
        let reward_pot = RewardPot::<T>::get(period).ok_or(Error::<T>::RewardPotNotFound)?;
        ensure!(reward_pot.funded, Error::<T>::RewardPeriodNotFunded);
        ensure!(
            !reward_pot.update_window_open(Self::time_now_sec()),
            Error::<T>::RewardUpdateWindowOpen
        );
        Ok(reward_pot)
    }

    /// Set `amount` (which may be zero) as the ended `period`'s reward total.
    ///
    /// Allowed while the period is unfunded, or funded and still inside its update window.
    /// The pot must already hold `amount` on top of `OutstandingRewardToPay`. No currency
    /// moves here.
    pub(crate) fn do_set_reward_amount(
        period: RewardPeriodIndex,
        amount: BalanceOf<T>,
    ) -> DispatchResult {
        ensure!(amount <= T::MaxRewardPerPeriod::get(), Error::<T>::RewardExceedsMax);

        let mut pot_info = RewardPot::<T>::get(period).ok_or(Error::<T>::RewardPotNotFound)?;
        ensure!(
            pot_info.can_update_amount(Self::time_now_sec()),
            Error::<T>::RewardUpdateWindowClosed
        );

        let outstanding_without_period =
            OutstandingRewardToPay::<T>::get().saturating_sub(pot_info.total_reward);
        ensure!(
            Self::reward_pot_balance() >= outstanding_without_period.saturating_add(amount),
            Error::<T>::InsufficientPotBalance
        );

        pot_info.total_reward = amount;
        pot_info.funded = true;
        RewardPot::<T>::insert(period, pot_info);
        OutstandingRewardToPay::<T>::put(outstanding_without_period.saturating_add(amount));

        Self::deposit_event(Event::RewardAmountSet { period, amount });
        Ok(())
    }
}
