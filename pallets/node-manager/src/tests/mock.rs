// Copyright 2025 Truth Network.

#![cfg(test)]

use crate::{self as pallet_node_manager, *};
use common_primitives::constants::{currency::BASE, NODE_MANAGER_PALLET_ID};
use frame_support::{parameter_types, weights::Weight};
use frame_system as system;
use sp_core::{
    offchain::testing::{OffchainState, PendingRequest},
    sr25519, H256,
};
use sp_runtime::{
    testing::{TestXt, UintAuthorityId},
    traits::{BlakeTwo256, ConvertInto, IdentifyAccount, IdentityLookup, Verify},
    BuildStorage, Perbill, SaturatedConversion,
};
use sp_state_machine::BasicExternalities;
use std::cell::RefCell;

pub type Signature = sr25519::Signature;
pub type AccountId = <Signature as Verify>::Signer;
pub type Extrinsic = TestXt<RuntimeCall, ()>;

type Block = frame_system::mocking::MockBlock<TestRuntime>;
