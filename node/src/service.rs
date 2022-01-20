//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use codec::Encode;
use rand::RngCore;
use sc_client_api::ExecutorProvider;
use sc_consensus::LongestChain;
use sc_consensus_pow::PowBlockImport;
use sc_executor::NativeElseWasmExecutor;
use sc_keystore::LocalKeystore;
use sc_service::{error::Error as ServiceError, Configuration, TFullCallExecutor, TaskManager};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sha3::Digest;
use sp_consensus::CanAuthorWithNativeVersion;
use sp_inherents::CreateInherentDataProviders;
use sp_runtime::{traits::IdentifyAccount, MultiSigner};
use std::thread;
use std::{sync::Arc, time::Duration};
use sybil_runtime::{self, opaque::Block, RuntimeApi};

pub type Executor = sc_executor::NativeElseWasmExecutor<ExecutorDispatch>;
pub struct ExecutorDispatch;

impl sc_executor::NativeExecutionDispatch for ExecutorDispatch {
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		sybil_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		sybil_runtime::native_version()
	}
}

type FullClient = sc_service::TFullClient<Block, RuntimeApi, Executor>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;
type SybilPowBlockImport = PowBlockImport<
	Block,
	Arc<FullClient>,
	FullClient,
	LongestChain<FullBackend, Block>,
	sybil_pow::SybilPow<FullClient>,
	CanAuthorWithNativeVersion<TFullCallExecutor<Block, Executor>>,
	Box<
		dyn CreateInherentDataProviders<
			Block,
			(),
			InherentDataProviders = sp_timestamp::InherentDataProvider,
		>,
	>,
>;
pub fn new_partial(
	config: &Configuration,
) -> Result<
	sc_service::PartialComponents<
		FullClient,
		FullBackend,
		FullSelectChain,
		sc_consensus::DefaultImportQueue<Block, FullClient>,
		sc_transaction_pool::FullPool<Block, FullClient>,
		(SybilPowBlockImport, Option<Telemetry>),
	>,
	ServiceError,
> {
	if config.keystore_remote.is_some() {
		return Err(ServiceError::Other(format!("Remote Keystores are not supported.")));
	}

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

	let executor = NativeElseWasmExecutor::<ExecutorDispatch>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
		config.runtime_cache_size
	);

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, Executor>(
			&config,
			telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
			executor,
		)?;
	let client = Arc::new(client);

	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", None, worker.run());
		telemetry
	});

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let can_author_with = sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());

	let block_import = sc_consensus_pow::PowBlockImport::new(
		client.clone(),
		client.clone(),
		sybil_pow::SybilPow::new(client.clone()),
		0,
		select_chain.clone(),
		Box::new(move |_, ()| async move {
			let provider = sp_timestamp::InherentDataProvider::from_system_time();
			Ok(provider)
		})
			as Box<
				dyn CreateInherentDataProviders<
					Block,
					(),
					InherentDataProviders = sp_timestamp::InherentDataProvider,
				>,
			>,
		can_author_with,
	);

	let import_queue = sc_consensus_pow::import_queue(
		Box::new(block_import.clone()),
		None,
		sybil_pow::SybilPow::new(client.clone()),
		&task_manager.spawn_essential_handle(),
		None,
	)?;

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_essential_handle(),
		client.clone(),
	);

	Ok(sc_service::PartialComponents {
		client,
		backend,
		task_manager,
		import_queue,
		keystore_container,
		select_chain,
		transaction_pool,
		other: (block_import, telemetry),
	})
}

fn remote_keystore(_url: &String) -> Result<Arc<LocalKeystore>, &'static str> {
	// FIXME: here would the concrete keystore be built,
	//        must return a concrete type (NOT `LocalKeystore`) that
	//        implements `CryptoStore` and `SyncCryptoStore`
	Err("Remote Keystore not supported.")
}

