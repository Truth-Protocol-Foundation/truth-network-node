use super::*;
use prediction_market_primitives::{test_helper::TestAccount, types::SignatureTest};
use sp_avn_common::Proof;
use sp_core::Pair;
use test_case::test_case;

fn create_signed_exit_proof(
    who: &TestAccount,
    market_id: &MarketId,
    pool_shares: &BalanceOf<Runtime>,
    min_amounts_out: &Vec<BalanceOf<Runtime>>,
) -> Proof<SignatureTest, TestAccountIdPK> {
    let relayer = eve();
    let block_number = System::block_number();
    let encoded_payload = NeoSwaps::encode_signed_exit_params(
        &relayer,
        market_id,
        pool_shares,
        min_amounts_out,
        &block_number,
    );

    let signature = SignatureTest::from(who.key_pair().sign(&encoded_payload));
    let proof = Proof { signer: who.key_pair().public(), relayer, signature };

    proof
}

struct SignedExitContext {
    pub pool_balances: Vec<u128>,
    pub spot_prices: Vec<BalanceOf<Runtime>>,
    pub liquidity: u128,
    pub market_id: MarketId,
    pub pool_shares_amount: u128,
    // pub category_count: u16,
    pub outcomes: Vec<AssetOf<Runtime>>,
}

impl Default for SignedExitContext {
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
            pool_balances: vec![100_000_000_000, 10_175_559_822],
            spot_prices,
            liquidity,
            market_id,
            pool_shares_amount: _4, // Remove 40% to the pool.
            outcomes: vec![],
        }
    }
}

impl SignedExitContext {
    fn setup_market(&mut self, market_status: &MarketStatus) {
        // Add a second LP to create a more generic situation, bringing the total of shares to _10.
        deposit_complete_set(self.market_id, bob(), self.liquidity);
        assert_ok!(NeoSwaps::join(
            RuntimeOrigin::signed(bob()),
            self.market_id,
            self.liquidity,
            vec![u128::MAX, u128::MAX],
        ));
        MarketCommons::mutate_market(&self.market_id, |market| {
            market.status = *market_status;
            Ok(())
        })
        .unwrap();
        let pool = Pools::<Runtime>::get(self.market_id).unwrap();
        self.outcomes = pool.assets();

        let alice_balances = [0, 44_912_220_089];
        assert_balances!(alice(), self.outcomes, alice_balances);
        assert_pool_state!(
            self.market_id,
            self.pool_balances,
            self.spot_prices,
            55_811_062_642,
            create_b_tree_map!({ alice() => _5, bob() => _5 }),
            0,
        );
    }
}

#[test_case(MarketStatus::Active, vec![39_960_000_000, 4_066_153_704], 33_508_962_010)]
#[test_case(MarketStatus::Resolved, vec![40_000_000_000, 4_070_223_928], 33_486_637_585)]
fn signed_exit_works(
    market_status: MarketStatus,
    amounts_out: Vec<BalanceOf<Runtime>>,
    new_liquidity_parameter: BalanceOf<Runtime>,
) {
    ExtBuilder::default().build().execute_with(|| {
        let mut context = SignedExitContext::default();
        let spot_prices = context.spot_prices.clone();
        let alice_account = TestAccount::new([0; 32]);

        let market_id = context.market_id;

        context.setup_market(&market_status);

        let alice_balances = [0, 44_912_220_089];

        let proof_blocknumber = System::block_number();
        let min_amounts_out = vec![0, 0];
        let proof = create_signed_exit_proof(
            &alice_account,
            &market_id,
            &context.pool_shares_amount,
            &min_amounts_out,
        );

        assert_ok!(NeoSwaps::signed_exit(
            RuntimeOrigin::signed(alice()),
            proof,
            market_id,
            context.pool_shares_amount,
            min_amounts_out,
            proof_blocknumber
        ));

        let new_pool_balances = context
            .pool_balances
            .iter()
            .zip(amounts_out.iter())
            .map(|(b, a)| b - a)
            .collect::<Vec<_>>();

        let new_alice_balances = alice_balances
            .iter()
            .zip(amounts_out.iter())
            .map(|(b, a)| b + a)
            .collect::<Vec<_>>();

        assert_balances!(alice(), context.outcomes, new_alice_balances);
        assert_pool_state!(
            market_id,
            new_pool_balances,
            spot_prices,
            new_liquidity_parameter,
            create_b_tree_map!({ alice() => _1, bob() => _5 }),
            0,
        );
        let pool_shares_amount = context.pool_shares_amount;
        System::assert_last_event(
            Event::ExitExecuted {
                who: alice(),
                market_id,
                pool_shares_amount,
                amounts_out,
                new_liquidity_parameter,
            }
            .into(),
        );
    });
}

