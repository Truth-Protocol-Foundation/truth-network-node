use super::*;
use pallet_prediction_markets::DispatchResultWithPostInfo;
use prediction_market_primitives::{test_helper::TestAccount, types::SignatureTest};
use sp_avn_common::Proof;
use sp_core::Pair;

struct SignedWithdrawContext {
    pub pool_balances: [u128; 2],
    pub spot_prices: Vec<BalanceOf<Runtime>>,
    pub liquidity_parameter: u128,
    pub market_id: MarketId,
    pub category_count: u16,
}
impl Default for SignedWithdrawContext {
    fn default() -> Self {
        let spot_prices = vec![_3_4, _1_4];
        let category_count = 2;
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Categorical(category_count),
            _10,
            spot_prices.clone(),
            CENT_BASE,
        );
        Self {
            pool_balances: [83_007_499_856, 400_000_000_000],
            spot_prices,
            liquidity_parameter: 288_539_008_176,
            market_id,
            category_count,
        }
    }
}

impl SignedWithdrawContext {
    fn create_signed_withdraw_proof(
        &self,
        who: &TestAccount,
    ) -> Proof<SignatureTest, TestAccountIdPK> {
        let relayer = eve();
        let block_number = System::block_number();
        let encoded_payload =
            NeoSwaps::encode_signed_withdraw_fees_params(&relayer, &self.market_id, &block_number);

        let signature = SignatureTest::from(who.key_pair().sign(&encoded_payload));
        let proof = Proof { signer: who.key_pair().public(), relayer, signature };

        proof
    }

    fn test_signed_withdraw(
        &self,
        who: AccountIdOf<Runtime>,
        fees_withdrawn: BalanceOf<Runtime>,
        fees_remaining: BalanceOf<Runtime>,
        proof: Proof<SignatureTest, TestAccountIdPK>,
    ) -> DispatchResultWithPostInfo {
        let block_number = System::block_number();
        self.test_signed_withdraw_with_block_number(
            who,
            fees_withdrawn,
            fees_remaining,
            proof,
            block_number,
        )
    }

    fn test_signed_withdraw_with_block_number(
        &self,
        who: AccountIdOf<Runtime>,
        fees_withdrawn: BalanceOf<Runtime>,
        fees_remaining: BalanceOf<Runtime>,
        proof: Proof<SignatureTest, TestAccountIdPK>,
        block_number: u32,
    ) -> DispatchResultWithPostInfo {
        let old_balance = <Runtime as Config>::MultiCurrency::free_balance(BASE_ASSET, &who);

        NeoSwaps::signed_withdraw_fees(
            RuntimeOrigin::signed(who),
            proof,
            self.market_id,
            block_number,
        )?;
        assert_balance!(&who, BASE_ASSET, old_balance + fees_withdrawn);
        assert_pool_state!(
            self.market_id,
            self.pool_balances,
            self.spot_prices,
            self.liquidity_parameter,
            create_b_tree_map!({ alice() => _10, bob() => _10, charlie() => _20 }),
            fees_remaining,
        );
        System::assert_last_event(
            Event::FeesWithdrawn { who, market_id: self.market_id, amount: fees_withdrawn }.into(),
        );
        Ok(().into())
    }

    fn join(&self, who: AccountIdOf<Runtime>, amount: BalanceOf<Runtime>) {
        // Adding a little more to ensure that rounding doesn't cause issues.
        deposit_complete_set(self.market_id, who, amount + CENT_BASE);
        assert_ok!(NeoSwaps::join(
            RuntimeOrigin::signed(who),
            self.market_id,
            amount,
            vec![u128::MAX; self.category_count as usize],
        ));
    }
}

fn deposit(who: AccountIdOf<Runtime>) {
    // Make sure everybody's got at least the minimum deposit.
    assert_ok!(<Runtime as Config>::MultiCurrency::deposit(
        BASE_ASSET,
        &who,
        <Runtime as Config>::MultiCurrency::minimum_balance(BASE_ASSET)
    ));
}

#[test]
fn signed_withdraw_fees_works() {
    // Verify that fees are correctly distributed among LPs.
    ExtBuilder::default().build().execute_with(|| {
        let context = SignedWithdrawContext::default();

        context.join(bob(), _10);
        context.join(charlie(), _20);

        // Mock up some fees.
        let mut pool = Pools::<Runtime>::get(context.market_id).unwrap();
        let fee_amount = _1;
        assert_ok!(AssetManager::deposit(pool.collateral, &pool.account_id, fee_amount));
        assert_ok!(pool.liquidity_shares_manager.deposit_fees(fee_amount));
        Pools::<Runtime>::insert(context.market_id, pool.clone());

        // Alice seed is 0
        let alice = TestAccount::new([0; 32]);
        deposit(alice.account_id());
        assert_ok!(context.test_signed_withdraw(
            alice.account_id(),
            _1_4,
            _3_4,
            context.create_signed_withdraw_proof(&alice),
        ));

        // Bob seed is 1
        let bob = TestAccount::new([1; 32]);
        deposit(bob.account_id());
        assert_ok!(context.test_signed_withdraw(
            bob.account_id(),
            _1_4,
            _1_2,
            context.create_signed_withdraw_proof(&bob),
        ));
        // Charlie seed is 2
        let charlie = TestAccount::new([2; 32]);
        deposit(charlie.account_id());
        assert_ok!(context.test_signed_withdraw(
            charlie.account_id(),
            _1_2,
            0,
            context.create_signed_withdraw_proof(&charlie),
        ));
    });
}

