use self::constants::{
    HALF_HOUR_SCHEDULE_PERIOD, QUORUM_FACTOR, SMALL_EVENT_CHALLENGE_PERIOD, SMALL_VOTING_PERIOD,
};
use constants::{EIGHT_HOURS_SCHEDULE_PERIOD, NORMAL_EVENT_CHALLENGE_PERIOD, NORMAL_VOTING_PERIOD};
use hex_literal::hex;
use pallet_avn::sr25519::AuthorityId as AvnId;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_chain_spec::Properties;
use sc_service::ChainType;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{crypto::UncheckedInto, ecdsa, sr25519, ByteArray, Pair, Public, H160, H256};
use sp_runtime::traits::{IdentifyAccount, Verify};
use tnf_node_runtime::{
    opaque::SessionKeys, AccountId, AnchorSummaryConfig, AssetRegistryConfig, AuraConfig,
    AuthorsManagerConfig, BalancesConfig, EthBridgeConfig, EthereumEventsConfig, GrandpaConfig,
    ImOnlineConfig, NodeManagerConfig, PredictionMarketsConfig, RuntimeGenesisConfig,
    SessionConfig, Signature, SudoConfig, SummaryConfig, SystemConfig, TokenManagerConfig,
    WASM_BINARY,
};

use common_primitives::{
    constants::{currency::*, *},
    types::BlockNumber,
};

pub(crate) type EthPublicKey = ecdsa::Public;
pub(crate) mod constants {
    use crate::chain_spec::*;

    pub(crate) const SMALL_VOTING_PERIOD: BlockNumber = 20 * BLOCKS_PER_MINUTE;
    pub(crate) const NORMAL_VOTING_PERIOD: BlockNumber = 30 * BLOCKS_PER_MINUTE;
    pub(crate) const HALF_HOUR_SCHEDULE_PERIOD: BlockNumber = 30 * BLOCKS_PER_MINUTE;
    pub(crate) const SMALL_EVENT_CHALLENGE_PERIOD: BlockNumber = 5 * BLOCKS_PER_MINUTE;
    pub(crate) const EIGHT_HOURS_SCHEDULE_PERIOD: BlockNumber = 8 * BLOCKS_PER_HOUR;
    pub(crate) const NORMAL_EVENT_CHALLENGE_PERIOD: BlockNumber = 20 * BLOCKS_PER_MINUTE;
    pub const QUORUM_FACTOR: u32 = 3;
}

pub(crate) fn tnf_chain_properties() -> Option<Properties> {
    let mut properties = Properties::new();
    properties.insert("tokenSymbol".into(), "TRUU".into());
    properties.insert("tokenDecimals".into(), 10.into());
    properties.insert("ss58Format".into(), TNF_CHAIN_PREFIX.into());
    return Some(properties);
}

fn session_keys(
    aura: AuraId,
    grandpa: GrandpaId,
    authority_discovery: AuthorityDiscoveryId,
    im_online: ImOnlineId,
    avn: AvnId,
) -> SessionKeys {
    SessionKeys { aura, grandpa, authority_discovery, im_online, avn }
}

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<RuntimeGenesisConfig>;

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Generate an account ID from seed.
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate an Aura authority key.
pub fn authority_keys_from_seed(
    s: &str,
) -> (AccountId, AuraId, GrandpaId, AuthorityDiscoveryId, ImOnlineId, AvnId) {
    (
        get_account_id_from_seed::<sr25519::Public>(s),
        get_from_seed::<AuraId>(s),
        get_from_seed::<GrandpaId>(s),
        get_from_seed::<AuthorityDiscoveryId>(s),
        get_from_seed::<ImOnlineId>(s),
        get_from_seed::<AvnId>(s),
    )
}

fn get_default_node_manager_config() -> NodeManagerConfig {
    return NodeManagerConfig {
        _phantom: Default::default(),
        reward_period: 30u32,
        max_batch_size: 10u32,
        heartbeat_period: 10u32,
        reward_amount: 20 * BASE,
    };
}

pub fn development_config() -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;

    Ok(ChainSpec::from_genesis(
        // Name
        "Development",
        // ID
        "dev",
        ChainType::Development,
        move || {
            testnet_genesis(
                wasm_binary,
                // Initial PoA authorities
                vec![authority_keys_from_seed("Alice")],
                // Sudo account
                get_account_id_from_seed::<sr25519::Public>("Alice"),
                // Pre-funded accounts
                vec![
                    get_account_id_from_seed::<sr25519::Public>("Alice"),
                    get_account_id_from_seed::<sr25519::Public>("Bob"),
                    get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
                ],
                true,
                // TNF bridge contract
                H160(hex!("5ABa34F607Ef8Ec56315b1A003Cd75114b41107B")),
                // Processed events
                vec![],
                // Lift transactions
                vec![],
                SMALL_EVENT_CHALLENGE_PERIOD,
                HALF_HOUR_SCHEDULE_PERIOD,
                SMALL_VOTING_PERIOD,
                // Tnf native token contract
                H160(hex!("c597D0a71fFFB0bA72D7d59479dfD66132a2B0E1")),
                tnf_dev_ethereum_public_keys(),
                None,
                get_default_node_manager_config(),
            )
        },
        // Bootnodes
        vec![],
        // Telemetry
        None,
        // Protocol ID
        None,
        None,
        // Properties
        tnf_chain_properties(),
        // Extensions
        None,
    ))
}

