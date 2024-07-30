use din_runtime::{opaque::Block, RuntimeApi};
use sc_consensus_babe::{BabeWorkerHandle, SlotProportion};
use sc_executor::{NativeElseWasmExecutor, NativeExecutionDispatch, NativeVersion};
use std::{path::Path, sync::Arc, time::Duration};

use futures::prelude::*;
use sc_client_api::{Backend, BlockBackend};
use sc_consensus::BasicQueue;
use sc_network_sync::warp::WarpSyncParams;
use sc_service::{error::Error as ServiceError, Configuration, PartialComponents, TaskManager};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sc_transaction_pool_api::OffchainTransactionPoolFactory;
use sp_core::U256;

pub use crate::eth::{db_config_dir, EthConfiguration};
use crate::eth::{
	new_frontier_partial, spawn_frontier_tasks, BackendType, FrontierBackend, FrontierBlockImport,
	FrontierPartialComponents,
};
use din_runtime::{self, TransactionConverter};

type BasicImportQueue = sc_consensus::DefaultImportQueue<Block>;
type FullPool<Client> = sc_transaction_pool::FullPool<Block, Client>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;

//type GrandpaBlockImport<Client> =
//sc_consensus_grandpa::GrandpaBlockImport<FullBackend, Block, Client, FullSelectChain>;
//type GrandpaLinkHalf<Client> = sc_consensus_grandpa::LinkHalf<Block, Client, FullSelectChain>;
//type BoxBlockImport = sc_consensus::BoxBlockImport<Block>;

// Our native executor instance.
pub struct ExecutorDispatch;

impl NativeExecutionDispatch for ExecutorDispatch {
	/// Only enable the benchmarking host functions when we actually want to benchmark.
	#[cfg(feature = "runtime-benchmarks")]
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;
	/// Otherwise we only use the default Substrate host functions.
	#[cfg(not(feature = "runtime-benchmarks"))]
	type ExtendHostFunctions = ();

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		din_runtime::api::dispatch(method, data)
	}

	fn native_version() -> NativeVersion {
		din_runtime::native_version()
	}
}
/// Full backend.
pub type FullBackend = sc_service::TFullBackend<Block>;
/// Full client.
pub type FullClient =
	sc_service::TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<ExecutorDispatch>>;
/// Grandpa block import handler
type FullGrandpaBlockImport =
	sc_consensus_grandpa::GrandpaBlockImport<FullBackend, Block, FullClient, FullSelectChain>;
/// BABE block import handler (wraps `FullGrandpaBlockImport`)
type FullBabeBlockImport =
	sc_consensus_babe::BabeBlockImport<Block, FullClient, FullGrandpaBlockImport>;
/// Seed block import handler Frontier(Babe(Grandpa)))
type SeedBlockImport = FrontierBlockImport<Block, FullBabeBlockImport, FullClient>;
pub type ImportSetup = (
	SeedBlockImport,
	sc_consensus_grandpa::LinkHalf<Block, FullClient, FullSelectChain>,
	sc_consensus_babe::BabeLink<Block>,
);

/// The minimum period of blocks on which justifications will be
/// imported and generated.
const GRANDPA_JUSTIFICATION_PERIOD: u32 = 512;

#[allow(clippy::type_complexity)]
pub fn new_partial(
	//<BIQ>(
	config: &Configuration,
	eth_config: &EthConfiguration,
	//build_import_queue: BIQ,
) -> Result<
	PartialComponents<
		FullClient,
		FullBackend,
		FullSelectChain,
		BasicImportQueue,
		FullPool<FullClient>,
		(Option<Telemetry>, ImportSetup, FrontierBackend, BabeWorkerHandle<Block>),
	>,
	ServiceError,