/// Builds a new service for a full client.
pub fn new_full(config: Configuration, threads: usize) -> Result<TaskManager, ServiceError> {
	let sc_service::PartialComponents {
		client,
		backend,
		mut task_manager,
		import_queue,
		mut keystore_container,
		select_chain: _,
		transaction_pool,
		other: (pow_block_import, mut telemetry),
	} = new_partial(&config)?;

	if let Some(url) = &config.keystore_remote {
		match remote_keystore(url) {
			Ok(k) => keystore_container.set_remote_keystore(k),
			Err(e) => {
				return Err(ServiceError::Other(format!(
					"Error hooking up remote keystore for {}: {}",
					url, e
				)))
			}
		};
	}

	let (network, system_rpc_tx, network_starter) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			block_announce_validator_builder: None,
			warp_sync: None,
		})?;

	if config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&config,
			task_manager.spawn_handle(),
			client.clone(),
			network.clone(),
		);
	}

	let role = config.role.clone();
	let prometheus_registry = config.prometheus_registry().cloned();

	let rpc_extensions_builder = {
		let client = client.clone();
		let pool = transaction_pool.clone();

		Box::new(move |deny_unsafe, _| {
			let deps =
				crate::rpc::FullDeps { client: client.clone(), pool: pool.clone(), deny_unsafe };

			Ok(crate::rpc::create_full(deps))
		})
	};

	let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		network: network.clone(),
		client: client.clone(),
		keystore: keystore_container.sync_keystore(),
		task_manager: &mut task_manager,
		transaction_pool: transaction_pool.clone(),
		rpc_extensions_builder,
		backend: backend.clone(),
		system_rpc_tx,
		config,
		telemetry: telemetry.as_mut(),
	})?;

	if role.is_authority() {
		let proposer_factory = sc_basic_authorship::ProposerFactory::new(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool,
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|x| x.handle()),
		);

		let can_author_with =
			sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());
		let select_chain = sc_consensus::LongestChain::new(backend.clone());

		let address = MultiSigner::from(sp_keyring::Sr25519Keyring::Alice.public())
			.into_account()
			.encode();

		let (worker, authorship_task) = sc_consensus_pow::start_mining_worker(
			Box::new(pow_block_import),
			client.clone(),
			select_chain,
			sybil_pow::SybilPow::new(client.clone()),
			proposer_factory,
			network.clone(),
			network.clone(),
			Some(address),
			move |_, ()| async move {
				let provider = sp_timestamp::InherentDataProvider::from_system_time();
				Ok(provider)
			},
			Duration::from_secs(10),
			Duration::from_secs(10),
			can_author_with,
		);

		for _ in 0..threads {
			let worker = worker.clone();

			thread::spawn(move || {
				let mut version = worker.version();
				let mut metadata = worker.metadata();

				loop {
					if version != worker.version() {
						version = worker.version();

						metadata = worker.metadata();
					}
					if let Some(ref metadata) = metadata {
						let mut rand = rand::thread_rng();

						let mut nonce = sp_core::H256::default();
						rand.fill_bytes(&mut nonce[..]);
						let compute = sybil_pow::Compute {
							pre_hash: metadata.pre_hash.clone(),
							difficulty: metadata.difficulty.clone(),
							nonce,
						};

						let work =
							sp_core::U256::from(&*sha3::Sha3_256::digest(&compute.encode()[..]));

						let difficulty = metadata.difficulty.clone();
						let (_, overflowed) = work.overflowing_mul(difficulty);

						if !overflowed {
							let seal = sybil_pow::SybilSeal { nonce, difficulty };

							futures::executor::block_on(worker.submit(seal.encode()));
						}
					} else {
						thread::sleep(Duration::new(1, 0));
					}
					std::hint::spin_loop()
				}
			});
		}

		task_manager.spawn_essential_handle().spawn("mining-task", None, authorship_task);
	}

	network_starter.start_network();
	Ok(task_manager)
}