pub fn local_testnet_config() -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;

    Ok(ChainSpec::from_genesis(
        // Name
        "Local Testnet",
        // ID
        "local_testnet",
        ChainType::Local,
        move || {
            testnet_genesis(
                wasm_binary,
                // Initial PoA authorities
                vec![authority_keys_from_seed("Alice"), authority_keys_from_seed("Bob")],
                // Sudo account
                get_account_id_from_seed::<sr25519::Public>("Alice"),
                // Pre-funded accounts
                vec![
                    get_account_id_from_seed::<sr25519::Public>("Alice"),
                    get_account_id_from_seed::<sr25519::Public>("Bob"),
                    get_account_id_from_seed::<sr25519::Public>("Charlie"),
                    get_account_id_from_seed::<sr25519::Public>("Dave"),
                    get_account_id_from_seed::<sr25519::Public>("Eve"),
                    get_account_id_from_seed::<sr25519::Public>("Ferdie"),
                    get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
                    get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
                ],
                true,
                // TNF bridge contract
                H160(hex!("5ABa34F607Ef8Ec56315b1A003Cd75114b41107B")),
                // Processed events
                vec![],
                // Lift transactions
                vec![],
                SMALL_EVENT_CHALLENGE_PERIOD,
                HALF_HOUR_SCHEDULE_PERIOD,
                SMALL_VOTING_PERIOD,
                // Tnf native token contract
                H160(hex!("c597D0a71fFFB0bA72D7d59479dfD66132a2B0E1")),
                tnf_dev_ethereum_public_keys(),
                None,
                get_default_node_manager_config(),
            )
        },
        // Bootnodes
        vec![],
        // Telemetry
        None,
        // Protocol ID
        None,
        None,
        // Properties
        tnf_chain_properties(),
        // Extensions
        None,
    ))
}

pub fn dev_testnet_config() -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;

    Ok(ChainSpec::from_genesis(
        // Name
        "Tnf Dev Testnet",
        // ID
        "tnf_dev_testnet",
        ChainType::Live,
        move || {
            testnet_genesis(
                wasm_binary,
                // Initial PoA authorities
                dev_testnet_candidates_keys(),
                // Sudo account
                AccountId::from(hex![
                    "8276f54bd529de582ee80d457c6ea10ce3167ffebd6eb015a1adcd7e2c2ab469"
                ]),
                // Pre-funded accounts
                vec![
                    // Sudo account
                    AccountId::from(hex![
                        "8276f54bd529de582ee80d457c6ea10ce3167ffebd6eb015a1adcd7e2c2ab469"
                    ]),
                    get_account_id_from_seed::<sr25519::Public>("Bank"),
                ],
                true,
                // TNF bridge contract
                H160(hex!("5816CEDff9DE7c5FB13dcFb1cE9038014b929b7E")),
                // Processed events
                vec![],
                // Lift transactions
                vec![],
                NORMAL_EVENT_CHALLENGE_PERIOD,
                EIGHT_HOURS_SCHEDULE_PERIOD,
                NORMAL_VOTING_PERIOD,
                // Tnf native token contract
                H160(hex!("25560bD4FD693922450D99188Fab23472e59015F")),
                dev_testnet_ethereum_public_keys(),
                None,
                get_default_node_manager_config(),
            )
        },
        // Bootnodes
        vec![],
        // Telemetry
        None,
        // Protocol ID
        None,
        None,
        // Properties
        tnf_chain_properties(),
        // Extensions
        None,
    ))
}

pub fn public_testnet_config() -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;

    Ok(ChainSpec::from_genesis(
        // Name
        "Tnf Public Testnet",
        // ID
        "tnf_public_testnet",
        ChainType::Live,
        move || {
            testnet_genesis(
                wasm_binary,
                // Initial PoA authorities
                public_testnet_candidates_keys(),
                // Sudo account
                AccountId::from(hex![
                    "defdc90405497fee04b4db6586666f9d4f3a62450983b0116ccd0f180fea3b73"
                ]),
                // Pre-funded accounts
                vec![
                    // Sudo account
                    AccountId::from(hex![
                        "defdc90405497fee04b4db6586666f9d4f3a62450983b0116ccd0f180fea3b73"
                    ]),
                    get_account_id_from_seed::<sr25519::Public>("Bank"),
                ],
                true,
                // TNF bridge contract
                H160(hex!("ad36dB955A0C881A78842eE1C8e848a7238637e8")),
                // Processed events
                vec![],
                // Lift transactions
                vec![],
                NORMAL_EVENT_CHALLENGE_PERIOD,
                EIGHT_HOURS_SCHEDULE_PERIOD,
                NORMAL_VOTING_PERIOD,
                // Tnf native token contract
                H160(hex!("6cAEfA7446E967018330cCeC5BA7A43956a45137")),
                public_testnet_ethereum_public_keys(),
                None,
                NodeManagerConfig {
                    _phantom: Default::default(),
                    reward_period: BLOCKS_PER_DAY as u32,
                    max_batch_size: 100u32,
                    heartbeat_period: 10u32,
                    reward_amount: 75_000_000 * BASE,
                },
            )
        },
        // Bootnodes
        vec![],
        // Telemetry
        None,
        // Protocol ID
        None,
        None,
        // Properties
        tnf_chain_properties(),
        // Extensions
        None,
    ))
}

