use crate::*;
use prediction_market_primitives::math::fixed::FixedMulDiv;
use sp_runtime::SaturatedConversion;
impl<T: Config> Pallet<T> {
    pub fn get_total_reward(
        oldest_period: &RewardPeriodIndex,
    ) -> Result<BalanceOf<T>, DispatchError> {
        let total_reward = RewardPot::<T>::get(oldest_period)
            .map(|reward_pot| reward_pot.total_reward)
            .unwrap_or_else(|| RewardAmount::<T>::get());

        ensure!(
            Self::reward_pot_balance().ge(&BalanceOf::<T>::from(total_reward)),
            Error::<T>::InsufficientBalanceForReward
        );

        Ok(total_reward)
    }

    pub fn calculate_reward(
        uptime: u64,
        total_uptime: &u64,
        total_reward: &BalanceOf<T>,
    ) -> Result<BalanceOf<T>, DispatchError> {
        let uptime_balance: BalanceOf<T> = uptime.saturated_into::<BalanceOf<T>>();
        let total_uptime_balance: BalanceOf<T> = (*total_uptime).saturated_into::<BalanceOf<T>>();
        total_reward.bmul_bdiv(uptime_balance, total_uptime_balance)
    }

    pub fn pay_reward(
        period: &RewardPeriodIndex,
        node: NodeId<T>,
        amount: BalanceOf<T>,
    ) -> DispatchResult {
        let node_owner = match <NodeRegistry<T>>::get(&node) {
            Some(info) => info.owner,
            None => {
                log::error!("💔 Error paying reward. Node not found in registry. Reward period: {:?}, Node {:?}, Amount: {:?}",
                  period, node, amount
                );

                Self::deposit_event(Event::ErrorPayingReward {
                    reward_period: *period,
                    node: node.clone(),
                    amount,
                    error: Error::<T>::NodeOwnerNotFound.into(),
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
}
