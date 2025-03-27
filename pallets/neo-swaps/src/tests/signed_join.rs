use super::*;
use prediction_market_primitives::{test_helper::TestAccount, types::SignatureTest};
use sp_avn_common::Proof;
use sp_core::Pair;

fn create_signed_join_proof(
    who: &TestAccount,
    market_id: &MarketId,
    pool_shares: &BalanceOf<Runtime>,
    max_amounts_in: &Vec<BalanceOf<Runtime>>,
) -> Proof<SignatureTest, TestAccountIdPK> {
    let relayer = eve();
    let block_number = System::block_number();
    let encoded_payload = NeoSwaps::encode_signed_join_params(
        &relayer,
        market_id,
        pool_shares,
        max_amounts_in,
        &block_number,
    );

    let signature = SignatureTest::from(who.key_pair().sign(&encoded_payload));
    let proof = Proof { signer: who.key_pair().public(), relayer, signature };

    proof
}

struct SignedJoinContext {
    pub pool_balances: Vec<u128>,
    pub spot_prices: Vec<BalanceOf<Runtime>>,
    pub liquidity: u128,
    pub market_id: MarketId,
    pub pool_shares_amount: u128,
    pub category_count: u16,
    pub outcomes: Vec<AssetOf<Runtime>>,
}

impl Default for SignedJoinContext {
    fn default() -> Self {
        let liquidity = _5;
        let spot_prices = vec![_1_6, _5_6 + 1];
        let market_id = create_market_and_deploy_pool(
            alice(),
            BASE_ASSET,
            MarketType::Scalar(0..=1),
            liquidity,
            spot_prices.clone(),
            CENT_BASE,
        );

        Self {
            // These need to match the actual pool balances after deployment
            pool_balances: vec![50_000_000_000, 5_087_779_911],
            spot_prices,
            liquidity,
            market_id,
            pool_shares_amount: _4, // Add 40% to the pool
            category_count: 2,
            outcomes: vec![],
        }
    }
}

impl SignedJoinContext {
    fn setup_market(&mut self) {
        // Make sure the market is Active for pool operations to work
        MarketCommons::mutate_market(&self.market_id, |market| {
            market.status = MarketStatus::Active;
            Ok(())
        })
        .unwrap();
        
        let pool = Pools::<Runtime>::get(self.market_id).unwrap();
        self.outcomes = pool.assets();
        
        // Verify the initial pool state matches our expected values
        assert_pool_state!(
            self.market_id,
            self.pool_balances,
            self.spot_prices,
            27_905_531_321,  // Updated liquidity parameter value
            create_b_tree_map!({ alice() => _5 }),
            0,
        );
    }
    
    fn prepare_outcome_tokens(&self, who: &TestAccountIdPK, amount: BalanceOf<Runtime>) {
        // Acquire outcome tokens for the test account to deposit into the pool
        deposit_complete_set(self.market_id, *who, amount);
    }
}

#[test]
fn signed_join_works() {
    ExtBuilder::default().build().execute_with(|| {
        let mut context = SignedJoinContext::default();
        let bob_account = TestAccount::new([1; 32]);

        let market_id = context.market_id;
        let pool_shares_amount = _4;
        let expected_liquidity_parameter = 50_229_956_378;
        
        context.setup_market();
        
        // Prepare outcome tokens for Bob to join the pool
        context.prepare_outcome_tokens(&bob(), pool_shares_amount * 2);
        
        // Calculate expected amounts in based on pool state
        // Max amounts set high to ensure the test passes
        let max_amounts_in = vec![u128::MAX, u128::MAX];
        
        let proof_blocknumber = System::block_number();
        let proof = create_signed_join_proof(
            &bob_account,
            &market_id,
            &pool_shares_amount,
            &max_amounts_in,
        );

        // Initial balances to verify later
        let bob_initial_balances = [
            <Runtime as Config>::MultiCurrency::free_balance(context.outcomes[0], &bob()),
            <Runtime as Config>::MultiCurrency::free_balance(context.outcomes[1], &bob()),
        ];

        // Execute signed join
        assert_ok!(NeoSwaps::signed_join(
            RuntimeOrigin::signed(bob()),
            proof,
            market_id,
            pool_shares_amount,
            max_amounts_in,
            proof_blocknumber
        ));

        // Get the pool to verify state changes
        let pool = Pools::<Runtime>::get(market_id).unwrap();
        
        // Check that Bob's outcome tokens were reduced
        let bob_final_balances = [
            <Runtime as Config>::MultiCurrency::free_balance(context.outcomes[0], &bob()),
            <Runtime as Config>::MultiCurrency::free_balance(context.outcomes[1], &bob()),
        ];
        
        // Bob's balances should be less after joining
        assert!(bob_final_balances[0] < bob_initial_balances[0]);
        assert!(bob_final_balances[1] < bob_initial_balances[1]);

        // Verify pool state changes
        assert_ok!(pool.liquidity_shares_manager.shares_of(&bob()));
        assert_eq!(pool.liquidity_parameter, expected_liquidity_parameter);
        
        // Verify the event was emitted
        System::assert_has_event(
            Event::JoinExecuted {
                who: bob(),
                market_id,
                pool_shares_amount,
                amounts_in: vec![
                    bob_initial_balances[0] - bob_final_balances[0],
                    bob_initial_balances[1] - bob_final_balances[1],
                ],
                new_liquidity_parameter: expected_liquidity_parameter,
            }
            .into(),
        );
    });
}

