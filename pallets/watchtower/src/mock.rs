// Copyright 2025 Truth Network.

#![cfg(test)]

use crate::{self as pallet_watchtower, *};
use frame_support::{
    derive_impl, pallet_prelude::MaxEncodedLen, traits::ConstU64,
};
use frame_system as system;
pub use parity_scale_codec::{alloc::sync::Arc, Decode, Encode};
use parking_lot::RwLock;
use sp_avn_common;
pub use sp_core::{
    offchain::{
        testing::{
            OffchainState, PendingRequest, PoolState, TestOffchainExt, TestTransactionPoolExt,
        },
        OffchainDbExt, OffchainWorkerExt, TransactionPoolExt,
    },
    sr25519, H256,
};
use sp_keystore::{testing::MemoryKeystore, KeystoreExt};
pub use sp_runtime::{
    testing::{TestXt, UintAuthorityId},
    traits::{BlakeTwo256, IdentityLookup, Verify},
    BuildStorage, Perbill,
};
use sp_state_machine::BasicExternalities;
use std::cell::RefCell;

pub type Signature = sr25519::Signature;
pub type AccountId = <Signature as Verify>::Signer;
pub type Extrinsic = TestXt<RuntimeCall, ()>;
pub type BlockNumber = u64;

type Block = frame_system::mocking::MockBlock<TestRuntime>;

pub fn account_from_bytes(bytes: [u8; 32]) -> AccountId {
    AccountId::from(sr25519::Public::from_raw(bytes))
}

pub use sp_avn_common::{RootId, RootRange};

pub mod pallet_summary {
    use super::*;

    pub use sp_avn_common::{RootId, RootRange};

    #[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, scale_info::TypeInfo)]
    pub enum SummaryStatus {
        Pending,
        Accepted,
        Rejected,
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, scale_info::TypeInfo)]
    pub enum Event<T: frame_system::Config> {
        SummaryReadyForValidation { root_id: RootId<BlockNumberFor<T>> },
    }

    impl<T: frame_system::Config> Event<T> {
        pub fn root_id(&self) -> &RootId<BlockNumberFor<T>> {
            match self {
                Event::SummaryReadyForValidation { root_id } => root_id,
            }
        }
    }

    impl MaxEncodedLen for SummaryStatus {
        fn max_encoded_len() -> usize {
            1
        }
    }
}

frame_support::construct_runtime!(
    pub enum TestRuntime
    {
        System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
        AVN: pallet_avn::{Pallet, Storage, Event, Config<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Watchtower: pallet_watchtower::{Pallet, Call, Storage, Event<T>, ValidateUnsigned},
    }
);

pub struct MockVoteStatusNotifier;
impl VoteStatusNotifier<BlockNumber> for MockVoteStatusNotifier {
    fn on_voting_completed(
        root_id: RootId<BlockNumber>,
        status: sp_avn_common::VotingStatus,
    ) -> DispatchResult {
        log::debug!(
            target: "watchtower::mock",
            "MockVoteStatusNotifier::on_voting_completed called with root_id: {:?}, status: {:?}",
            root_id, status
        );
        Ok(())
    }
}

pub struct MockNodeManager;
impl NodeManagerInterface<AccountId, UintAuthorityId> for MockNodeManager {
    fn is_authorized_watchtower(who: &AccountId) -> bool {
        AUTHORIZED_WATCHTOWERS.with(|w| w.borrow().contains(who))
    }

    fn get_node_signing_key(node: &AccountId) -> Option<UintAuthorityId> {
        NODE_SIGNING_KEYS.with(|keys| keys.borrow().get(node).cloned())
    }

    fn get_node_from_local_signing_keys() -> Option<(AccountId, UintAuthorityId)> {
        use sp_runtime::RuntimeAppPublic;

        let local_keys: Vec<UintAuthorityId> = UintAuthorityId::all();
        let authorized_watchtowers = AUTHORIZED_WATCHTOWERS.with(|w| w.borrow().clone());

        // Find the first match between local keys and authorized watchtowers
        for local_key in local_keys.iter() {
            for node in authorized_watchtowers.iter() {
                if let Some(node_signing_key) = Self::get_node_signing_key(node) {
                    if *local_key == node_signing_key {
                        return Some((node.clone(), node_signing_key));
                    }
                }
            }
        }
        None
    }

    fn get_authorized_watchtowers_count() -> u32 {
        AUTHORIZED_WATCHTOWERS.with(|w| w.borrow().len() as u32)
    }
}

thread_local! {
    pub static AUTHORIZED_WATCHTOWERS: RefCell<Vec<AccountId>> = RefCell::new(vec![
        account_from_bytes([1u8; 32]),
        account_from_bytes([2u8; 32]),
        account_from_bytes([3u8; 32]),
    ]);

    pub static NODE_SIGNING_KEYS: RefCell<std::collections::HashMap<AccountId, UintAuthorityId>> =
        RefCell::new({
            let mut keys = std::collections::HashMap::new();
            keys.insert(account_from_bytes([1u8; 32]), UintAuthorityId(1));
            keys.insert(account_from_bytes([2u8; 32]), UintAuthorityId(2));
            keys.insert(account_from_bytes([3u8; 32]), UintAuthorityId(3));
            keys
        });
}

impl Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = ();
    type SignerId = UintAuthorityId;
    type VoteStatusNotifier = MockVoteStatusNotifier;
    type NodeManager = MockNodeManager;
    type MinVotingPeriod = ConstU64<10>;
}

