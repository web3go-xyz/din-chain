use canbus_runtime::{opaque::Block, RuntimeApi};
use node_primitives::{AccountId, Balance, Nonce};
use sc_executor::{NativeElseWasmExecutor, NativeExecutionDispatch, NativeVersion};
use sp_consensus_babe::AuthorityId as BabeId;

use crate::eth::EthCompatRuntimeApiCollection;

pub type Client = FullClient<canbus_runtime::RuntimeApi, CanbusRuntimeExecutor>;

pub struct CanbusRuntimeExecutor;

/// A set of APIs that every runtimes must implement.
pub trait BaseRuntimeApiCollection:
	sp_api::ApiExt<Block>
	+ sp_api::Metadata<Block>
	+ sp_block_builder::BlockBuilder<Block>
	+ sp_offchain::OffchainWorkerApi<Block>
	+ sp_session::SessionKeys<Block>
	+ sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
{
}

impl<Api> BaseRuntimeApiCollection for Api where
	Api: sp_api::ApiExt<Block>
		+ sp_api::Metadata<Block>
		+ sp_block_builder::BlockBuilder<Block>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_session::SessionKeys<Block>
		+ sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
{
}

/// A set of APIs that template runtime must implement.
pub trait RuntimeApiCollection:
	BaseRuntimeApiCollection
	+ EthCompatRuntimeApiCollection
	+ sp_consensus_babe::BabeApi<Block>
	+ sp_consensus_grandpa::GrandpaApi<Block>
	+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce>
	+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
{
}

impl<Api> RuntimeApiCollection for Api where
	Api: BaseRuntimeApiCollection
		+ EthCompatRuntimeApiCollection
		+ sp_consensus_babe::BabeApi<Block>
		+ sp_consensus_grandpa::GrandpaApi<Block>
		+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce>
		+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
{
}