pub(crate) fn dev_testnet_candidates_keys(
) -> Vec<(AccountId, AuraId, GrandpaId, AuthorityDiscoveryId, ImOnlineId, AvnId)> {
    let initial_authorities: Vec<(
        AccountId,
        AuraId,
        GrandpaId,
        AuthorityDiscoveryId,
        ImOnlineId,
        AvnId,
    )> = vec![
        (
            // account: 5H944g4h6ZyJN1DzZUK86FnRjiGGkavDfWZxmVRMfwL56jNw
            hex!["e07e2bf370c83cc5e587bb043b89e405a08eccf951fbda61a8f59992e79dcf79"].into(),
            // aura: 5CGDXdqf3u4qNEwyak5UKz9eoK71vX2qrsbpeFdTYrVnCmXM
            hex!["08c79573882a16cc29ead931e5ce71301cae17a60dcd9bea18d5e0994dadc926"]
                .unchecked_into(),
            // gran: 5HijQmtC3pKpzAQPBTtZkuNMZpFuTo2FTepYni6uBJxu9Yye
            hex!["fa2db8ebafaeecaf37606c4094cc04deb4f44149edf30b3f6422df382d0569ca"]
                .unchecked_into(),
            // audi: 5GQWmjRsA2A6Pj5Phe9GoxZWPm2rcKnubjjKGJKKaYz2K2Ah
            hex!["c00c86ade6ed9e1d6e3a5c81870ed51c067c7750eea951642a6d0e71b2376a73"]
                .unchecked_into(),
            // imon: 5HYXg2i8Et2cVi4HikXsd6upXu6ZdkwjpqRg31HxvdZNHAkG
            hex!["f265c215048c119627b16381e8f93896eafbc11af46fee9af20c6bbb74aca266"]
                .unchecked_into(),
            // avnk: 5GxZM8wN3JSwLozGbSzvtkBnZb7CTVHToafE9twqUFgFteTR
            hex!["d87d0ceb36d44d38367c9cbc37dc13dd618d1f220adc4823adc916ca48b68060"]
                .unchecked_into(),
        ),
        (
            // account: 5EeZcKuD7T4mVG377JWxK7XqziBZwY4iGZxRqLvk3CzVbRRo
            hex!["724b0c3f8edec4763f1de1aaa763d8b6bbfe898eff0cf84cb0fe21506ba6bc33"].into(),
            // aura: 5DRjy31RkFw3bLFvjRRyRceJNv2DEtpdtHSCgNytNq6QYcmH
            hex!["3c477d3351e4ab5b398a2a2c1caa883f6c18e2d1662161a7f72c60cf300ef71b"]
                .unchecked_into(),
            // gran: 5Fcrkj4MBYkPvEDWFdVaLQxUa2EMbizSdsz8iXj2711Rc3e9
            hex!["9d3a814cfd374533eba1f16dc53b9b7ed4de7b04f6138cebf8ec69c1866822c5"]
                .unchecked_into(),
            // audi: 5Fsjv5jxHcoJrKyx13efnfi2h81eWrWeid2naCTFDNpUriYv
            hex!["a8942a43fb2884c69beea41a67307380ff938b94a2251098e93538dabba20f65"]
                .unchecked_into(),
            // imon: 5DUNUydzuYzRHs3iRfKwoLyzBrzhykD7TW2qvPPrnw5Y3kQr
            hex!["3e48e78ce622117b009921e3988d813324423cdfc107249be0dc79a76b26d63f"]
                .unchecked_into(),
            // avnk: 5HZ1dxdzscfNXio4abPzdyLwHg5Z6zvmKHMFZ82MK21tm7xS
            hex!["f2c3e4f638ff276c7c89eda87aa82d8556716e56fb83abc23c6f5f4effed4219"]
                .unchecked_into(),
        ),
        (
            // account: 5HbgK4dQ1gb7jnxy8DF1L8TXeizSPeRvHhGZ4v1yonCUDpoy
            hex!["f4cc92fa71b6cc32d93a10d8e828cc6803bb6ea603c1c70b2b5413f7d6ac853e"].into(),
            // aura: 5Hjd87cLkfmRRZaXJqKd3fbcAHSqidj2ietJ49XqKtK36ka4
            hex!["fadbcd9279170b4f511d6359fe60654102eba4058bd89ca314e5e1836db3b63e"]
                .unchecked_into(),
            // gran: 5DrdJ3e2hXdcmYFwG3sD32zpRCLa1KXYLkS6CeB7EL3JjUUQ
            hex!["4f4229d85f535798251ef8100955428a43dc0fe073cd90e7d55e974160bb16f3"]
                .unchecked_into(),
            // audi: 5GX1GWLmiRYKiUtpDV7Nmk3Qqk7DHiFvzFiDnhueeMH6Cfwo
            hex!["c4ffeccb00b15837e0f27b4659b2bcfbbe51de46783c35fde403fbe1b69a770d"]
                .unchecked_into(),
            // imon: 5EvGxChWcCWnhPHZBdgmKMfwYSmDxNXaU4b1uMUkg2PnC1yG
            hex!["7e46e69ad174e683d774c5bfe215fc81293f0e1bd43f62dc8772269213c8287f"]
                .unchecked_into(),
            // avnk: 5GsJJ61iJJkAU5JF3J957X56Ckpo1oHbdVw8iAgEerF4ucpL
            hex!["d47a26e299ff5e6fd7e4e1119750a5d3a1424be51e308357bb35b6e94187354c"]
                .unchecked_into(),
        ),
        (
            // account: 5EZfSzbFWgxsbQEF8tCNf2A1cG41uyN21BNPTET9VSvyDYJk
            hex!["6e8e7a18fbcfa0afdb43ebc32e447c162e08129f42b918995660a0e94ddd102b"].into(),
            // aura: 5Fe7A9cjn144wtGT6pZjLznCoz1TkM3GLAaHnSbWhpqZ8mDF
            hex!["9e2e3cf8f48d318281220b502a94da2a0ec952b69d8bb428096a76f15d436b61"]
                .unchecked_into(),
            // gran: 5GTc2WwFGrWhpSuxUuRkRAGxS9MMyzazK72nzfAYXAFffFVe
            hex!["c267f3af8be22e1fe2ca48748aa3550e3f3c5870b70b071411571d749c3e43f0"]
                .unchecked_into(),
            // audi: 5G9d5nYgonoppGBFyCFFLwmyazw6uvvvVZcYMd9E3CPzGL5e
            hex!["b4b11735780e7e7430c52c9316746d01af4c4d4ba19740b08edd1c68c8bb0738"]
                .unchecked_into(),
            // imon: 5FkNYgAk6UA6nFrRd2JU3C6Qrj8HhccANbDTY4G1eNVR5pjJ
            hex!["a2f583363a633fe48d2294352f3c213a18f032f6082227a80c28cbaa9dc0f035"]
                .unchecked_into(),
            // avnk: 5C7NxXheZpZQBKF7wM9XR2KCgiXFwMcJMwH7qoeM898Wbpg4
            hex!["020a1de05bc2d5f516a2dc4bba1c914dcdd2319f557298ec5a176a7d43874337"]
                .unchecked_into(),
        ),
        (
            // account: 5ERk5KpNhR1uKE3E2X5tyxWRcVR9TL91AHj6DUfe3wA71gTH
            hex!["6884166b647a650bfb9670d3d0f316fc6243177ad9465a0165b049577246ef69"].into(),
            // aura: 5HRLq5YRS6MDKmL3q3VDJpFn7hjUbxRWmmwBo7Hc29vkCNiw
            hex!["ecea8943f84c46eee1d4e3deaf84aad2e56568bd8bd8d5ab1c809574d8057b2d"]
                .unchecked_into(),
            // gran: 5DGsD6CDfAageJWvgGwWHwGvb4RXi38Rg4xrcDYZq1bHkd71
            hex!["3582a907642802f569c827a858576944b33e52475b51e9d31449a81227434e7c"]
                .unchecked_into(),
            // audi: 5EoH4jAduoW2oGxJ2RU1dZ8ibczfSyF2RoRvrMztJ1TVTJrM
            hex!["78f08fcb4b2cd71498a596c6da23ddd99f0554f55f493f2c26cfba2e9aa6b95a"]
                .unchecked_into(),
            // imon: 5CRpyst9mdMZRKTrtK2MFvXvTzuBNtiH8rGZjC2goYECAkHZ
            hex!["101c22d22ff524f9c981ec3797f3fc89fea3e5a042027a9656daa5900c865b4b"]
                .unchecked_into(),
            // avnk: 5FRyVT21TuDzkEjvCXjnFrWpg4ExWz5Lv5M8NTfUDSio1LHh
            hex!["94ed7bab9da720d457094c3d79a524443c74cea46bf830a2636fc57720b0ce71"]
                .unchecked_into(),
        ),
    ];
    return initial_authorities;
}