impl<LocalCall> system::offchain::SendTransactionTypes<LocalCall> for TestRuntime
where
    RuntimeCall: From<LocalCall>,
{
    type OverarchingCall = RuntimeCall;
    type Extrinsic = Extrinsic;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl system::Config for TestRuntime {
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type Block = Block;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Nonce = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountData = ();
}

impl pallet_avn::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type AuthorityId = UintAuthorityId;
    type EthereumPublicKeyChecker = ();
    type NewSessionHandler = ();
    type DisabledValidatorChecker = ();
    type WeightInfo = ();
}

impl pallet_timestamp::Config for TestRuntime {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ConstU64<12000>;
    type WeightInfo = ();
}

pub fn watchtower_account_1() -> AccountId {
    account_from_bytes([1u8; 32])
}

pub fn watchtower_account_2() -> AccountId {
    account_from_bytes([2u8; 32])
}

pub fn watchtower_account_3() -> AccountId {
    account_from_bytes([3u8; 32])
}

pub fn unauthorized_account() -> AccountId {
    account_from_bytes([99u8; 32])
}

pub fn get_test_root_id() -> RootId<BlockNumber> {
    RootId { range: RootRange { from_block: 1, to_block: 10 }, ingress_counter: 0 }
}

pub fn get_test_onchain_hash() -> WatchtowerOnChainHash {
    H256::from_slice(&[1u8; 32])
}

pub struct ExtBuilder {
    pub storage: sp_runtime::Storage,
    offchain_state: Option<Arc<RwLock<OffchainState>>>,
    pool_state: Option<Arc<RwLock<PoolState>>>,
    txpool_extension: Option<TestTransactionPoolExt>,
    offchain_extension: Option<TestOffchainExt>,
    offchain_registered: bool,
}

impl ExtBuilder {
    pub fn build_default() -> Self {
        let storage = frame_system::GenesisConfig::<TestRuntime>::default()
            .build_storage()
            .unwrap()
            .into();

        Self {
            storage,
            pool_state: None,
            offchain_state: None,
            txpool_extension: None,
            offchain_extension: None,
            offchain_registered: false,
        }
    }

    pub fn with_watchtowers(mut self) -> Self {
        let watchtowers: Vec<AccountId> = AUTHORIZED_WATCHTOWERS.with(|w| w.borrow().clone());

        BasicExternalities::execute_with_storage(&mut self.storage, || {
            for watchtower in &watchtowers {
                frame_system::Pallet::<TestRuntime>::inc_providers(watchtower);
            }
        });
        self
    }

    pub fn for_offchain_worker(mut self) -> Self {
        assert!(!self.offchain_registered);
        let (offchain, offchain_state) = TestOffchainExt::new();
        let (pool, pool_state) = TestTransactionPoolExt::new();
        self.txpool_extension = Some(pool);
        self.offchain_extension = Some(offchain);
        self.pool_state = Some(pool_state);
        self.offchain_state = Some(offchain_state);
        self.offchain_registered = true;
        self
    }

    pub fn as_externality(self) -> sp_io::TestExternalities {
        let keystore = MemoryKeystore::new();

        let mut ext = sp_io::TestExternalities::from(self.storage);
        ext.register_extension(KeystoreExt(Arc::new(keystore)));

        ext.execute_with(|| {
            frame_system::Pallet::<TestRuntime>::set_block_number(1u32.into());
        });
        ext
    }

