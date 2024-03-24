use crate::{io, RELAY_URL};
use dlc_messages::{message_handler::read_dlc_message, Message, WireMessage};
use lightning::{
    ln::wire::Type,
    util::ser::{Readable, Writeable},
};
use nostr::{
    PublicKey,
    nips::nip04::{decrypt, encrypt}, secp256k1::{Parity, PublicKey as NostrPublicKey, Secp256k1, SecretKey, XOnlyPublicKey}, Event, EventBuilder, EventId, Filter, Keys, Kind, Tag, Timestamp, Url
};
use nostr_sdk::{Client, RelayPoolNotification};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::{path::Path, time::Duration};

pub const DLC_MESSAGE_KIND: Kind = Kind::Custom(8_888);
pub const ORACLE_ANNOUNCMENT_KIND: Kind = Kind::Custom(88);
pub const ORACLE_ATTESTATION_KIND: Kind = Kind::Custom(89);

pub struct NostrDlcHandler {
    pub keys: Keys,
    pub relay_url: Url,
    pub client: Client
}

impl NostrDlcHandler {
    pub fn new(
        wallet_name: &str,
        relay_url: String,
    ) -> anyhow::Result<NostrDlcHandler> {
        let keys_file = io::get_ernest_dir().join(wallet_name).join("nostr_keys");
        let keys = if Path::new(&keys_file).exists() {
            let secp = Secp256k1::new();
            let contents = std::fs::read(&keys_file)?;
            let secret_key = SecretKey::from_slice(&contents)?;
            Keys::new_with_ctx(&secp, secret_key.into())
        } else {
            let keys = Keys::generate();
            let secret_key = keys.secret_key()?;
            std::fs::write(keys_file, secret_key.secret_bytes())?;
            keys
        };

        let relay_url = relay_url.parse()?;
        let client = Client::new(&keys);

        Ok(NostrDlcHandler {
            keys,
            relay_url,
            client
        })
    }

    pub fn public_key(&self) -> PublicKey {
        self.keys.public_key()
    }

    pub fn create_dlc_message_filter(&self, since: Timestamp) -> Filter {
        Filter::new().kind(DLC_MESSAGE_KIND).since(since).pubkey(self.public_key())
    }

    pub fn create_oracle_message_filter(&self, since: Timestamp) -> Filter {
        Filter::new().kinds([ORACLE_ANNOUNCMENT_KIND, ORACLE_ATTESTATION_KIND]).since(since)
    }

    pub fn create_dlc_msg_event(
        &self,
        to: PublicKey,
        event_id: Option<EventId>,
        msg: Message,
    ) -> anyhow::Result<Event> {
        let mut bytes = msg.type_id().encode();
        bytes.extend(msg.encode());

        let content = encrypt(&self.keys.secret_key()?.clone(), &to, base64::encode(&bytes))?;

        let p_tags = Tag::PublicKey { public_key: self.public_key(), relay_url: None, alias: None, uppercase: false };

        let e_tags = event_id.map(|e| Tag::Event { event_id: e, relay_url: None, marker: None });

        let tags = [Some(p_tags), e_tags]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        let event = EventBuilder::new(DLC_MESSAGE_KIND, content, tags).to_event(&self.keys)?;

        Ok(event)
    }

    pub fn parse_dlc_msg_event(&self, event: &Event) -> anyhow::Result<Message> {
        let decrypt = decrypt(
            &self.keys.secret_key().unwrap(),
            &event.pubkey,
            &event.content,
        )?;

        let bytes = base64::decode(decrypt)?;

        let mut cursor = lightning::io::Cursor::new(bytes);

        let msg_type: u16 = Readable::read(&mut cursor).unwrap();

        let Some(wire) = read_dlc_message(msg_type, &mut cursor).unwrap() else {
            return Err(anyhow::anyhow!("Couldn't read DLC message."));
        };

        match wire {
            WireMessage::Message(msg) => Ok(msg),
            WireMessage::SegmentStart(_) | WireMessage::SegmentChunk(_) => {
                Err(anyhow::anyhow!("Blah blah, something with a wire"))
            }
        }
    }

    pub fn handle_dlc_msg_event(&self, event: Event) {
        match event.kind {
            Kind::Custom(89) => println!("Oracle attestation kind."),
            Kind::Custom(88) => println!("Oracle announcement kind."),
            Kind::Custom(8_888) => println!("DLC message."),
            _ => println!("unknown")
        }

        // let msg = self.parse_dlc_msg_event(&event)?;
        //
        // let pubkey = PublicKey::from_slice(
        //     &event
        //         .pubkey
        //         .public_key(nostr::secp256k1::Parity::Even)
        //         .serialize(),
        // )?;

        // let mut dlc = self.manager.lock().unwrap();
        //
        // let msg_opts = dlc.on_dlc_message(&msg, pubkey)?;
        //
        // if let Some(msg) = msg_opts {
        //     let event = self.create_dlc_msg_event(event.pubkey, Some(event.id), msg)?;
        //     return Ok(Some(event));
        // }

        // Ok(None)
    }

    pub async fn listen(&self) -> anyhow::Result<Client> {
        let client = Client::new(&self.keys);

        let since = Timestamp::now();

        client.add_relay(RELAY_URL).await?;

        let msg_subscription = self.create_dlc_message_filter(since);
        let oracle_subscription = self.create_oracle_message_filter(since);

        client.subscribe(vec![msg_subscription, oracle_subscription], None).await;

        client.connect().await;

        Ok(client)
    }
}
