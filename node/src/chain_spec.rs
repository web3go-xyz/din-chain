use hex_literal::hex;

use din_runtime::{
	constants::currency::DOLLARS, opaque::SessionKeys, BabeConfig, BalancesConfig,
	EVMChainIdConfig, ImOnlineConfig, MaxNominations, RuntimeGenesisConfig, SessionConfig,
	StakerStatus, StakingConfig, SudoConfig, SystemConfig, WASM_BINARY,
};
use node_primitives::{AccountId, Balance, Signature};
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_chain_spec::Properties;
use sc_service::ChainType;
use sc_telemetry::TelemetryEndpoints;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public};
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	Perbill,
};

const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/ 0";

const DEFAULT_EVM_CHAIN_ID: u64 = 42;

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<RuntimeGenesisConfig>;

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

const DEFAULT_ENDOWED_ACCOUNT_BALANCE: Balance = 10_000_000 * DOLLARS;

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
	properties.insert("tokenSymbol".into(), "DIN".into());
	properties.insert("tokenDecimals".into(), 18.into());
	properties
}

pub fn chain_spec_dev() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;

	Ok(ChainSpec::from_genesis(
		// Name
		"din-dev",
		// ID
		"din-dev",
		ChainType::Development,
		move || {
			build_genesis(
				wasm_binary,
				// Initial authorities
				vec![authority_keys_from_seed("Alice"), authority_keys_from_seed("Bob")],
				// Initial nominators
				vec![],
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
			)
		},
		// Bootnodes
		vec![],
		// Telemetry
		None,
		// Protocol ID
		Some("din"),
		None,
		// Properties
		Some(default_properties()),
		// Extensions
		None,
	))
}

pub fn chain_spec_local() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.expect(
		"Development wasm binary is not available. This means the client is built with \
		 `SKIP_WASM_BUILD` flag and it is only usable for production chains. Please rebuild with \
		 the flag disabled.",
	);

	// SECRET="slow awkward present example safe bundle science ocean cradle word tennis earn" sh
	// scripts/prepare-test-net.sh 3
	let initial_authorities: Vec<(AccountId, AccountId, BabeId, GrandpaId, ImOnlineId)> = vec![
		(
			//5EvydUTtHvt39Khac3mMxNPgzcfu49uPDzUs3TL7KEzyrwbw
			hex!["7ecfd50629cdd246649959d88d490b31508db511487e111a52a392e6e458f518"].into(),
			//5HQyX5gyy77m9QLXguAhiwjTArHYjYspeY98dYDu1JDetfZg
			hex!["eca2cca09bdc66a7e6d8c3d9499a0be2ad4690061be8a9834972e17d13d2fe7e"].into(),
			//5G13qYRudTyttwTJvHvnwp8StFtcfigyPnwfD4v7LNopsnX4
			hex!["ae27367cb77850fb195fe1f9c60b73210409e68c5ad953088070f7d8513d464c"]
				.unchecked_into(),
			//5Eb7wM65PNgtY6e33FEAzYtU5cRTXt6WQvZTnzaKQwkVcABk
			hex!["6faae44b21c6f2681a7f60df708e9f79d340f7d441d28bd987fab8d05c6487e8"]
				.unchecked_into(),
			//5CdS2wGo4qdTQceVfEnbZH8vULeBrnGYCxSCxDna4tQSMV6y
			hex!["18f5d55f138bfa8e0ea26ed6fa56817b247de3c2e2030a908c63fb37c146473f"]
				.unchecked_into(),
		),
		(
			//5DFpvDUdCgw54E3E357GR1PyJe3Ft9s7Qyp7wbELAoJH9RQa
			hex!["34b7b3efd35fcc3c1926ca065381682b1af29b57dabbcd091042c6de1d541b7d"].into(),
			//5DZSSsND5wCjngvyXv27qvF3yPzt3MCU8rWnqNy4imqZmjT8
			hex!["4226796fa792ac78875e023ff2e30e3c2cf79f0b7b3431254cd0f14a3007bc0e"].into(),
			//5CPrgfRNDQvQSnLRdeCphP3ibj5PJW9ESbqj2fw29vBMNQNn
			hex!["0e9b60f04be3bffe362eb2212ea99d2b909b052f4bff7c714e13c2416a797f5d"]
				.unchecked_into(),
			//5FXFsPReTUEYPRNKhbTdUathcWBsxTNsLbk2mTpYdKCJewjA
			hex!["98f4d81cb383898c2c3d54dab28698c0f717c81b509cb32dc6905af3cc697b18"]
				.unchecked_into(),
			//5CDYSCJK91r8y2r1V4Ddrit4PFMEkwZXJe8mNBqGXJ4xWCWq
			hex!["06bd7dd4ab4c808c7d09d9cb6bd27fbcd99ad8400e99212b335056c475c24031"]
				.unchecked_into(),
		),
		(
			//5E2cob2jrXsBkTih56pizwSqENjE4siaVdXhaD6akLdDyVq7
			hex!["56e0f73c563d49ee4a3971c393e17c44eaa313dabad7fcf297dc3271d803f303"].into(),
			//5D4rNYgP9uFNi5GMyDEXTfiaFLjXyDEEX2VvuqBVi3f1qgCh
			hex!["2c58e5e1d5aef77774480cead4f6876b1a1a6261170166995184d7f86140572b"].into(),
			//5Ea2D65KXqe625sz4uV1jjhSfuigVnkezC8VgEj9LXN7ERAk
			hex!["6ed45cb7af613be5d88a2622921e18d147225165f24538af03b93f2a03ce6e13"]
				.unchecked_into(),
			//5G4kCbgqUhEyrRHCyFwFEkgBZXoYA8sbgsRxT9rY8Tp5Jj5F
			hex!["b0f8d2b9e4e1eafd4dab6358e0b9d5380d78af27c094e69ae9d6d30ca300fd86"]
				.unchecked_into(),
			//5HVhFBLFTKSZK9fX6RktckWDTgYNoSd33fgonsEC8zfr4ddm
			hex!["f03c3e184b2883eec9beaeb97f54321587e7476b228831ea0b5fc6da847ea975"]
				.unchecked_into(),
		),
	];

	// generated with secret
	// subkey inspect --scheme sr25519 "royal novel glad piece waste napkin little pioneer decline
	// fancy train sell"
	let root_key: AccountId = hex![
		// 5Fk6QsYKvDXxdXumGdHnNQ7V7FziREy6qn8WjDLEWF8WsbU3
		"a2bf32e50edd79c181888da41c80c67c191e9e6b29d3f2efb102ca0e2b53c558"
	]
	.into();

	let endowed_accounts: Vec<AccountId> = vec![root_key.clone()];

	let boot_nodes = vec![];

	Ok(ChainSpec::from_genesis(
		// Name
		"din-local",
		// ID
		"din-local",
		ChainType::Live,
		move || {
			build_genesis(
				wasm_binary,
				initial_authorities.clone(),
				vec![],
				root_key.clone(),
				endowed_accounts.clone(),
				true,
			)
		},
		boot_nodes,
		Some(
			TelemetryEndpoints::new(vec![(STAGING_TELEMETRY_URL.to_string(), 0)])
				.expect("Staging telemetry url is valid; qed"),
		),
		None,
		None,
		None,
		Default::default(),
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
	const ENDOWMENT: Balance = DEFAULT_ENDOWED_ACCOUNT_BALANCE;
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
		babe: BabeConfig {
			epoch_config: Some(din_runtime::BABE_GENESIS_EPOCH_CONFIG),
			..Default::default()
		},
		grandpa: Default::default(),
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
