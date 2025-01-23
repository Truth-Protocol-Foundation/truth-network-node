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
use sp_avn_common::event_types::ValidEvents;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{crypto::UncheckedInto, ecdsa, sr25519, ByteArray, Pair, Public, H160, H256};
use sp_runtime::{
    traits::{IdentifyAccount, Verify},
    BoundedVec,
};
use tnf_node_runtime::{
    opaque::SessionKeys, AccountId, AnchorSummaryConfig, Asset, AssetRegistryConfig,
    AssetRegistryStringLimit, AuraConfig, BalancesConfig, CustomMetadata, EthBridgeConfig,
    EthereumEventsConfig, GrandpaConfig, ImOnlineConfig, PredictionMarketsConfig,
    RuntimeGenesisConfig, SessionConfig, Signature, SudoConfig, SummaryConfig, SystemConfig,
    AuthorsManagerConfig, TokenManagerConfig, WASM_BINARY,
};

use codec::Encode;
use common_primitives::{
    constants::{currency::*, *},
    types::BlockNumber,
};
pub use orml_traits::asset_registry::AssetMetadata;

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
    properties.insert("tokenSymbol".into(), "TNF".into());
    properties.insert("tokenDecimals".into(), 10.into());
    properties.insert("ss58Format".into(), TNF_CHAIN_PREFIX.into());
    return Some(properties)
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