>
//where
//	BIQ: FnOnce(
//		Arc<FullClient>,
//		&Configuration,
//		&EthConfiguration,
//		&TaskManager,
//		Option<TelemetryHandle>,
//		GrandpaBlockImport<FullClient>,
//	) -> Result<(BasicImportQueue, BoxBlockImport), ServiceError>,
{
	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let executor = sc_service::new_native_or_wasm_executor(config);

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, _>(
			config,
			telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
			executor,
		)?;
	let client = Arc::new(client);

	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", None, worker.run());
		telemetry
	});

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_essential_handle(),
		client.clone(),
	);

	let (grandpa_block_import, grandpa_link) = sc_consensus_grandpa::block_import(
		client.clone(),
		GRANDPA_JUSTIFICATION_PERIOD,
		&client,
		select_chain.clone(),
		telemetry.as_ref().map(|x| x.handle()),
	)?;

	let (block_import, babe_link) = sc_consensus_babe::block_import(
		sc_consensus_babe::configuration(&*client)?,
		grandpa_block_import.clone(),
		client.clone(),
	)?;

	let frontier_backend = match eth_config.frontier_backend_type {
		BackendType::KeyValue => FrontierBackend::KeyValue(fc_db::kv::Backend::open(
			Arc::clone(&client),
			&config.database,
			&db_config_dir(config),
		)?),
		BackendType::Sql => {
			let db_path = db_config_dir(config).join("sql");
			let overrides = crate::rpc::overrides_handle(client.clone());
			std::fs::create_dir_all(&db_path).expect("failed creating sql db directory");
			let backend = futures::executor::block_on(fc_db::sql::Backend::new(
				fc_db::sql::BackendConfig::Sqlite(fc_db::sql::SqliteBackendConfig {
					path: Path::new("sqlite:///")
						.join(db_path)
						.join("frontier.db3")
						.to_str()
						.unwrap(),
					create_if_missing: true,
					thread_count: eth_config.frontier_sql_backend_thread_count,
					cache_size: eth_config.frontier_sql_backend_cache_size,
				}),
				eth_config.frontier_sql_backend_pool_size,
				std::num::NonZeroU32::new(eth_config.frontier_sql_backend_num_ops_timeout),
				overrides.clone(),
			))
			.unwrap_or_else(|err| panic!("failed creating sql backend: {:?}", err));
			FrontierBackend::Sql(backend)
		},
	};

	let frontier_block_import = FrontierBlockImport::new(block_import.clone(), client.clone());

	let slot_duration = babe_link.config().slot_duration();

	let (import_queue, babe_worker_handle) =
		sc_consensus_babe::import_queue(sc_consensus_babe::ImportQueueParams {
			link: babe_link.clone(),
			block_import: frontier_block_import.clone(),
			justification_import: Some(Box::new(grandpa_block_import)),
			client: client.clone(),
			select_chain: select_chain.clone(),
			create_inherent_data_providers: move |_, ()| async move {
				let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

				let slot =
					sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
						*timestamp,
						slot_duration,
					);

				Ok((slot, timestamp))
			},
			spawner: &task_manager.spawn_essential_handle(),
			registry: config.prometheus_registry(),
			telemetry: telemetry.as_ref().map(|x| x.handle()),
			offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(transaction_pool.clone()),
		})?;

	let import_setup = (frontier_block_import, grandpa_link, babe_link);

	Ok(PartialComponents {
		client,
		backend,
		task_manager,
		import_queue,
		keystore_container,
		select_chain,
		transaction_pool,
		other: (telemetry, import_setup, frontier_backend, babe_worker_handle),
	})
}

//pub fn build_babe_grandpa_import_queue(
//	client: Arc<FullClient>,
//	config: &Configuration,
//	eth_config: &EthConfiguration,
//	task_manager: &TaskManager,
//	telemetry: Option<TelemetryHandle>,
//	grandpa_block_import: GrandpaBlockImport<FullClient>,
//	select_chain: FullSelectChain,
//	offchain_tx_pool_factory: OffchainTransactionPoolFactory<Block>,
//) -> Result<
//	((BasicImportQueue, BabeWorkerHandle<Block>), BoxBlockImport<Block>, BabeLink<Block>),
//	ServiceError,
//> {
//	// TODO should we use this instead of babe block import?
//	// let _frontier_block_import =
//	//     FrontierBlockImport::new(grandpa_block_import.clone(), client.clone());
//
//	let (block_import, babe_link) = sc_consensus_babe::block_import(
//		sc_consensus_babe::configuration(&*client)?,
//		grandpa_block_import.clone(),
//		client.clone(),
//	)?;
//
//	let slot_duration = babe_link.config().slot_duration();
//	let justification_import = grandpa_block_import;
//	let target_gas_price = eth_config.target_gas_price;
//	let import_queue = sc_consensus_babe::import_queue(sc_consensus_babe::ImportQueueParams {
//		link: babe_link.clone(),
//		block_import: block_import.clone(),
//		justification_import: Some(Box::new(justification_import)),
//		client: client.clone(),
//		select_chain,
//		create_inherent_data_providers: move |_, ()| async move {
//			let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
//
//			let slot =
//
// sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
// *timestamp,                    slot_duration,
//                );
//			let dynamic_fee = fp_dynamic_fee::InherentDataProvider(U256::from(target_gas_price));
//
//			Ok((slot, timestamp, dynamic_fee))
//		},
//		spawner: &task_manager.spawn_essential_handle(),
//		registry: config.prometheus_registry(),
//		telemetry,
//		offchain_tx_pool_factory,
//	})?;
//
//	Ok((import_queue, Box::new(block_import), babe_link))
//}

