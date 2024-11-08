use crate::error::esplora_err_to_manager_err;
use bdk_esplora::esplora_client::Error as EsploraError;
use bdk_esplora::esplora_client::{AsyncClient, BlockingClient, Builder};
use bitcoin::Network;
use bitcoin::{Transaction, Txid};
use dlc_manager::error::Error as ManagerError;

/// Esplora client for getting chain information. Holds both a blocking
/// and an async client.
///
/// Used by rust-dlc for getting transactions related to DLC contracts.
/// Used by bdk to sync the wallet and track transaction.
pub struct EsploraClient {
    pub blocking_client: BlockingClient,
    pub async_client: AsyncClient,
    network: Network,
}

impl EsploraClient {
    pub fn new(esplora_host: &str, network: Network) -> anyhow::Result<EsploraClient> {
        let builder = Builder::new(esplora_host);
        let blocking_client = builder.clone().build_blocking();
        let async_client = builder.build_async()?;
        Ok(EsploraClient {
            blocking_client,
            async_client,
            network,
        })
    }
}

/// Implements the `dlc_manager::Blockchain` interface. Grabs chain related information
/// regarding DLC transactions.
impl dlc_manager::Blockchain for EsploraClient {
    fn get_network(&self) -> Result<Network, ManagerError> {
        Ok(self.network)
    }

    fn get_transaction(&self, tx_id: &Txid) -> Result<Transaction, ManagerError> {
        tracing::info!(txid = tx_id.to_string(), "Broadcasting transaction.");
        let txn = self
            .blocking_client
            .get_tx(tx_id)
            .map_err(esplora_err_to_manager_err)?;

        match txn {
            Some(txn) => Ok(txn),
            None => Err(esplora_err_to_manager_err(
                EsploraError::TransactionNotFound(*tx_id),
            )),
        }
    }

    fn send_transaction(&self, transaction: &bitcoin::Transaction) -> Result<(), ManagerError> {
        tracing::info!(
            txid = transaction.compute_txid().to_string(),
            num_inputs = transaction.input.len(),
            num_outputs = transaction.output.len(),
            "Broadcasting transaction."
        );

        if let Err(e) = self.blocking_client.broadcast(transaction) {
            tracing::error!(
                error =? e,
                "Could not broadcast transaction {}",
                transaction.compute_txid()
            );
            return Err(esplora_err_to_manager_err(e));
        }

        Ok(())
    }

    fn get_block_at_height(&self, height: u64) -> Result<bitcoin::Block, ManagerError> {
        tracing::info!(height, "Getting block at height.");
        let block_hash = self
            .blocking_client
            .get_block_hash(height as u32)
            .map_err(esplora_err_to_manager_err)?;

        let block = self
            .blocking_client
            .get_block_by_hash(&block_hash)
            .map_err(esplora_err_to_manager_err)?;

        match block {
            Some(block) => Ok(block),
            None => Err(esplora_err_to_manager_err(EsploraError::HttpResponse {
                status: 404,
                message: "Block not found in esplore".into(),
            })),
        }
    }

    fn get_blockchain_height(&self) -> Result<u64, ManagerError> {
        Ok(self
            .blocking_client
            .get_height()
            .map_err(esplora_err_to_manager_err)? as u64)
    }

    fn get_transaction_confirmations(&self, tx_id: &bitcoin::Txid) -> Result<u32, ManagerError> {
        tracing::info!(
            txid = tx_id.to_string(),
            "Getting transaction confirmations."
        );
        let txn = self
            .blocking_client
            .get_tx_status(tx_id)
            .map_err(esplora_err_to_manager_err)?;
        let tip_height = self
            .blocking_client
            .get_height()
            .map_err(esplora_err_to_manager_err)?;

        if txn.confirmed {
            match txn.block_height {
                Some(height) => Ok(tip_height - height),
                None => Ok(0),
            }
        } else {
            Ok(0)
        }
    }
}
