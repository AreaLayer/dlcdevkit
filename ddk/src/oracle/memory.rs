use bitcoin::bip32::Xpriv;
use bitcoin::key::rand::Rng;
use bitcoin::secp256k1::schnorr::Signature;
use kormir::storage::{MemoryStorage, Storage};
use kormir::{Oracle as Kormir, OracleAttestation};

use crate::Oracle;

pub struct MemoryOracle {
    pub oracle: Kormir<MemoryStorage>,
}

impl Default for MemoryOracle {
    fn default() -> Self {
        let mut seed: [u8; 64] = [0; 64];
        bitcoin::key::rand::thread_rng().fill(&mut seed);
        let xpriv = Xpriv::new_master(bitcoin::Network::Regtest, &seed).unwrap();
        let oracle = Kormir::from_xpriv(MemoryStorage::default(), xpriv).unwrap();
        Self { oracle }
    }
}

impl Oracle for MemoryOracle {
    fn name(&self) -> String {
        "kormir".to_string()
    }
}

#[async_trait::async_trait]
impl ddk_manager::Oracle for MemoryOracle {
    fn get_public_key(&self) -> bitcoin::XOnlyPublicKey {
        self.oracle.public_key()
    }

    async fn get_announcement(
        &self,
        event_id: &str,
    ) -> Result<kormir::OracleAnnouncement, ddk_manager::error::Error> {
        Ok(self
            .oracle
            .storage
            .get_event(event_id.parse().unwrap())
            .await
            .unwrap()
            .unwrap()
            .announcement)
    }

    async fn get_attestation(
        &self,
        event_id: &str,
    ) -> Result<kormir::OracleAttestation, ddk_manager::error::Error> {
        let event = self
            .oracle
            .storage
            .get_event(event_id.parse().unwrap())
            .await
            .unwrap()
            .unwrap();

        let sigs = event
            .signatures
            .iter()
            .map(|sig| sig.1)
            .collect::<Vec<Signature>>();

        let outcomes = event
            .signatures
            .iter()
            .map(|outcome| outcome.0.clone())
            .collect::<Vec<String>>();

        Ok(OracleAttestation {
            event_id: event.announcement.oracle_event.event_id,
            oracle_public_key: self.oracle.public_key(),
            signatures: sigs,
            outcomes,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeDelta};
    use ddk_manager::Oracle;

    use super::*;

    #[tokio::test]
    async fn get_and_sign() {
        let oracle = MemoryOracle::default();
        let expiry = TimeDelta::seconds(15);
        let timestamp: u32 = Local::now()
            .checked_add_signed(expiry)
            .unwrap()
            .timestamp()
            .try_into()
            .unwrap();
        let (id, announcement) = oracle
            .oracle
            .create_enum_event(
                "event_id".into(),
                vec!["rust".into(), "go".into()],
                timestamp,
            )
            .await
            .unwrap();
        println!("create: {}", announcement.oracle_event.event_id);

        let ann = oracle.get_announcement(&format!("{id}")).await.unwrap();
        println!("get_announcement: {}", ann.oracle_event.event_id);

        assert_eq!(ann, announcement);

        let sign = oracle
            .oracle
            .sign_enum_event(id, "rust".to_string())
            .await
            .unwrap();

        println!("sign: {:?}", sign.event_id);

        let att = oracle.get_attestation(&format!("{id}")).await.unwrap();
        println!("get_attestation: {}", att.event_id);

        assert_eq!(sign, att);
    }
}