/// Builds a new service for a full client.
pub async fn new_full(
	mut config: Configuration,
	eth_config: EthConfiguration,
) -> Result<TaskManager, ServiceError> {
	let PartialComponents {
		client,
		backend,
		mut task_manager,
		import_queue,
		keystore_container,
		select_chain,
		transaction_pool,
		other: (mut telemetry, import_setup, frontier_backend, babe_worker_handle),
	} = new_partial(&config, &eth_config)?;

	let (block_import, grandpa_link, babe_link) = import_setup;

	let FrontierPartialComponents { filter_pool, fee_history_cache, fee_history_cache_limit } =
		new_frontier_partial(&eth_config)?;

	let mut net_config = sc_network::config::FullNetworkConfiguration::new(&config.network);

	let grandpa_protocol_name = sc_consensus_grandpa::protocol_standard_name(
		&client.block_hash(0).ok().flatten().expect("Genesis block exists; qed"),
		&config.chain_spec,
	);
	net_config.add_notification_protocol(sc_consensus_grandpa::grandpa_peers_set_config(
		grandpa_protocol_name.clone(),
	));

	let warp_sync = Arc::new(sc_consensus_grandpa::warp_proof::NetworkProvider::new(
		backend.clone(),
		grandpa_link.shared_authority_set().clone(),
		Vec::default(),
	));

	let (network, system_rpc_tx, tx_handler_controller, network_starter, sync_service) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			net_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			block_announce_validator_builder: None,
			warp_sync_params: Some(WarpSyncParams::WithProvider(warp_sync)),
			block_relay: None,
		})?;

	if config.offchain_worker.enabled {
		task_manager.spawn_handle().spawn(
			"offchain-workers-runner",
			"offchain-worker",
			sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
				runtime_api_provider: client.clone(),
				is_validator: config.role.is_authority(),
				keystore: Some(keystore_container.keystore()),
				offchain_db: backend.offchain_storage(),
				transaction_pool: Some(OffchainTransactionPoolFactory::new(
					transaction_pool.clone(),
				)),
				network_provider: network.clone(),
				enable_http_requests: true,
				custom_extensions: |_| vec![],
			})
			.run(client.clone(), task_manager.spawn_handle())
			.boxed(),
		);
	}

	let role = config.role.clone();
	let force_authoring = config.force_authoring;
	let backoff_authoring_blocks =
		Some(sc_consensus_slots::BackoffAuthoringOnFinalizedHeadLagging::default());
	let name = config.network.node_name.clone();
	let enable_grandpa = !config.disable_grandpa;
	let prometheus_registry = config.prometheus_registry().cloned();
	let overrides = crate::rpc::overrides_handle(client.clone());

	// Sinks for pubsub notifications.
	// Everytime a new subscription is created, a new mpsc channel is added to the sink pool.
	// The MappingSyncWorker sends through the channel on block import and the subscription emits a
	// notification to the subscriber on receiving a message through this channel. This way we avoid
	// race conditions when using native substrate block import notification stream.
	let pubsub_notification_sinks: fc_mapping_sync::EthereumBlockNotificationSinks<
		fc_mapping_sync::EthereumBlockNotification<Block>,
	> = Default::default();
	let pubsub_notification_sinks = Arc::new(pubsub_notification_sinks);

	// for ethereum-compatibility rpc.
	config.rpc_id_provider = Some(Box::new(fc_rpc::EthereumSubIdProvider));

	let rpc_builder = {
		let client = client.clone();
		let pool = transaction_pool.clone();
		let select_chain = select_chain.clone();
		let network = network.clone();
		let sync_service = sync_service.clone();

		let is_authority = role.is_authority();
		let enable_dev_signer = eth_config.enable_dev_signer;
		let max_past_logs = eth_config.max_past_logs;
		let execute_gas_limit_multiplier = eth_config.execute_gas_limit_multiplier;
		let filter_pool = filter_pool.clone();
		let frontier_backend = frontier_backend.clone();
		let pubsub_notification_sinks = pubsub_notification_sinks.clone();
		let overrides = overrides.clone();
		let fee_history_cache = fee_history_cache.clone();
		let block_data_cache = Arc::new(fc_rpc::EthBlockDataCacheTask::new(
			task_manager.spawn_handle(),
			overrides.clone(),
			eth_config.eth_log_block_cache,
			eth_config.eth_statuses_cache,
			prometheus_registry.clone(),
		));

		let slot_duration = babe_link.config().slot_duration();
		let babe_config = babe_link.config().clone();
		let keystore = keystore_container.keystore().clone();
		let target_gas_price = eth_config.target_gas_price;
		let justification_stream = grandpa_link.justification_stream();
		let shared_authority_set = grandpa_link.shared_authority_set().clone();
		let finality_proof_provider = sc_consensus_grandpa::FinalityProofProvider::new_for_service(
			backend.clone(),
			Some(shared_authority_set.clone()),
		);
		let pending_create_inherent_data_providers = move |_, ()| async move {
			let current = sp_timestamp::InherentDataProvider::from_system_time();
			let next_slot = current.timestamp().as_millis() + slot_duration.as_millis();
			let timestamp = sp_timestamp::InherentDataProvider::new(next_slot.into());
			let slot = sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
				*timestamp,
				slot_duration,
			);
			let dynamic_fee = fp_dynamic_fee::InherentDataProvider(U256::from(target_gas_price));
			Ok((slot, timestamp, dynamic_fee))
		};

		Box::new(
			move |deny_unsafe, subscription_task_executor: sc_rpc::SubscriptionTaskExecutor| {
				let eth_deps = crate::rpc::EthDeps {
					client: client.clone(),
					pool: pool.clone(),
					graph: pool.pool().clone(),
					converter: Some(TransactionConverter),
					is_authority,
					enable_dev_signer,
					network: network.clone(),
					sync: sync_service.clone(),
					frontier_backend: match frontier_backend.clone() {
						fc_db::Backend::KeyValue(b) => Arc::new(b),
						fc_db::Backend::Sql(b) => Arc::new(b),
					},
					overrides: overrides.clone(),
					block_data_cache: block_data_cache.clone(),
					filter_pool: filter_pool.clone(),

					max_past_logs,
					fee_history_cache: fee_history_cache.clone(),
					fee_history_cache_limit,
					execute_gas_limit_multiplier,
					forced_parent_hashes: None,
					pending_create_inherent_data_providers,
				};
				let shared_voter_state = sc_consensus_grandpa::SharedVoterState::empty();

				let deps = crate::rpc::FullDeps {
					client: client.clone(),
					pool: pool.clone(),
					select_chain: select_chain.clone(),
					deny_unsafe,
					eth: eth_deps,
					babe: crate::rpc::BabeDeps {
						babe_config: babe_config.clone(),
						babe_worker_handle: babe_worker_handle.clone(),
						keystore: keystore.clone(),
					},
					grandpa: crate::rpc::GrandpaDeps {
						shared_voter_state: shared_voter_state.clone(),
						shared_authority_set: shared_authority_set.clone(),
						justification_stream: justification_stream.clone(),
						subscription_executor: subscription_task_executor.clone(),
						finality_provider: finality_proof_provider.clone(),
					},
				};
				crate::rpc::create_full(
					deps,
					subscription_task_executor,
					pubsub_notification_sinks.clone(),
				)
				.map_err(Into::into)
			},
		)
	};

	let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		config,
		client: client.clone(),
		backend: backend.clone(),
		task_manager: &mut task_manager,
		keystore: keystore_container.keystore(),
		transaction_pool: transaction_pool.clone(),
		rpc_builder,
		network: network.clone(),
		system_rpc_tx,
		tx_handler_controller,
		sync_service: sync_service.clone(),
		telemetry: telemetry.as_mut(),
	})?;

	spawn_frontier_tasks(
		&task_manager,
		client.clone(),
		backend,
		frontier_backend,
		filter_pool,
		overrides,
		fee_history_cache,
		fee_history_cache_limit,
		sync_service.clone(),
		pubsub_notification_sinks,
	)
	.await;

	if role.is_authority() {
		let proposer_factory = sc_basic_authorship::ProposerFactory::new(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool.clone(),
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|x| x.handle()),
		);

		let slot_duration = babe_link.config().slot_duration();
		//let target_gas_price = eth_config.target_gas_price;

		let babe_config = sc_consensus_babe::BabeParams {
			keystore: keystore_container.keystore(),
			client: client.clone(),
			select_chain,
			env: proposer_factory,
			block_import: block_import.clone(),
			sync_oracle: sync_service.clone(),
			justification_sync_link: sync_service.clone(),
			create_inherent_data_providers: move |parent, ()| {
				let client_clone = client.clone();
				async move {
					let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

					let slot =
						sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
							*timestamp,
							slot_duration,
						);

					// NOTE - check if we can remove this
					let storage_proof =
						sp_transaction_storage_proof::registration::new_data_provider(
							&*client_clone,
							&parent,
						)?;

					// TODO should we use this ?
					// let dynamic_fee =
					// fp_dynamic_fee::InherentDataProvider(U256::from(target_gas_price)); Ok((slot,
					// timestamp, dynamic_fee))
					Ok((slot, timestamp, storage_proof))
				}
			},
			force_authoring,
			backoff_authoring_blocks,
			babe_link,
			block_proposal_slot_portion: SlotProportion::new(0.5),
			max_block_proposal_slot_portion: None,
			telemetry: telemetry.as_ref().map(|x| x.handle()),
		};

		let babe = sc_consensus_babe::start_babe(babe_config)?;

		task_manager.spawn_essential_handle().spawn_blocking(
			"babe-proposer",
			Some("block-authoring"),
			babe,
		);
	}

	if enable_grandpa {
		// if the node isn't actively participating in consensus then it doesn't
		// need a keystore, regardless of which protocol we use below.
		let keystore = if role.is_authority() { Some(keystore_container.keystore()) } else { None };

		let grandpa_config = sc_consensus_grandpa::Config {
			// FIXME #1578 make this available through chainspec
			gossip_duration: Duration::from_millis(333),
			justification_generation_period: GRANDPA_JUSTIFICATION_PERIOD,
			name: Some(name),
			observer_enabled: false,
			keystore,
			local_role: role,
			telemetry: telemetry.as_ref().map(|x| x.handle()),
			protocol_name: grandpa_protocol_name,
		};

		// start the full GRANDPA voter
		// NOTE: non-authorities could run the GRANDPA observer protocol, but at
		// this point the full voter should provide better guarantees of block
		// and vote data availability than the observer. The observer has not
		// been tested extensively yet and having most nodes in a network run it
		// could lead to finality stalls.
		let grandpa_config = sc_consensus_grandpa::GrandpaParams {
			config: grandpa_config,
			link: grandpa_link,
			network,
			sync: sync_service,
			voting_rule: sc_consensus_grandpa::VotingRulesBuilder::default().build(),
			prometheus_registry,
			shared_voter_state: sc_consensus_grandpa::SharedVoterState::empty(),
			telemetry: telemetry.as_ref().map(|x| x.handle()),
			offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(transaction_pool),
		};

		// the GRANDPA voter task is considered infallible, i.e.
		// if it fails we take down the service with it.
		task_manager.spawn_essential_handle().spawn_blocking(
			"grandpa-voter",
			None,
			sc_consensus_grandpa::run_grandpa_voter(grandpa_config)?,
		);
	}

	network_starter.start_network();
	Ok(task_manager)
}

pub fn new_chain_ops(
	config: &mut Configuration,
	eth_config: &EthConfiguration,
) -> Result<
	(Arc<FullClient>, Arc<FullBackend>, BasicQueue<Block>, TaskManager, FrontierBackend),
	ServiceError,
> {
	config.keystore = sc_service::config::KeystoreConfig::InMemory;
	let PartialComponents { client, backend, import_queue, task_manager, other, .. } =
		new_partial(config, eth_config)?;
	Ok((client, backend, import_queue, task_manager, other.2))
}
