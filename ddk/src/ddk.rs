use crate::chain::EsploraClient;
use crate::wallet::DlcDevKitWallet;
use crate::{DdkOracle, DdkStorage, DdkTransport};
use bdk::chain::PersistBackend;
use bdk::wallet::ChangeSet;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::{
    contract::contract_input::ContractInput, CachedContractSignerProvider, ContractId,
    SimpleSigner, SystemTimeProvider,
};
use dlc_messages::oracle_msgs::OracleAnnouncement;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub type DlcDevKitDlcManager<S, O, WS> = dlc_manager::manager::Manager<
    Arc<DlcDevKitWallet<WS>>,
    Arc<CachedContractSignerProvider<Arc<DlcDevKitWallet<WS>>, SimpleSigner>>,
    Arc<EsploraClient>,
    Arc<S>,
    Arc<O>,
    Arc<SystemTimeProvider>,
    Arc<DlcDevKitWallet<WS>>,
    SimpleSigner,
>;

pub struct DlcDevKit<T: DdkTransport, S: DdkStorage, O: DdkOracle, WS: PersistBackend<ChangeSet>> {
    pub wallet: Arc<DlcDevKitWallet<WS>>,
    pub manager: Arc<Mutex<DlcDevKitDlcManager<S, O, WS>>>,
    pub transport: Arc<T>,
    pub storage: Arc<S>,
    pub oracle: Arc<O>,
}

impl<
        T: DdkTransport + std::marker::Send + std::marker::Sync + 'static,
        S: DdkStorage,
        O: DdkOracle,
        WS: PersistBackend<ChangeSet> + std::marker::Send + Clone + 'static
    > DlcDevKit<T, S, O, WS>
{
    pub async fn start(&self) -> anyhow::Result<()> {
        tracing::info!("Starting ddk...");
        let transport_listener = self.transport.clone();
        let wallet = self.wallet.clone();
        let _dlc_manager = self.manager.clone();

        tokio::spawn(async move {
            transport_listener.listen().await;
        });
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(10));
            loop {
                timer.tick().await;
                log::info!("Syncing wallet...");
                wallet.sync().unwrap();
            }
        });

        let _transport_clone = self.transport.clone();
        tokio::spawn(async move {
            // transport_clone.receive_dlc_message(&dlc_manager).await;
        });

        Ok(())
    }

    pub fn transport_type(&self) -> String {
        self.transport.name()
    }

    pub async fn send_dlc_offer(
        &self,
        contract_input: &ContractInput,
        oracle_announcement: &OracleAnnouncement,
        counter_party: PublicKey,
    ) -> anyhow::Result<()> {
        let mut manager = self.manager.lock().await;

        let _offer_msg = manager.send_offer_with_announcements(
            contract_input,
            counter_party,
            vec![vec![oracle_announcement.clone()]],
        )?;

        Ok(())
    }

    pub async fn accept_dlc_offer(&self, contract: [u8; 32]) -> anyhow::Result<()> {
        let mut dlc = self.manager.lock().await;

        let contract_id = ContractId::from(contract);

        tracing::info!("Before accept: {:?}", contract_id);
        let (_, _public_key, _accept_dlc) = dlc.accept_contract_offer(&contract_id)?;

        tracing::info!("Accepted");

        Ok(())
    }
}