pub(crate) fn dev_testnet_ethereum_public_keys() -> Vec<EthPublicKey> {
    return vec![
        // 0x4EFfeDe129e2f74A7Cf8b559Bc7c5713097A7c80
        ecdsa::Public::from_slice(&hex![
            "02485ee3a192eb693ec705eb437f418a46db3dc3616699b80e6bd77a2389366c02"
        ])
        .unwrap(),
        // 0x906b3D48f9595888d4ea7fd0ea5769438D1628Ac
        ecdsa::Public::from_slice(&hex![
            "033aed797b6ee54187db786592a065c48976221a662beddad8ee4f2cfcf04b5604"
        ])
        .unwrap(),
        // 0x360E608b4D6a63646c09A272c02001E21CAb8869
        ecdsa::Public::from_slice(&hex![
            "0297396dd5aa6ab7bede976e440a4f96324a48e612943f95f436242318f35a127a"
        ])
        .unwrap(),
        // 0x82baEdC5e93c59F2ac97c0f65bb4499CBd1ba03A
        ecdsa::Public::from_slice(&hex![
            "03ea4210cc61cd598f760dd0f7de1c8458ba16cd2c0fc4bef232e484ca820fd511"
        ])
        .unwrap(),
        // 0xB1DD07E7DF6b2f11A9305a98cbcdb7Dc403f9655
        ecdsa::Public::from_slice(&hex![
            "0231ebb34b20b5f9d2761feef8f55ab50a5041634a7e190945ddf9c9c847dd851c"
        ])
        .unwrap(),
    ]
}