mod fails_when {
    use super::*;

    #[test]
    fn proof_has_wrong_relayer() {
        ExtBuilder::default().build().execute_with(|| {
            let mut context = SignedJoinContext::default();
            let bob_account = TestAccount::new([1; 32]);

            let market_id = context.market_id;
            context.setup_market();
            
            context.prepare_outcome_tokens(&bob(), context.pool_shares_amount * 2);
            
            let max_amounts_in = vec![u128::MAX, u128::MAX];
            let proof_blocknumber = System::block_number();
            
            // Create proof with wrong relayer (dave instead of eve)
            let proof = Proof {
                relayer: dave(),
                ..create_signed_join_proof(
                    &bob_account,
                    &market_id,
                    &context.pool_shares_amount,
                    &max_amounts_in,
                )
            };

            assert_noop!(
                NeoSwaps::signed_join(
                    RuntimeOrigin::signed(bob()),
                    proof,
                    market_id,
                    context.pool_shares_amount,
                    max_amounts_in,
                    proof_blocknumber
                ),
                Error::<Runtime>::UnauthorizedSignedTransaction
            );
        });
    }

    #[test]
    fn proof_data_mismatch_signature() {
        ExtBuilder::default().build().execute_with(|| {
            let mut context = SignedJoinContext::default();
            let bob_account = TestAccount::new([1; 32]);

            let market_id = context.market_id;
            context.setup_market();
            
            context.prepare_outcome_tokens(&bob(), context.pool_shares_amount * 2);
            
            let max_amounts_in = vec![u128::MAX, u128::MAX];
            let proof_blocknumber = System::block_number();
            
            // Create proof with incorrect signature
            let proof = Proof {
                signature: bob_account.key_pair().sign(&[1u8; 10]),
                ..create_signed_join_proof(
                    &bob_account,
                    &market_id,
                    &context.pool_shares_amount,
                    &max_amounts_in,
                )
            };

            assert_noop!(
                NeoSwaps::signed_join(
                    RuntimeOrigin::signed(bob()),
                    proof,
                    market_id,
                    context.pool_shares_amount,
                    max_amounts_in,
                    proof_blocknumber
                ),
                Error::<Runtime>::UnauthorizedSignedTransaction
            );
        });
    }

    #[test]
    fn proof_data_mismatch_signer() {
        ExtBuilder::default().build().execute_with(|| {
            let mut context = SignedJoinContext::default();
            let bob_account = TestAccount::new([1; 32]);

            let market_id = context.market_id;
            context.setup_market();
            
            context.prepare_outcome_tokens(&bob(), context.pool_shares_amount * 2);
            
            let max_amounts_in = vec![u128::MAX, u128::MAX];
            let proof_blocknumber = System::block_number();
            
            // Create proof with wrong signer
            let proof = Proof {
                signer: dave(),
                ..create_signed_join_proof(
                    &bob_account,
                    &market_id,
                    &context.pool_shares_amount,
                    &max_amounts_in,
                )
            };

            assert_noop!(
                NeoSwaps::signed_join(
                    RuntimeOrigin::signed(bob()),
                    proof,
                    market_id,
                    context.pool_shares_amount,
                    max_amounts_in,
                    proof_blocknumber
                ),
                Error::<Runtime>::SenderIsNotSigner
            );
        });
    }

    #[test]
    fn proof_has_expired() {
        ExtBuilder::default().build().execute_with(|| {
            let mut context = SignedJoinContext::default();
            let bob_account = TestAccount::new([1; 32]);

            let market_id = context.market_id;
            context.setup_market();
            
            context.prepare_outcome_tokens(&bob(), context.pool_shares_amount * 2);
            
            let max_amounts_in = vec![u128::MAX, u128::MAX];
            let proof_blocknumber = System::block_number();
            
            let proof = create_signed_join_proof(
                &bob_account,
                &market_id,
                &context.pool_shares_amount,
                &max_amounts_in,
            );

            // Advance blocks to expire the transaction
            System::set_block_number(proof_blocknumber + 100);
            
            assert_noop!(
                NeoSwaps::signed_join(
                    RuntimeOrigin::signed(bob()),
                    proof,
                    market_id,
                    context.pool_shares_amount,
                    max_amounts_in,
                    proof_blocknumber
                ),
                Error::<Runtime>::SignedTransactionExpired
            );
        });
    }
    
