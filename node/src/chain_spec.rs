use canbus_runtime::{
	constants::currency::DOLLARS, opaque::SessionKeys, BabeConfig, BalancesConfig,
	EVMChainIdConfig, GrandpaConfig, ImOnlineConfig, MaxNominations, NominationPoolsConfig,
	RuntimeGenesisConfig, SessionConfig, StakerStatus, StakingConfig, SudoConfig, SystemConfig,
	WASM_BINARY,
};
use node_primitives::{AccountId, Balance, Signature};
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_chain_spec::Properties;
use sc_service::ChainType;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	Perbill,
};

const DEFAULT_EVM_CHAIN_ID: u64 = 42;

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<RuntimeGenesisConfig>;

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

const DEFAULT_ENDOWED_ACCOUNT_BALANCE: Balance = 100 * DOLLARS;

type AccountPublic = <Signature as Verify>::Signer;

/// Generate an account ID from seed.
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate an Babe authority key.
pub fn authority_keys_from_seed(s: &str) -> (AccountId, AccountId, BabeId, GrandpaId, ImOnlineId) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", s)),
		get_account_id_from_seed::<sr25519::Public>(s),
		get_from_seed::<BabeId>(s),
		get_from_seed::<GrandpaId>(s),
		get_from_seed::<ImOnlineId>(s),
	)
}

fn session_keys(babe: BabeId, grandpa: GrandpaId, im_online: ImOnlineId) -> SessionKeys {
	SessionKeys { babe, grandpa, im_online }
}

/// Get default chain properties for Litentry which will be filled into chain spec
fn default_properties() -> Properties {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "CAN".into());
	properties.insert("tokenDecimals".into(), 18.into());
	properties
}

pub fn chain_spec_dev() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;

	Ok(ChainSpec::from_genesis(
		// Name
		"canbus-dev",
		// ID
		"canbus-dev",
		ChainType::Development,
		move || {
			build_genesis(
				wasm_binary,
				// Initial PoA authorities
				vec![authority_keys_from_seed("Alice"), authority_keys_from_seed("Bob")],
				// Initial nominators
				vec![],
				// Sudo account
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				// Pre-funded accounts
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
				],
				true,
			)
		},
		// Bootnodes
		vec![],
		// Telemetry
		None,
		// Protocol ID
		Some("canbus"),
		None,
		// Properties
		Some(default_properties()),
		// Extensions
		None,
	))
}

fn build_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AccountId, AccountId, BabeId, GrandpaId, ImOnlineId)>,
	initial_nominators: Vec<AccountId>,
	root_key: AccountId,
	mut endowed_accounts: Vec<AccountId>,
	_enable_println: bool,
) -> RuntimeGenesisConfig {
	// endow all authorities and nominators.
	initial_authorities
		.iter()
		.map(|x| &x.0)
		.chain(initial_nominators.iter())
		.for_each(|x| {
			if !endowed_accounts.contains(x) {
				endowed_accounts.push(x.clone())
			}
		});

	// stakers: all validators and nominators.
	const ENDOWMENT: Balance = 10_000_000 * DOLLARS;
	const STASH: Balance = ENDOWMENT / 1000;
	let mut rng = rand::thread_rng();
	let stakers = initial_authorities
		.iter()
		.map(|x| (x.0.clone(), x.1.clone(), STASH, StakerStatus::Validator))
		.chain(initial_nominators.iter().map(|x| {
			use rand::{seq::SliceRandom, Rng};
			let limit = (MaxNominations::get() as usize).min(initial_authorities.len());
			let count = rng.gen::<usize>() % limit;
			let nominations = initial_authorities
				.as_slice()
				.choose_multiple(&mut rng, count)
				.into_iter()
				.map(|choice| choice.0.clone())
				.collect::<Vec<_>>();
			(x.clone(), x.clone(), STASH, StakerStatus::Nominator(nominations))
		}))
		.collect::<Vec<_>>();

	RuntimeGenesisConfig {
		system: SystemConfig { code: wasm_binary.to_vec(), ..Default::default() },
		balances: BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, DEFAULT_ENDOWED_ACCOUNT_BALANCE))
				.collect(),
		},
		// TODO: Check initial authorities of babe and grandpa
		babe: BabeConfig {
			authorities: initial_authorities.iter().map(|x| (x.2.clone(), 1)).collect(),
			epoch_config: Some(canbus_runtime::BABE_GENESIS_EPOCH_CONFIG),
			..Default::default()
		},
		grandpa: GrandpaConfig {
			authorities: initial_authorities.iter().map(|x| (x.3.clone(), 1)).collect(),
			..Default::default()
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(x.0.clone(), x.0.clone(), session_keys(x.2.clone(), x.3.clone(), x.4.clone()))
				})
				.collect::<Vec<_>>(),
		},
		staking: StakingConfig {
			validator_count: initial_authorities.len() as u32,
			minimum_validator_count: initial_authorities.len() as u32,
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			slash_reward_fraction: Perbill::from_percent(10),
			stakers,
			..Default::default()
		},
		nomination_pools: NominationPoolsConfig {
			min_create_bond: 10 * DOLLARS,
			min_join_bond: 1 * DOLLARS,
			..Default::default()
		},
		sudo: SudoConfig {
			// Assign network admin rights.
			key: Some(root_key),
		},
		transaction_payment: Default::default(),
		im_online: ImOnlineConfig { keys: vec![] },
		// EVM compatibility
		evm_chain_id: EVMChainIdConfig { chain_id: DEFAULT_EVM_CHAIN_ID, ..Default::default() },
		ethereum: Default::default(),
		evm: Default::default(),
		base_fee: Default::default(),
		assets: Default::default(),
	}
}