pub(crate) fn public_testnet_candidates_keys(
) -> Vec<(AccountId, AuraId, GrandpaId, AuthorityDiscoveryId, ImOnlineId, AvnId)> {
    let initial_authorities: Vec<(
        AccountId,
        AuraId,
        GrandpaId,
        AuthorityDiscoveryId,
        ImOnlineId,
        AvnId,
    )> = vec![
        (
            // account: 5ETqzTCh8EzS4Qr6XvnSSjEeHf4aDJHzRHV7MbFDVdrPxUAA
            hex!["6a1e7e4df2f9eb44c28cb783284cf9ca7f82ff9db0a45b5e870f088083afcb2b"].into(),
            // aura: 5Fgyi7KQ4PNgowVYDWxmkRKaYcfcN8TnqimQsYz9Aiimw884
            hex!["a05ee621e557f5e2dc826580a574ab8299e266cd019e196a3350a0b0c778ee6e"]
                .unchecked_into(),
            // gran: 5DRKCBCJWSrzM3FuKC29L3X7M6Y6jrw9GrfwT2gjsjtCyuaQ
            hex!["3bf41822265b7159ba0b6f88255aabeeb4881957d11b6aa3eb3af9fcc28befd1"]
                .unchecked_into(),
            // audi: 5DfaN5cVUzjrePza4H7ydwZ14V19wPTAQui6MmX27aTvzBFG
            hex!["46d49967fe97bb44a85edbeef610c218b1bcd156ffff36d7bfb84ceded78b72e"]
                .unchecked_into(),
            // imon: 5HYqdUHBjFceawfYCdwV8PszZuitNXpQPhcJVTxguGv3JfaZ
            hex!["f2a233f80272a838fd720ed66e7bca464df1a8fce765a55df7e9ced399610659"]
                .unchecked_into(),
            // avnk: 5DLAVKVk9msWhgqTtVbre5KFXhkz9H7CAQLjHGi7pEsowwJK
            hex!["380690c684e5bdbfd88190e1ffc264dc5c1213a7d6dcc09a612cd1e98e98391b"]
                .unchecked_into(),
        ),
        (
            // account: 5H6enpsFZZiFam4m8guSJNz7FepDY7sRNaKRj8AN2GQZMadA
            hex!["dea9560dbdc1371c64e9a1cd7a34b2f3f7cc392ae1b1a42377b06c697ba81a20"].into(),
            // aura: 5CzRzQ3ykxNwoyqGiD8nNaAnUBNN3WjFyeWX97MyqNMm91nF
            hex!["28f9d6f36e93ddff3f8d457ea130b6903309db9272c9998d1dcbb562d047c00a"]
                .unchecked_into(),
            // gran: 5GCWyZ8PmfrtkRyUowL6x5HYtvYA7fV1YFdqukQtiunCjuz9
            hex!["b6e64474bcfea39fbc9d24133f01b7b4b1ff290de80740fb6b26e653fc4be046"]
                .unchecked_into(),
            // audi: 5D8A1LUvWaZ6WQhzXjDa55UsHs8akjZmEVgK1ftFdfq1GNzx
            hex!["2eddff1e182096cd1d6ee121c48374a62352e6108106b1f0d0e67d071d5ba51a"]
                .unchecked_into(),
            // imon: 5GEExNeQGAwgaX86HQDyWmmo2kSV6QeWVsccYDEtJ3mjMcA3
            hex!["b836d431c4165a9c1e78903831fab5499d9f4ebd37eb332bf4fb0c9b91982511"]
                .unchecked_into(),
            // avnk: 5CY9uAoh9pSaXdjb74PYWWJndzTbE6pFtsBAXf2BPBizfoPG
            hex!["14ef4b7d7fc371436651a3ce24fe39b4dc23d5bd8fe865ab05d95916b6139e1d"]
                .unchecked_into(),
        ),
        (
            // account: 5EEgcP7MGVEWy4657gZeHoUCdVJ1pMH24Q1P2eU8HS3iMVuv
            hex!["6014b9b73bebd99afb90dd17163b7416652f3be341da57b82418462eaf8dc404"].into(),
            // aura: 5FHQq8mKuqaGoLYUtW9fWv4bgu9J8Amid2VKPnyVx3am5QzH
            hex!["8e659310f5b7a4743436bcfce14a1f7530367388a2117ff71a1acd6536357e45"]
                .unchecked_into(),
            // gran: 5GLqSxQ8oCy6DX6Hd95o4sjUk6MSGQ15EQ9AWAScmYHEw9vY
            hex!["bd3e6a162805cc301e9fac3aafca7c31efd30c38a54050ab69bfbffd4d558f9e"]
                .unchecked_into(),
            // audi: 5FEHwPbQdm2eroUnMRB2zVtdw5fE4gNwwMmSHSiTedwvpnLK
            hex!["8c04a32239c86e1279236457120487a2275b64345e8bedc8626ba25f24daab32"]
                .unchecked_into(),
            // imon: 5GxRUgSheNo5sZaEMevkZ21ZsG5BBpDJUxZpf9uYhxfQXYtt
            hex!["d8628ed0776d556e9046dc5a555a853a1ea10bcccd60cd62bc0a1acd4649a204"]
                .unchecked_into(),
            // avnk: 5FEqXNSMpukJWSewZu52oYRJcATAPAFvw3ocGorhDrmf4Pe2
            hex!["8c6ef6f8dce7ea9a78ec4f283bda4a5bf50b4ea1af9015fcc2a23eb9a367c10c"]
                .unchecked_into(),
        ),
        (
            // account: 5D9j4R9BKCUMiB1BEgFw6rzAwox5HCtGrg51MDCVP4tMzjAt
            hex!["3010823654b4e9d74b42269826080f92d2dc062864e11032d0ee2aa371267d55"].into(),
            // aura: 5DnicFVA4CJL2k3q5GyFVwFBVA9qL89Tq3oR1VWS9xVHnqFe
            hex!["4c4711e599fa0a3b497b286f77747ad66a1b0fb42860b4d42b0c21f026f5c820"]
                .unchecked_into(),
            // gran: 5GJBxwBdStdtPSDgVmKGgzqL1GSAmrriJVv3BxJt8Uw3XDkU
            hex!["bb39be816aa0d988c96f79ad841d56f4127da05298f128aa6b09305b802768e2"]
                .unchecked_into(),
            // audi: 5DwAjFoS86amWTcSdauC3n6N9X61EaLQ3SAFD1a33hi6RGz2
            hex!["52b8f6aa85d53d21b7c3e4dec7afc59418d3d8e7f7f9b98846f9d2855e9f032d"]
                .unchecked_into(),
            // imon: 5EkC2VmyqNV8NaMSiFJMDYTYp4s7kNtaR9R7hcvSUq6JddJp
            hex!["7695dd501505a4879201fbf7b87347ab44db5d4bc2ddbd1d3a3b3dea6ec19d54"]
                .unchecked_into(),
            // avnk: 5He85quBVPc5dSjHvZQRteDEJ8fxpBrp9rMzxwmtNeiwVX8d
            hex!["f6a9d24d5611de40851435d2f6ec1f98bed6d659c61b15a8732eff90f3378016"]
                .unchecked_into(),
        ),
        (
            // account: 5En4gpDctYk1Wy89KGk5XJNFNpc9ZHtiPfcdh4UMrpNbEKAu
            hex!["7803a61c895984f9c7a7e2900d26cdb8c558ee58e7df9845d7cd4600ef82ac44"].into(),
            // aura: 5EkQaPAfeyMeDdQ5W8UDSbnHx2ctHU3VNbsmim8L5akp1S2b
            hex!["76c01c5ac02880bb5f6dbc274aaaafcf98a9c20abf8ab177b7ee8d8974dd7f4c"]
                .unchecked_into(),
            // gran: 5EceNCayniaRxH6eqZRKxEGDEeHCy8nGy4jPSTBbLA4Kuhj5
            hex!["70d49193d4c9b4b94a74459298f99e7cec15c744257d448529712ad6aa4e9c49"]
                .unchecked_into(),
            // audi: 5HBX9Yck2FXTnDRi4Y88Sg4Gf6N4tJibfJkrPMEKsyDkXU3T
            hex!["e25fd5d8d5deba31b3ac8ac9ecfe11feb005371dc1d5e5fa5408915724c52920"]
                .unchecked_into(),
            // imon: 5ChgiKbsPZGvrW17AD2SxFouX4WLCn7uPZicFjMwfQvifoWn
            hex!["1c343aac46f1287f43d41d5db038a3f95ad1fe57e5d0eb2215a4f551f9f15f65"]
                .unchecked_into(),
            // avnk: 5HHNaPwn6zjofdFoYV5p6y6uLWxSvddYYSqHGRZcCv6GNhJm
            hex!["e6d67410a6b513c1a61b16789dfb681488f976577ce171feb46aba8514056365"]
                .unchecked_into(),
        ),
    ];
    return initial_authorities;
}

