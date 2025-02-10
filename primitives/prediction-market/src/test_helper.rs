#![cfg(any(feature = "mock", test))]
use crate::types::TestAccountIdPK;
use parity_scale_codec::Decode;
use sp_core::{sr25519, Pair};

pub struct TestAccount {
    pub seed: [u8; 32],
}

impl TestAccount {
    pub fn new(seed: [u8; 32]) -> Self {
        TestAccount { seed }
    }

    pub fn account_id(&self) -> TestAccountIdPK {
        return TestAccountIdPK::decode(&mut self.key_pair().public().to_vec().as_slice()).unwrap();
    }

    pub fn key_pair(&self) -> sr25519::Pair {
        return sr25519::Pair::from_seed(&self.seed);
    }
}

pub fn get_account_from_seed(seed: [u8; 32]) -> TestAccountIdPK {
    TestAccount::new(seed).account_id()
}

pub fn get_account(index: u8) -> TestAccountIdPK {
    TestAccount::new([index; 32]).account_id()
}