mod fails_when {
    use super::*;
    #[test]
    fn proof_has_wrong_relayer() {
        // Verify that fees are correctly distributed among LPs.
        ExtBuilder::default().build().execute_with(|| {
            let context = SignedWithdrawContext::default();

            context.join(bob(), _10);
            context.join(charlie(), _20);

            // Mock up some fees.
            let mut pool = Pools::<Runtime>::get(context.market_id).unwrap();
            let fee_amount = _1;
            assert_ok!(AssetManager::deposit(pool.collateral, &pool.account_id, fee_amount));
            assert_ok!(pool.liquidity_shares_manager.deposit_fees(fee_amount));
            Pools::<Runtime>::insert(context.market_id, pool.clone());

            // Alice seed is 0
            let alice = TestAccount::new([0; 32]);
            deposit(alice.account_id());
            let bad_proof =
                Proof { relayer: dave(), ..context.create_signed_withdraw_proof(&alice) };
            assert_noop!(
                context.test_signed_withdraw(alice.account_id(), _1_4, _3_4, bad_proof),
                Error::<Runtime>::UnauthorizedSignedTransaction
            );
        });
    }

    #[test]
    fn proof_data_mismatch_relayer() {
        // Verify that fees are correctly distributed among LPs.
        ExtBuilder::default().build().execute_with(|| {
            let context = SignedWithdrawContext::default();

            context.join(bob(), _10);
            context.join(charlie(), _20);

            // Mock up some fees.
            let mut pool = Pools::<Runtime>::get(context.market_id).unwrap();
            let fee_amount = _1;
            assert_ok!(AssetManager::deposit(pool.collateral, &pool.account_id, fee_amount));
            assert_ok!(pool.liquidity_shares_manager.deposit_fees(fee_amount));
            Pools::<Runtime>::insert(context.market_id, pool.clone());

            // Alice seed is 0
            let alice = TestAccount::new([0; 32]);
            deposit(alice.account_id());
            let bad_proof =
                Proof { relayer: bob(), ..context.create_signed_withdraw_proof(&alice) };
            assert_noop!(
                context.test_signed_withdraw(alice.account_id(), _1_4, _3_4, bad_proof),
                Error::<Runtime>::UnauthorizedSignedTransaction
            );
        });
    }

    #[test]
    fn proof_data_mismatch_signature() {
        // Verify that fees are correctly distributed among LPs.
        ExtBuilder::default().build().execute_with(|| {
            let context = SignedWithdrawContext::default();

            context.join(bob(), _10);
            context.join(charlie(), _20);

            // Mock up some fees.
            let mut pool = Pools::<Runtime>::get(context.market_id).unwrap();
            let fee_amount = _1;
            assert_ok!(AssetManager::deposit(pool.collateral, &pool.account_id, fee_amount));
            assert_ok!(pool.liquidity_shares_manager.deposit_fees(fee_amount));
            Pools::<Runtime>::insert(context.market_id, pool.clone());

            // Alice seed is 0
            let alice = TestAccount::new([0; 32]);
            deposit(alice.account_id());
            let bad_proof = Proof {
                signature: alice.key_pair().sign(&[1u8; 10]),
                ..context.create_signed_withdraw_proof(&alice)
            };
            assert_noop!(
                context.test_signed_withdraw(alice.account_id(), _1_4, _3_4, bad_proof),
                Error::<Runtime>::UnauthorizedSignedTransaction
            );
        });
    }

    #[test]
    fn proof_data_mismatch_signer() {
        // Verify that fees are correctly distributed among LPs.
        ExtBuilder::default().build().execute_with(|| {
            let context = SignedWithdrawContext::default();

            context.join(bob(), _10);
            context.join(charlie(), _20);

            // Mock up some fees.
            let mut pool = Pools::<Runtime>::get(context.market_id).unwrap();
            let fee_amount = _1;
            assert_ok!(AssetManager::deposit(pool.collateral, &pool.account_id, fee_amount));
            assert_ok!(pool.liquidity_shares_manager.deposit_fees(fee_amount));
            Pools::<Runtime>::insert(context.market_id, pool.clone());

            // Alice seed is 0
            let alice = TestAccount::new([0; 32]);
            deposit(alice.account_id());
            let bad_proof =
                Proof { signer: dave(), ..context.create_signed_withdraw_proof(&alice) };
            assert_noop!(
                context.test_signed_withdraw(alice.account_id(), _1_4, _3_4, bad_proof),
                Error::<Runtime>::SenderIsNotSigner
            );
        });
    }

    #[test]
    fn proof_has_expired() {
        // Verify that fees are correctly distributed among LPs.
        ExtBuilder::default().build().execute_with(|| {
            let context = SignedWithdrawContext::default();

            context.join(bob(), _10);
            context.join(charlie(), _20);

            // Mock up some fees.
            let mut pool = Pools::<Runtime>::get(context.market_id).unwrap();
            let fee_amount = _1;
            assert_ok!(AssetManager::deposit(pool.collateral, &pool.account_id, fee_amount));
            assert_ok!(pool.liquidity_shares_manager.deposit_fees(fee_amount));
            Pools::<Runtime>::insert(context.market_id, pool.clone());

            // Alice seed is 0
            let alice = TestAccount::new([0; 32]);
            deposit(alice.account_id());

            let proof_blocknumber = System::block_number();
            let proof = context.create_signed_withdraw_proof(&alice);
            System::set_block_number(proof_blocknumber + 100);

            assert_noop!(
                context.test_signed_withdraw_with_block_number(
                    alice.account_id(),
                    _1_4,
                    _3_4,
                    proof,
                    proof_blocknumber
                ),
                Error::<Runtime>::SignedTransactionExpired
            );
        });
    }
}