pub(crate) fn public_testnet_ethereum_public_keys() -> Vec<EthPublicKey> {
    return vec![
        // 0xfA61F1aECB01E7569DED655830991D1a81715a63
        ecdsa::Public::from_slice(&hex![
            "02592d0ade0996f68f2144043543d96e7cab45067c843e63619bd80aa0babd0f7b"
        ])
        .unwrap(),
        // 0xdCb1EE2698d75122A5f515Fc0008C315a5f985AE
        ecdsa::Public::from_slice(&hex![
            "0355f69aa6f7f7e780f550eae0fdebd6be93d465b8e20ff4633d4ae8ec54d24ad8"
        ])
        .unwrap(),
        // 0x59c14ea597De4e9104ED54d5e1319fe675c38C9b
        ecdsa::Public::from_slice(&hex![
            "03e5c6b4372f83d6931148a892a4d548eead399d77064e4200dec3cfee8233f59d"
        ])
        .unwrap(),
        // 0xF5Cd702FfC63f217b4425FC1472a80161a00DB8f
        ecdsa::Public::from_slice(&hex![
            "02c5c1490a3b126036c862cc8e6dc7fef4b8d428d6b11813dc26c1d8e5ac54d3ef"
        ])
        .unwrap(),
        // 0xE12fb4a709a83F402f1C074DAd2ADf4cd42490d1
        ecdsa::Public::from_slice(&hex![
            "032283cd4d7e2901395cb7d7167960060c6754012d025c216fb2f8d333e14d9580"
        ])
        .unwrap(),
    ]
}

/// Configure initial storage state for FRAME modules.
fn testnet_genesis(
    wasm_binary: &[u8],
    initial_authorities: Vec<(
        AccountId,
        AuraId,
        GrandpaId,
        AuthorityDiscoveryId,
        ImOnlineId,
        AvnId,
    )>,
    root_key: AccountId,
    endowed_accounts: Vec<AccountId>,
    _enable_println: bool,
    tnf_eth_contract: H160,
    processed_events: Vec<(H256, H256)>,
    lift_tx_hashes: Vec<H256>,
    event_challenge_period: BlockNumber,
    schedule_period: BlockNumber,
    voting_period: BlockNumber,
    l2_token_contract: H160,
    eth_public_keys: Vec<EthPublicKey>,
    default_non_l2_token: Option<H160>,
    node_manager: NodeManagerConfig,
) -> RuntimeGenesisConfig {
    RuntimeGenesisConfig {
        avn: pallet_avn::GenesisConfig {
            _phantom: Default::default(),
            bridge_contract_address: tnf_eth_contract,
        },
        system: SystemConfig {
            // Add Wasm runtime to storage.
            code: wasm_binary.to_vec(),
            ..Default::default()
        },
        balances: BalancesConfig {
            // Configure endowed accounts with initial balance of 100 TRUU (BASE)
            balances: endowed_accounts.iter().cloned().map(|k| (k, 100 * BASE)).collect(),
        },
        aura: AuraConfig { authorities: vec![] },
        grandpa: GrandpaConfig { ..Default::default() },
        session: SessionConfig {
            keys: initial_authorities
                .iter()
                .map(|x| {
                    (
                        x.0.clone(),
                        x.0.clone(),
                        session_keys(
                            x.1.clone(),
                            x.2.clone(),
                            x.3.clone(),
                            x.4.clone(),
                            x.5.clone(),
                        ),
                    )
                })
                .collect::<Vec<_>>(),
        },
        authors_manager: AuthorsManagerConfig {
            authors: initial_authorities
                .iter()
                .map(|x| x.0.clone())
                .zip(eth_public_keys.iter().map(|pk| pk.clone()))
                .collect::<Vec<_>>(),
        },
        authority_discovery: Default::default(),
        im_online: ImOnlineConfig { keys: vec![] },
        eth_bridge: EthBridgeConfig {
            _phantom: Default::default(),
            eth_tx_lifetime_secs: 2 * BLOCKS_PER_HOUR as u64,
            next_tx_id: 1u32,
            eth_block_range_size: 20u32,
        },
        sudo: SudoConfig {
            // Assign network admin rights.
            key: Some(root_key.clone()),
        },
        transaction_payment: Default::default(),
        ethereum_events: EthereumEventsConfig {
            nft_t1_contracts: vec![],
            processed_events: processed_events
                .iter()
                .map(|(sig, tx)| (sig.to_owned(), tx.to_owned(), true))
                .collect::<Vec<_>>(),
            lift_tx_hashes,
            quorum_factor: QUORUM_FACTOR,
            event_challenge_period,
        },
        summary: SummaryConfig { schedule_period, voting_period, _phantom: Default::default() },
        anchor_summary: AnchorSummaryConfig {
            schedule_period,
            voting_period,
            _phantom: Default::default(),
        },
        token_manager: TokenManagerConfig {
            _phantom: Default::default(),
            lower_account_id: H256(hex!(
                "000000000000000000000000000000000000000000000000000000000000dead"
            )),
            // Tnf native token contract
            avt_token_contract: l2_token_contract,
            lower_schedule_period: 300,
            balances: {
                if default_non_l2_token.is_some() {
                    endowed_accounts
                        .iter()
                        .cloned()
                        .map(|k| (default_non_l2_token.unwrap(), k, 100 * BASE))
                        .collect()
                } else {
                    vec![]
                }
            },
        },
        nft_manager: Default::default(),
        node_manager,
        advisory_committee: Default::default(),
        tokens: Default::default(),
        asset_registry: AssetRegistryConfig { last_asset_id: Default::default(), assets: vec![] },
        prediction_markets: PredictionMarketsConfig { vault_account: Some(root_key.clone()) },
    }
}

fn tnf_dev_ethereum_public_keys() -> Vec<EthPublicKey> {
    return vec![
        ecdsa::Public::from_slice(&hex![
            "02607fa03c770bcdab1c1c57379547e1497bdf984c88964b4850f0e7ff61fa5e4c"
        ])
        .unwrap(),
        ecdsa::Public::from_slice(&hex![
            "02cc03652fb15df45212c9fe99c6e456a532e204b8dd6566ca6b288eb822c90779"
        ])
        .unwrap(),
        ecdsa::Public::from_slice(&hex![
            "0262ebe4e87161a52647a111bf7f790b12b37031fb999176ea53078ef782806850"
        ])
        .unwrap(),
        ecdsa::Public::from_slice(&hex![
            "02fd28d1a51307b69ad7b1c702ba33969c37e323950128d00f7f4ce60cb744bfe4"
        ])
        .unwrap(),
        ecdsa::Public::from_slice(&hex![
            "03c9a1c6b1dce4c228a1577cfa252c7120f69404d9f40e42b1137f484e95e08f61"
        ])
        .unwrap(),
    ];
}

