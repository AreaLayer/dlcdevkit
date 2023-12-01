use crate::{
    error::{bdk_err_to_manager_err, esplora_err_to_manager_err},
    ErnestWallet,
};
use bdk::{
    bitcoin::{
        secp256k1::{PublicKey, Secp256k1},
        util::bip32::ChildNumber,
        Address, Network, Script, Txid,
    },
    blockchain::esplora::EsploraError,
};
use dlc_manager::error::Error as ManagerError;
use lightning::chain::chaininterface::{ConfirmationTarget, FeeEstimator};
use std::sync::atomic::Ordering;

impl FeeEstimator for ErnestWallet {
    fn get_est_sat_per_1000_weight(&self, confirmation_target: ConfirmationTarget) -> u32 {
        self.fees
            .get(&confirmation_target)
            .unwrap()
            .load(Ordering::Acquire)
    }
}

impl dlc_manager::Wallet for ErnestWallet {
    fn get_new_address(&self) -> Result<Address, ManagerError> {
        Ok(self
            .new_external_address()
            .map_err(bdk_err_to_manager_err)?
            .address)
    }

    fn get_new_secret_key(&self) -> Result<bitcoin::secp256k1::SecretKey, ManagerError> {
        let network_index = if self.network == Network::Bitcoin {
            ChildNumber::from_hardened_idx(0).unwrap()
        } else {
            ChildNumber::from_hardened_idx(1).unwrap()
        };

        let path = [
            ChildNumber::from_hardened_idx(420).unwrap(),
            network_index,
            ChildNumber::from_hardened_idx(0).unwrap(),
        ];

        let secp = Secp256k1::new();

        Ok(self.xprv.derive_priv(&secp, &path).unwrap().private_key)
    }

    fn import_address(&self, _address: &Address) -> Result<(), ManagerError> {
        // might be ok, might not
        Ok(())
    }

    // return all utxos
    // fixme
    fn get_utxos_for_amount(
        &self,
        _amount: u64,
        _fee_rate: Option<u64>,
        _lock_utxos: bool,
    ) -> Result<Vec<dlc_manager::Utxo>, ManagerError> {
        let wallet = self.inner.lock().unwrap();

        let local_utxos = wallet.list_unspent().map_err(bdk_err_to_manager_err)?;

        let dlc_utxos = local_utxos
            .iter()
            .map(|utxo| {
                let address =
                    Address::from_script(&utxo.txout.script_pubkey, self.network).unwrap();
                dlc_manager::Utxo {
                    tx_out: utxo.txout.clone(),
                    outpoint: utxo.outpoint,
                    address,
                    redeem_script: Script::new(),
                    reserved: false,
                }
            })
            .collect();

        Ok(dlc_utxos)
    }
}

impl dlc_manager::Signer for ErnestWallet {
    // Waiting for rust-dlc PR
    fn sign_tx_input(
        &self,
        _tx: &mut bitcoin::Transaction,
        _input_index: usize,
        _tx_out: &bitcoin::TxOut,
        _redeem_script: Option<bitcoin::Script>,
    ) -> Result<(), ManagerError> {
        Ok(())
    }

    fn get_secret_key_for_pubkey(
        &self,
        _pubkey: &PublicKey,
    ) -> Result<bitcoin::secp256k1::SecretKey, ManagerError> {
        let network_index = if self.network == Network::Bitcoin {
            ChildNumber::from_hardened_idx(0).unwrap()
        } else {
            ChildNumber::from_hardened_idx(1).unwrap()
        };

        let path = [
            ChildNumber::from_hardened_idx(420).unwrap(),
            network_index,
            ChildNumber::from_hardened_idx(0).unwrap(),
        ];

        let secp = Secp256k1::new();

        Ok(self.xprv.derive_priv(&secp, &path).unwrap().private_key)
    }
}

impl dlc_manager::Blockchain for ErnestWallet {
    fn get_network(&self) -> Result<bitcoin::network::constants::Network, ManagerError> {
        Ok(self.network)
    }

    fn get_transaction(&self, tx_id: &Txid) -> Result<bitcoin::Transaction, ManagerError> {
        let wallet = self.inner.lock().unwrap();

        let txn = wallet
            .get_tx(tx_id, false)
            .map_err(bdk_err_to_manager_err)?;

        match txn {
            Some(txn) => Ok(txn.transaction.unwrap()),
            None => Err(bdk_err_to_manager_err(bdk::Error::TransactionNotFound)),
        }
    }

    fn send_transaction(&self, transaction: &bitcoin::Transaction) -> Result<(), ManagerError> {
        Ok(self
            .blockchain
            .broadcast(transaction)
            .map_err(esplora_err_to_manager_err)?)
    }

    fn get_block_at_height(&self, height: u64) -> Result<bitcoin::Block, ManagerError> {
        let block_hash = self
            .blockchain
            .get_block_hash(height as u32)
            .map_err(esplora_err_to_manager_err)?;

        let block = self
            .blockchain
            .get_block_by_hash(&block_hash)
            .map_err(esplora_err_to_manager_err)?;

        match block {
            Some(block) => Ok(block),
            None => Err(esplora_err_to_manager_err(EsploraError::HttpResponse(404))),
        }
    }

    fn get_blockchain_height(&self) -> Result<u64, ManagerError> {
        Ok(self.blockchain.get_height().map_err(esplora_err_to_manager_err)? as u64)
    }

    fn get_transaction_confirmations(&self, tx_id: &Txid) -> Result<u32, ManagerError> {
        let txn = self
            .blockchain
            .get_tx_status(tx_id)
            .map_err(esplora_err_to_manager_err)?;
        let tip_height = self
            .blockchain
            .get_height()
            .map_err(esplora_err_to_manager_err)?;

        match txn {
            Some(txn) => {
                if txn.confirmed {
                    match txn.block_height {
                        Some(height) => Ok(tip_height - height),
                        None => Ok(0),
                    }
                } else {
                    Err(esplora_err_to_manager_err(
                        EsploraError::TransactionNotFound(*tx_id),
                    ))
                }
            }
            None => Err(esplora_err_to_manager_err(
                EsploraError::TransactionNotFound(*tx_id),
            )),
        }
    }
}