    #[test]
    fn insufficient_outcome_tokens() {
        ExtBuilder::default().build().execute_with(|| {
            let mut context = SignedJoinContext::default();
            let bob_account = TestAccount::new([1; 32]);

            let market_id = context.market_id;
            context.setup_market();
            
            // Don't prepare enough outcome tokens (only prepare half of what's needed)
            context.prepare_outcome_tokens(&bob(), context.pool_shares_amount / 2);
            
            // Set max_amounts_in to very small values that will cause the test to fail
            let max_amounts_in = vec![1, 1];
            let proof_blocknumber = System::block_number();
            
            let proof = create_signed_join_proof(
                &bob_account,
                &market_id,
                &context.pool_shares_amount,
                &max_amounts_in,
            );

            assert_noop!(
                NeoSwaps::signed_join(
                    RuntimeOrigin::signed(bob()),
                    proof,
                    market_id,
                    context.pool_shares_amount,
                    max_amounts_in,
                    proof_blocknumber
                ),
                Error::<Runtime>::AmountInAboveMax
            );
        });
    }
    
    #[test]
    fn market_not_active() {
        ExtBuilder::default().build().execute_with(|| {
            let mut context = SignedJoinContext::default();
            let bob_account = TestAccount::new([1; 32]);

            let market_id = context.market_id;
            
            // First set up with Active status to ensure pool is properly initialized
            context.setup_market();
            
            // Prepare outcome tokens while the market is still active
            context.prepare_outcome_tokens(&bob(), context.pool_shares_amount * 2);
            
            // Then change market status to a non-active status
            MarketCommons::mutate_market(&market_id, |market| {
                market.status = MarketStatus::Disputed;
                Ok(())
            })
            .unwrap();
            
            let max_amounts_in = vec![u128::MAX, u128::MAX];
            let proof_blocknumber = System::block_number();
            
            let proof = create_signed_join_proof(
                &bob_account,
                &market_id,
                &context.pool_shares_amount,
                &max_amounts_in,
            );

            // Test that it fails with MarketNotActive error
            assert_noop!(
                NeoSwaps::signed_join(
                    RuntimeOrigin::signed(bob()),
                    proof,
                    market_id,
                    context.pool_shares_amount,
                    max_amounts_in,
                    proof_blocknumber
                ),
                Error::<Runtime>::MarketNotActive
            );
        });
    }
    
    #[test]
    fn zero_pool_shares_amount() {
        ExtBuilder::default().build().execute_with(|| {
            let mut context = SignedJoinContext::default();
            let bob_account = TestAccount::new([1; 32]);

            let market_id = context.market_id;
            context.setup_market();
            context.prepare_outcome_tokens(&bob(), context.pool_shares_amount * 2);
            
            let max_amounts_in = vec![u128::MAX, u128::MAX];
            let proof_blocknumber = System::block_number();
            
            // Create proof with zero pool shares
            let zero_pool_shares: BalanceOf<Runtime> = 0;
            let proof = create_signed_join_proof(
                &bob_account,
                &market_id,
                &zero_pool_shares,
                &max_amounts_in,
            );

            assert_noop!(
                NeoSwaps::signed_join(
                    RuntimeOrigin::signed(bob()),
                    proof,
                    market_id,
                    zero_pool_shares,
                    max_amounts_in,
                    proof_blocknumber
                ),
                Error::<Runtime>::ZeroAmount
            );
        });
    }
    
    #[test]
    fn position_too_small() {
        ExtBuilder::default().build().execute_with(|| {
            let mut context = SignedJoinContext::default();
            let bob_account = TestAccount::new([1; 32]);

            let market_id = context.market_id;
            context.setup_market();
            context.prepare_outcome_tokens(&bob(), context.pool_shares_amount * 2);
            
            let max_amounts_in = vec![u128::MAX, u128::MAX];
            let proof_blocknumber = System::block_number();
            
            // Create a very small position
            let tiny_position: BalanceOf<Runtime> = 1;
            let proof = create_signed_join_proof(
                &bob_account,
                &market_id,
                &tiny_position,
                &max_amounts_in,
            );

            assert_noop!(
                NeoSwaps::signed_join(
                    RuntimeOrigin::signed(bob()),
                    proof,
                    market_id,
                    tiny_position,
                    max_amounts_in,
                    proof_blocknumber
                ),
                Error::<Runtime>::MinRelativeLiquidityThresholdViolated
            );
        });
    }
}