pub fn mainnet_config() -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;

    Ok(ChainSpec::from_genesis(
        // Name
        "Tnf Mainnet",
        // ID
        "tnf_mainnet_v1",
        ChainType::Live,
        move || {
            testnet_genesis(
                wasm_binary,
                // Initial PoA authorities
                mainnet_candidates_keys(),
                // Sudo account
                AccountId::from(hex![
                    "ca04d57ddbd9c5af2e0d2a4df82673caab59c0daf9376f3201ba87bc3fbedd1f"
                ]),
                // Pre-funded accounts
                vec![
                    // Sudo account
                    AccountId::from(hex![
                        "ca04d57ddbd9c5af2e0d2a4df82673caab59c0daf9376f3201ba87bc3fbedd1f"
                    ]),
                ],
                true,
                // TNF bridge contract
                // TODO update me
                H160(hex!("ad36dB955A0C881A78842eE1C8e848a7238637e8")),
                // Processed events
                // TODO update me
                vec![],
                // Lift transactions
                vec![],
                NORMAL_EVENT_CHALLENGE_PERIOD,
                EIGHT_HOURS_SCHEDULE_PERIOD,
                NORMAL_VOTING_PERIOD,
                // Tnf native token contract
                H160(hex!("dae0fafd65385e7775cf75b1398735155ef6acd2")),
                mainnet_ethereum_public_keys(),
                None,
                NodeManagerConfig {
                    _phantom: Default::default(),
                    reward_period: BLOCKS_PER_DAY as u32,
                    max_batch_size: 100u32,
                    heartbeat_period: 10u32,
                    reward_amount: 75_000_000 * BASE,
                },
            )
        },
        // Bootnodes
        vec![],
        // Telemetry
        None,
        // Protocol ID
        None,
        // Fork ID
        None,
        // Properties
        tnf_chain_properties(),
        // Extensions
        None,
    ))
}

