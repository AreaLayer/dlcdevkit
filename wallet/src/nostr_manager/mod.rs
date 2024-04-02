pub mod dlc_handler;
pub mod relay_handler;

pub use dlc_handler::NostrDlcHandler;
pub use nostr;
pub use nostr_relay_pool::RelayPoolNotification;
pub use nostr_sdk;

use crate::{wallet::ErnestWallet, RELAY_HOST};
use bdk::bitcoin::Network;
use relay_handler::NostrDlcRelayHandler;
use std::sync::Arc;

pub struct ErnestNostr {
    pub wallet: Arc<ErnestWallet>,
    pub relays: Arc<NostrDlcRelayHandler>,
}

impl ErnestNostr {
    pub fn new(name: &str, esplora_url: &str, network: Network) -> anyhow::Result<ErnestNostr> {
        let wallet = Arc::new(ErnestWallet::new(name, esplora_url, network)?);

        let relays = Arc::new(NostrDlcRelayHandler::new(name, RELAY_HOST)?);

        Ok(ErnestNostr { wallet, relays })
    }
}