pub fn testnet_config() -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;

    Ok(ChainSpec::from_genesis(
        // Name
        "Tnf Testnet",
        // ID
        "tnf_testnet_v4",
        ChainType::Live,
        move || {
            testnet_genesis(
                wasm_binary,
                // Initial PoA authorities
                tnf_authorities_keys(),
                // Sudo account
                AccountId::from(hex![
                    "7cd31fc71b34745337a31428d29e8d0e645c0a785862c088b850927a87878615"
                ]),
                // Pre-funded accounts
                vec![
                    // Sudo account
                    AccountId::from(hex![
                        "7cd31fc71b34745337a31428d29e8d0e645c0a785862c088b850927a87878615"
                    ]),
                ],
                true,
                // TNF bridge contract
                H160(hex!("D31D4bE8B01534B04062672e5d6CC932b0e948b7")),
                // Processed events
                vec![(
                    ValidEvents::Lifted.signature(),
                    H256(hex!("ef9eb934f90153dd2f3bbf16cfd25d641f1508456b7a1d35f35eea581cda5f93")),
                )],
                // Lift transactions
                vec![H256(hex!(
                    "446cdb96c9f336fb24c6191496a4c1b15a2c2b0adb703ac4811e1813bb0dc936"
                ))],
                NORMAL_EVENT_CHALLENGE_PERIOD,
                EIGHT_HOURS_SCHEDULE_PERIOD,
                NORMAL_VOTING_PERIOD,
                // Tnf native token contract
                H160(hex!("bFaffD8001493Dfeb51C26748d2AfF53C2984190")),
                tnf_testnet_ethereum_public_keys(),
                None,
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
            // Configure endowed accounts with initial balance of 100 TNF (BASE)
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
        advisory_committee: Default::default(),
        tokens: Default::default(),
        asset_registry: AssetRegistryConfig {
            last_asset_id: Default::default(),
            assets: vec![(
                H160::from([1; 20]),
                Asset::ForeignAsset(4),
                AssetMetadata::<Balance, CustomMetadata, AssetRegistryStringLimit>::encode(
                    &AssetMetadata {
                        decimals: 6,
                        name: BoundedVec::truncate_from(
                            "Eth USDC - foreign token".as_bytes().to_vec(),
                        ),
                        symbol: BoundedVec::truncate_from("USDC".as_bytes().to_vec()),
                        existential_deposit: 0,
                        location: None,
                        additional: CustomMetadata {
                            eth_address: H160::from([1; 20]),
                            allow_as_base_asset: true,
                        },
                    },
                ),
            )],
        },
        prediction_markets: PredictionMarketsConfig { vault_account: Some(root_key.clone()) },
    }
}

#[rustfmt::skip]
fn tnf_authorities_keys(
) -> Vec<(AccountId, AuraId, GrandpaId,AuthorityDiscoveryId, ImOnlineId, AvnId)> {
	let initial_authorities: Vec<(AccountId, AuraId, GrandpaId, AuthorityDiscoveryId, ImOnlineId, AvnId)> = vec![
		(
			hex!["78155939f63f04d5d9b69cc1cfb3e98c9e7e940cec690a26cbdea7be8b9f7533"].into(),

			hex!["6e4477a528d628a3dc92ade8f5c0844bf21713b7757d50a7f4079287c79d9265"].unchecked_into(),

            hex!["35d944adb5498bcd8e4e27501e13aecddec28fee676dab224f7941d9080bb342"].unchecked_into(),

			hex!["5ca21e88094806900d70998fb1684fbb23aa70b22af22b08bc57309c25670b6c"].unchecked_into(),

			hex!["b4e69c3554c700da19ff78b383df1afae7fff8c1cdae34af472915abf799363d"].unchecked_into(),

			hex!["188f539aea7c884ac5d7deb243b8ff3e14ed5eca5671e746bf6196b6e7f9f631"].unchecked_into(),
		),
		(

			hex!["78bbb5eec6e6d79d679d44f0f6ab820d0c0b955def3b05b8f1dbb23f9048592f"].into(),

			hex!["6c4fce431a884322bf5ff5abf731862fb4d4df3f6bfbeb9ea9435e6a0e9bc84a"].unchecked_into(),

            hex!["4d76eef668527f71a96780ff0550004d58c66dcd4dfddb430269091d02215abb"].unchecked_into(),

			hex!["bc3c04bfe155487ae9e2cb5be05e20c79796660fe16bedfc9d74c8909eb60041"].unchecked_into(),

			hex!["d847db2bdcbc60c16bf4de9e33bfefff83b1f131e04b206aeed23aa03861fc68"].unchecked_into(),

			hex!["50c256214f16037ca860192a6831a31f11979eb3c456cb5cc66d18b804901d62"].unchecked_into(),
		),
		(

			hex!["e293b717b63cf1ebce61a0b4dc8a0fcc7670e7ea9638e45dcda46fe23194c377"].into(),

			hex!["4e95211b1164c3951189edc84880f1ac04246c0514b247501724fab58c1d4862"].unchecked_into(),

            hex!["5d9f1b2253cbb71618b439ad47433ef47a1185d49b132d9ed0a855eab4ffe525"].unchecked_into(),

			hex!["faddbba8514adc9ad22513c6e1e5ca7dde46255524f0e4eab6568c3fb5221b68"].unchecked_into(),

			hex!["1a82f74cf1fc4cbd7f5d79544904aacdc0fca776a8ac1ac5f778ac50b0f7ff7c"].unchecked_into(),

			hex!["f0e4e31c7d876d747363af9ccb7c0aeb6980d8606da1dda70e3e52e53c893201"].unchecked_into(),
		),
		(

			hex!["281d02bdcd58e133a269848d1dc1d730df6173f2e13a71a7007e00f0c7a6223e"].into(),

			hex!["d04c5c09e4d2a38ceb7728f840f7edbc84f143cc292300f21a97a1bbec80e047"].unchecked_into(),

            hex!["8d622f89a21c8552e276a4c7a07f96f47c97734da02ad9fef516685ec6d80798"].unchecked_into(),

			hex!["6c3b24b9b664de6b41793d3e703b0bcc36e172def630035dbc4420b4ff8b3603"].unchecked_into(),

			hex!["5cf916222842df2991cbf3103bef62bfad0b97146e893474da45d81708befa4f"].unchecked_into(),

			hex!["90b3514661b5b607a22fbbd72fae4e1a15b867a9df5cb8854a4ceb3a9b27bb39"].unchecked_into(),
		),
		(

			hex!["8af997028297e0b69be1bf436ac3a6dec6438badfdb281e637caff3c54d23642"].into(),

			hex!["5202312b14f1db2fdd90aa1865e95a95e72592042e6891dff753719b61b5f761"].unchecked_into(),

            hex!["b788335a95d0118828141ac70c34f462e014d73631801b8592ef27a0b9804a38"].unchecked_into(),

			hex!["66568462c1c2a90f388810d38bfc6ff1783fe6efffd3ac0762b9fc9700e96016"].unchecked_into(),

			hex!["bcd35266703747231ab338092a44b76b83a8a93cb4e7338323c038a83b2f9872"].unchecked_into(),

			hex!["a0845dab784052b4e8ce4090dcce2ad5a58a616aeca63dce4062213e18876374"].unchecked_into(),
		),
	];
	return initial_authorities;
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
    ]
}

fn tnf_testnet_ethereum_public_keys() -> Vec<EthPublicKey> {
    return vec![
        ecdsa::Public::from_slice(&hex![
            "02376fdd0add4a5ab1c3536422dd8647b729ad5d35ebeae5358fd54ac2ac7ce7d2"
        ])
        .unwrap(),
        ecdsa::Public::from_slice(&hex![
            "02f094c62de2f01a2f26bd1db153bcec9e57a3e94a97cd3fa3702fb4730d9084e4"
        ])
        .unwrap(),
        ecdsa::Public::from_slice(&hex![
            "036e83d53555e68cdb38f8f92be68c0610e08ebe7a6ef9c6ed5ac9dcdf575308a2"
        ])
        .unwrap(),
        ecdsa::Public::from_slice(&hex![
            "03ef2cfe2d40140b9de6b1af5b4d172e3538548bb8f3b55042294915dcbafe45fd"
        ])
        .unwrap(),
        ecdsa::Public::from_slice(&hex![
            "021473964134e3f5603ccb563dbafafff81e1047c7d7c8cd1cd62cd033f43697ef"
        ])
        .unwrap(),
    ]
}