pub(crate) fn mainnet_candidates_keys(
) -> Vec<(AccountId, AuraId, GrandpaId, AuthorityDiscoveryId, ImOnlineId, AvnId)> {
    let initial_authorities: Vec<(
        AccountId,
        AuraId,
        GrandpaId,
        AuthorityDiscoveryId,
        ImOnlineId,
        AvnId,
    )> = vec![
        (
            // account: 5Di5HT5J9RWRVkK7wqA1Fy1Npqco5otwQaehBeuUNncCJbQu
            hex!["48bc71ac1d8ec4f0583f0c0e1eb6bf719e227d49522da8ed4cde9bfe48cbfc03"].into(),
            // aura: 5Dr4TYiNowrskuoScdqSNbuSyNayz2GywCKpRueJugS2rczD
            hex!["4ed3a0838937142a6261211ffe08674654c54ad3343053500404134f8aab3b5d"]
                .unchecked_into(),
            // gran: 5Cv7hN2ke4wah5q3X6wy8kcwQ3wyUgZVTDnvdSw7e4dNCPoC
            hex!["25af46a3c625ed2aa5ccec2a8049685a98551a2b3f32df80869806c694bd2674"]
                .unchecked_into(),
            // audi: 5FpbHQxXjdX1vszNTpjVTETHHKBBsEfMywrhxo6GyPWWGU8c
            hex!["a62d5ec8a049d529fdd3df30c55ef641d42acaf70b2e1a8fe6d4aaebff4ec741"]
                .unchecked_into(),
            // imon: 5HKirmcXqFiYdqLYgW38syNLFBnWd5FCd1WB4LvPierPccBk
            hex!["e8a13873222008c171ab1800caa7670fe96c511c956ff89c56f2a192ecf02d51"]
                .unchecked_into(),
            // avnk: 5EexQkynFrjavaup1zyTCWFx8E35hKVdXy2NpGrnmSAYNfzy
            hex!["7297cd265d2a1368060326aebb2eb9d644f1ac2a39f1172047bf78f1ec53184c"]
                .unchecked_into(),
        ),
        (
            // account: 5CJPghhi1Lcc6b9yptougRzsAGVZ6yzYaSnnnym6Dh7KLv9N
            hex!["0a70438a1b745e50271acdbe63c881181dc7e9c7562bbf1c88f6336262d2a65f"].into(),
            // aura: 5FYptPaNe4TGYi3KqDfq8SYz54EioCKgduWgvcvRijMSFgwP
            hex!["9a273c55da7952b84311a4248feda2e976c1696152b5c0b19e9bd071bd0c9972"]
                .unchecked_into(),
            // gran: 5FWdygLoFtPkPE2kV5hp9Q3mKwmZJ8Pxumhg9nXkgTMikuDW
            hex!["987c05c5e9903b24b6a081c9e983c4b6b27bbe1a4d0d056643483609f646a3e9"]
                .unchecked_into(),
            // audi: 5ERr5pBPWnGjm3S8Kxrh858BTz2bWvckSSfpxXJp3wDiMy1T
            hex!["689850527759c964b998835bb890642adf35290603993644dd9b241093335025"]
                .unchecked_into(),
            // imon: 5GjZ3vDAjPAxygT8VQfVSb36z6nnWsVpcMKzLTQb28t5P9nZ
            hex!["ce91dc5d318698a4ef9541a7a25e55c712695e37de7ae9bba29b773d5b827e32"]
                .unchecked_into(),
            // avnk: 5H3RK8WxSTyyubwd11xw2WfUiEjRw4KsP88LC6qBeQaUYUnz
            hex!["dc323b079c5b268105f3e9d894e439d8d191b2f6437db6c3f953137a9d1f0c6c"]
                .unchecked_into(),
        ),
        (
            // account: 5FHJEaMX4jVLTCHfAMEHxKYzKNBndu8YbSPr7oHRWMNYxyir
            hex!["8e4f5ef30c72969ff456f7ff48b9e6c134929f4e63f5e70f6b039593bdf3a256"].into(),
            // aura: 5EjmfpDYzxeSKUpQpHd3GcFTbw39TTRuzzTnAWWR6xX6LQRn
            hex!["7643df77d2ea68e3832930694d218972e3df7a3d770481903849720d91219008"]
                .unchecked_into(),
            // gran: 5GRkpcaXvXpUut5VfW1wf4dXpTR1QHdcJJz25rfwPLixawR5
            hex!["c0ff11244d92dae6fae3a417cb73e3edc8a2b071bf8642541fbee5236afc606f"]
                .unchecked_into(),
            // audi: 5FF73sXvwVJVf5SjzbnrXZacSB6mcnz31tM8jiqHpwYb3gY5
            hex!["8ca33ac33d393ced7c13d2ad66c87971d472ab933c939f10c5271ff6f7a7a619"]
                .unchecked_into(),
            // imon: 5HDsxk9SFJGEibyWbPBWxq1AFCviAE6CGiNSkXffkk8HmVLF
            hex!["e42c643cb0b293b28fdbcd3b27975a14c8f2134ad94412f8aa1e0172e5d94d33"]
                .unchecked_into(),
            // avnk: 5HEX9E61Zs3D7vUHjZtAwyczn4ds6t2Cb3nfHDSeix8UNmaz
            hex!["e4a98d9217c52133c878e9bc9340b90dbe28151ae5ef103d7f829ab81353621e"]
                .unchecked_into(),
        ),
        (
            // account: 5GbmvsdMWRc6S3iz9bhK4Vgceockpgte3kBwbfaPFw2WWVxC
            hex!["c8a340275eb82c0aa0778574d4adbacd60c79cf89fc3d0f751caa41006c23c61"].into(),
            // aura: 5CMEiFAg17YVtENZ8oywiQrAR24jCc5BUwUiT61km48c5QVF
            hex!["0c9bcaf92ca3e9a3aef9c31c558634bcbd38eddf15feb6efe5971fd8b71d5d50"]
                .unchecked_into(),
            // gran: 5HQnxZNBZvisB7eMm2efaWkNmaxko4cdMauLkU2QPGDHvrrT
            hex!["ec7f3fab8745f35edae92cfe7831b5ab6a6ca9370c08bc59ffbc3e8e8f021d3c"]
                .unchecked_into(),
            // audi: 5CPtL7syaCrdRpQNnjde85f5TiqGvmLGqNuCRpt1VntJHSza
            hex!["0ea0eb3dbe3aefd5ff5614f955223c68a59a7175ec62824e34528149aeeb4155"]
                .unchecked_into(),
            // imon: 5FnMcHnxfZ7SqrQ8mYg3gC2uH59xFhEgJ4968woPSHUc6Y4N
            hex!["a478d8c95dc800801138a2d942ad08dc6deccbef1ee63792f35e842b1af3b92e"]
                .unchecked_into(),
            // avnk: 5HYetTfWqrvPbuqQHk95aKx4xPASMMBcz2QsuApbqHcvj9Yt
            hex!["f27e0b2eb8c3e23c7bb4234ce56f679cee5856840df971b0f86aceec868f8242"]
                .unchecked_into(),
        ),
        (
            // account: 5FCkD3hwTb15jB5ox8Suas56vsvrBf6Q4RmNJLGa8aG5umCf
            hex!["8ad694373add4bea87fbda36b5155044f8d2e7c6761f43d12367227a097fa152"].into(),
            // aura: 5Cqq2CbKJHMhnF6P47u9puwuAuVgTxisNhs7rviXRrLxKZ8c
            hex!["226a29183f103b6af875b837b610c8bcd6aa9a3db66c81fa5dec8c326f8da562"]
                .unchecked_into(),
            // gran: 5CRsgp5xFHbyKeCE1NxD4XtsmoGZeC7xtd23bTB8wEMErA5K
            hex!["10253e940e2a8dd5ff646a2dc2704284f1ae288c600ebfa42399939099b2552b"]
                .unchecked_into(),
            // audi: 5EtXzdu3kgHQzpi14GwSyUzvpLozzZENdv6ubb1kXWHy5Ktf
            hex!["7cf30bb8be1ce67c4bad36c370d6a72db5cd43247cfd47f99e659909fff95e48"]
                .unchecked_into(),
            // imon: 5FLcmnzxexQdKPzADkyLYz7tQE1J2DtnPRE3pt93KMd2Np9S
            hex!["90d7832c42836ccddb122a86518bea56d6291fbf4c688fa81d0f5af388a39e12"]
                .unchecked_into(),
            // avnk: 5GGiALAx1XxidwzAghMtmJgEfAawyuM82TP2ccZcG5hKZMrL
            hex!["ba18e75f2c81318cc55457874525a8f70eb14fbb26a90df8dde3adf98cdc3607"]
                .unchecked_into(),
        ),
    ];
    return initial_authorities;
}

pub(crate) fn mainnet_ethereum_public_keys() -> Vec<EthPublicKey> {
    return vec![
        // 0xadB107062b47AA6124B6B8C373F706480E968B1A
        ecdsa::Public::from_slice(&hex![
            "03e6b029409e6a81b21299e7208b1a6a4d869d7ba9735fd978469b8f2f32934dab"
        ])
        .unwrap(),
        // 0x88eC651413a5C78e4F8a1f0120a33267464395b9
        ecdsa::Public::from_slice(&hex![
            "03a4b1c40828b8c12f3688793b922613653414be52bf5a1d109e1a7afacc4470b5"
        ])
        .unwrap(),
        // 0xF7645C0dBb0024882A8c0D9663d7551ca48c2047
        ecdsa::Public::from_slice(&hex![
            "0350fc23b93f6f883677b976201cd7f74f9d844e5bc520f3b4bcdff9969843bec1"
        ])
        .unwrap(),
        // 0x282f02E73492F4dffA8DB2Cb096F2Eb72fFc63F3
        ecdsa::Public::from_slice(&hex![
            "03a4038b4a6878e32f20763037760e7b2fd59fc0ef0bb146cb2cf1010881526c8b"
        ])
        .unwrap(),
        // 0x9364E5BA6F9ea1e12B6Eba1fb4229801Cd908965
        ecdsa::Public::from_slice(&hex![
            "02bc7fe8abf2884b3ff257b8606bc6fd0670b01bbf7ebe101efd26c44b8d5c5266"
        ])
        .unwrap(),
    ]
}