mod fails_when {
    use super::*;
    use test_case::test_case;

    #[test_case(MarketStatus::Active)]
    #[test_case(MarketStatus::Resolved)]
    fn proof_has_wrong_relayer(market_status: MarketStatus) {
        ExtBuilder::default().build().execute_with(|| {
            let mut context = SignedExitContext::default();
            let alice_account = TestAccount::new([0; 32]);

            let market_id = context.market_id;

            context.setup_market(&market_status);

            let proof_blocknumber = System::block_number();
            let min_amounts_out = vec![0, 0];
            let proof = Proof {
                relayer: dave(),
                ..create_signed_exit_proof(
                    &alice_account,
                    &market_id,
                    &context.pool_shares_amount,
                    &min_amounts_out,
                )
            };

            assert_noop!(
                NeoSwaps::signed_exit(
                    RuntimeOrigin::signed(alice()),
                    proof,
                    market_id,
                    context.pool_shares_amount,
                    min_amounts_out,
                    proof_blocknumber
                ),
                Error::<Runtime>::UnauthorizedSignedTransaction
            );
        });
    }

    #[test_case(MarketStatus::Active)]
    #[test_case(MarketStatus::Resolved)]
    fn proof_data_mismatch_signature(market_status: MarketStatus) {
        ExtBuilder::default().build().execute_with(|| {
            let mut context = SignedExitContext::default();
            let alice_account = TestAccount::new([0; 32]);

            let market_id = context.market_id;

            context.setup_market(&market_status);

            let proof_blocknumber = System::block_number();
            let min_amounts_out = vec![0, 0];
            let proof = Proof {
                signature: alice_account.key_pair().sign(&[1u8; 10]),
                ..create_signed_exit_proof(
                    &alice_account,
                    &market_id,
                    &context.pool_shares_amount,
                    &min_amounts_out,
                )
            };

            assert_noop!(
                NeoSwaps::signed_exit(
                    RuntimeOrigin::signed(alice()),
                    proof,
                    market_id,
                    context.pool_shares_amount,
                    min_amounts_out,
                    proof_blocknumber
                ),
                Error::<Runtime>::UnauthorizedSignedTransaction
            );
        });
    }

    #[test_case(MarketStatus::Active)]
    #[test_case(MarketStatus::Resolved)]
    fn proof_data_mismatch_signer(market_status: MarketStatus) {
        ExtBuilder::default().build().execute_with(|| {
            let mut context = SignedExitContext::default();
            let alice_account = TestAccount::new([0; 32]);

            let market_id = context.market_id;

            context.setup_market(&market_status);

            let proof_blocknumber = System::block_number();
            let min_amounts_out = vec![0, 0];
            let bad_proof = Proof {
                signer: dave(),
                ..create_signed_exit_proof(
                    &alice_account,
                    &market_id,
                    &context.pool_shares_amount,
                    &min_amounts_out,
                )
            };

            assert_noop!(
                NeoSwaps::signed_exit(
                    RuntimeOrigin::signed(alice()),
                    bad_proof,
                    market_id,
                    context.pool_shares_amount,
                    min_amounts_out,
                    proof_blocknumber
                ),
                Error::<Runtime>::SenderIsNotSigner
            );
        });
    }

    #[test_case(MarketStatus::Active)]
    #[test_case(MarketStatus::Resolved)]
    fn proof_has_expired(market_status: MarketStatus) {
        ExtBuilder::default().build().execute_with(|| {
            let mut context = SignedExitContext::default();
            let alice_account = TestAccount::new([0; 32]);

            let market_id = context.market_id;

            context.setup_market(&market_status);

            let proof_blocknumber = System::block_number();
            let min_amounts_out = vec![0, 0];
            let proof = create_signed_exit_proof(
                &alice_account,
                &market_id,
                &context.pool_shares_amount,
                &min_amounts_out,
            );

            System::set_block_number(proof_blocknumber + 100);
            assert_noop!(
                NeoSwaps::signed_exit(
                    RuntimeOrigin::signed(alice()),
                    proof,
                    market_id,
                    context.pool_shares_amount,
                    min_amounts_out,
                    proof_blocknumber
                ),
                Error::<Runtime>::SignedTransactionExpired
            );
        });
    }
}
