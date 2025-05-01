// Copyright 2022-2024 Forecasting Technologies LTD.
// Copyright 2021-2022 Zeitgeist PM LLC.
// Copyright 2019-2020 Parity Technologies (UK) Ltd.
//
// This file is part of Zeitgeist.
//
// Zeitgeist is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at
// your option) any later version.
//
// Zeitgeist is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Zeitgeist. If not, see <https://www.gnu.org/licenses/>.

#[macro_export]
macro_rules! impl_fee_types {
    () => {
        use frame_support::traits::{fungibles::Credit, tokens::fungibles::Balanced};
        use sp_runtime::traits::AccountIdConversion;
        pub struct DealWithForeignFees;

        impl OnUnbalanced<Credit<AccountId, Tokens>> for DealWithForeignFees {
            fn on_unbalanced(fees_and_tips: Credit<AccountId, Tokens>) {
                // We have to manage the mint / burn ratio on the BASE chain,
                // but we do not have the responsibility and necessary knowledge to
                // manage the mint / burn ratio for any other chain.
                // Thus we should keep 100% of the foreign tokens in the treasury.
                // Handle the split imbalances
                // on_unbalanced is not implemented for other currencies than the native currency
                // https://github.com/paritytech/substrate/blob/85415fb3a452dba12ff564e6b093048eed4c5aad/frame/treasury/src/lib.rs#L618-L627
                // https://github.com/paritytech/substrate/blob/5ea6d95309aaccfa399c5f72e5a14a4b7c6c4ca1/frame/treasury/src/lib.rs#L490
                let res = <Tokens as Balanced<AccountId>>::resolve(
                    &TreasuryPalletId::get().into_account_truncating(),
                    fees_and_tips,
                );
                debug_assert!(res.is_ok());
            }
        }
    };
}

#[macro_export]
macro_rules! impl_market_creator_fees {
    () => {
        use orml_traits::MultiCurrency;
        use prediction_market_primitives::traits::{DistributeFees, MarketCommonsPalletApi};
        use sp_runtime::{DispatchError, SaturatedConversion};

        pub struct MarketCreatorFee;

        /// Uses the `creator_fee` field defined by the specified market to deduct a fee for the
        /// market's creator. Calling `distribute` is noop if the market doesn't exist or the
        /// transfer fails for any reason.
        impl DistributeFees for MarketCreatorFee {
            type Asset = Asset<MarketId>;
            type AccountId = AccountId;
            type Balance = Balance;
            type MarketId = MarketId;

            fn distribute(
                market_id: Self::MarketId,
                asset: Self::Asset,
                account: &Self::AccountId,
                amount: Self::Balance,
            ) -> Self::Balance {
                Self::do_distribute(market_id, asset, account, amount)
                    .unwrap_or_else(|_| 0u8.saturated_into())
            }

            fn fee_percentage(market_id: Self::MarketId) -> Perbill {
                Self::fee_percentage(market_id).unwrap_or(Perbill::zero())
            }
        }

        impl MarketCreatorFee {
            fn do_distribute(
                market_id: MarketId,
                asset: Asset<MarketId>,
                account: &AccountId,
                amount: Balance,
            ) -> Result<Balance, DispatchError> {
                let market = MarketCommons::market(&market_id)?;
                let fee_amount = Self::fee_percentage(market_id)?.mul_floor(amount);
                // Might fail if the transaction is too small
                <AssetManager as MultiCurrency<_>>::transfer(
                    asset,
                    account,
                    &market.creator,
                    fee_amount,
                )?;
                Ok(fee_amount)
            }

            fn fee_percentage(market_id: MarketId) -> Result<Perbill, DispatchError> {
                let market = MarketCommons::market(&market_id)?;
                Ok(market.creator_fee)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_winner_fees {
    () => {
        pub struct WinnerFee;

        /// Uses the Vault account to deduct a fee from the winner's payout.
        /// Calling `distribute` is noop if the market doesn't exist or the
        /// transfer fails for any reason.
        impl DistributeFees for WinnerFee {
            type Asset = Asset<MarketId>;
            type AccountId = AccountId;
            type Balance = Balance;
            type MarketId = MarketId;

            fn distribute(
                market_id: Self::MarketId,
                asset: Self::Asset,
                account: &Self::AccountId,
                amount: Self::Balance,
            ) -> Self::Balance {
                Self::do_distribute(market_id, asset, account, amount)
                    .unwrap_or_else(|_| 0u8.saturated_into())
            }

            fn fee_percentage(market_id: Self::MarketId) -> Perbill {
                Self::fee_percentage(market_id).unwrap_or(Perbill::zero())
            }
        }

        impl WinnerFee {
            fn do_distribute(
                market_id: MarketId,
                asset: Asset<MarketId>,
                account: &AccountId,
                amount: Balance,
            ) -> Result<Balance, DispatchError> {
                let recipient = PredictionMarkets::vault_account()?;
                let fee_amount = Self::fee_percentage(market_id)?.mul_floor(amount);
                // Might fail if the transaction is too small
                <AssetManager as MultiCurrency<_>>::transfer(
                    asset, account, &recipient, fee_amount,
                )?;
                Ok(fee_amount)
            }

            fn fee_percentage(_market_id: MarketId) -> Result<Perbill, DispatchError> {
                Ok(WinnerFeePercentage::get())
            }
        }
    };
}