    pub fn as_externality_with_state(
        self,
    ) -> (sp_io::TestExternalities, Arc<RwLock<PoolState>>, Arc<RwLock<OffchainState>>) {
        assert!(self.offchain_registered);
        let keystore = MemoryKeystore::new();

        let mut ext = sp_io::TestExternalities::from(self.storage);
        ext.register_extension(KeystoreExt(Arc::new(keystore)));
        ext.register_extension(OffchainDbExt::new(self.offchain_extension.clone().unwrap()));
        ext.register_extension(OffchainWorkerExt::new(self.offchain_extension.unwrap()));
        ext.register_extension(TransactionPoolExt::new(self.txpool_extension.unwrap()));

        assert!(self.pool_state.is_some());
        assert!(self.offchain_state.is_some());

        ext.execute_with(|| {
            Timestamp::set_timestamp(1);
            frame_system::Pallet::<TestRuntime>::set_block_number(1u32.into());
        });

        (ext, self.pool_state.unwrap(), self.offchain_state.unwrap())
    }

    pub fn build_and_execute_with_state<R>(
        self,
        execute: impl FnOnce(
            &mut sp_io::TestExternalities,
            &Arc<RwLock<PoolState>>,
            &Arc<RwLock<OffchainState>>,
        ) -> R,
    ) -> R {
        let (mut ext, pool_state, offchain_state) = self.as_externality_with_state();
        execute(&mut ext, &pool_state, &offchain_state)
    }

    pub fn for_benchmarks(self) -> Self {
        #[cfg(feature = "runtime-benchmarks")]
        {
            use frame_benchmarking::whitelisted_caller;
            let benchmark_caller: AccountId = whitelisted_caller();

            AUTHORIZED_WATCHTOWERS.with(|w| {
                let mut watchtowers = w.borrow_mut();
                if !watchtowers.contains(&benchmark_caller) {
                    watchtowers.push(benchmark_caller.clone());
                }
            });

            NODE_SIGNING_KEYS.with(|keys| {
                let mut key_map = keys.borrow_mut();
                if !key_map.contains_key(&benchmark_caller) {
                    use sp_runtime::testing::UintAuthorityId;
                    key_map.insert(benchmark_caller, UintAuthorityId(999));
                }
            });
        }

        self
    }
}

pub(crate) fn roll_forward(num_blocks_to_roll: u64) {
    let mut current_block = System::block_number();
    let target_block = current_block + num_blocks_to_roll;
    while current_block < target_block {
        current_block = roll_one_block();
    }
}

pub(crate) fn roll_one_block() -> u64 {
    System::on_finalize(System::block_number());
    System::set_block_number(System::block_number() + 1);
    System::on_initialize(System::block_number());
    Watchtower::on_initialize(System::block_number());
    System::block_number()
}

pub fn mock_avn_service_response(
    state: &mut OffchainState,
    from_block: u32,
    to_block: u32,
    response: &Option<Vec<u8>>,
) {
    let url = format!("http://127.0.0.1:2020/roothash/{}/{}", from_block, to_block);

    state.expect_request(PendingRequest {
        method: "GET".into(),
        uri: url.into(),
        response: response.clone(),
        sent: true,
        ..Default::default()
    });
}

pub fn mock_successful_root_hash_response() -> Vec<u8> {
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into()
}

pub fn mock_invalid_root_hash_response() -> Vec<u8> {
    "invalid_hex_response".into()
}

pub fn create_mock_summary_ready_event(
) -> (SummarySource, RootId<BlockNumber>, WatchtowerOnChainHash) {
    (SummarySource::EthereumBridge, get_test_root_id(), get_test_onchain_hash())
}

pub fn assert_watchtower_vote_event_emitted(
    voter: &AccountId,
    instance: SummarySource,
    root_id: &RootId<BlockNumber>,
    vote: bool,
) {
    let events = System::events();
    assert!(
        events.iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::Watchtower(crate::Event::WatchtowerVoteSubmitted {
                    voter: ref v,
                    summary_instance: i,
                    root_id: ref r,
                    vote_is_valid: vote_val
                }) if v == voter && i == instance && r == root_id && vote_val == vote
            )
        }),
        "Expected WatchtowerVoteSubmitted event not found"
    );
}

pub fn assert_consensus_reached_event_emitted(
    instance: SummarySource,
    root_id: &RootId<BlockNumber>,
    result: VotingStatus,
) {
    let events = System::events();
    assert!(
        events.iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::Watchtower(crate::Event::WatchtowerConsensusReached {
                    summary_instance: i,
                    root_id: ref r,
                    consensus_result: ref res
                }) if i == instance && r == root_id && *res == result
            )
        }),
        "Expected WatchtowerConsensusReached event not found"
    );
}
