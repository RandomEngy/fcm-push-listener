use std::{sync::Arc, str};
use ece::EcKeyComponents;
use log::{debug, warn};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_rustls::{rustls, TlsConnector, client::TlsStream};
use prost::Message;
use base64::{Engine as _, engine::general_purpose::{URL_SAFE_NO_PAD, URL_SAFE}};
use std::time::Instant;

use crate::{Error, Registration, gcm};

const MCS_VERSION: u8 = 41;

pub mod mcs {
    include!(concat!(env!("OUT_DIR"), "/mcs_proto.rs"));
}

// Message tags, for reference
//     HeartbeatPing       = 0,
//     HeartbeatAck        = 1,
//     LoginRequest        = 2,
//     LoginResponse       = 3,
//     Close               = 4,
//     MessageStanza       = 5,
//     PresenceStanza      = 6,
//     IqStanza            = 7,
//     DataMessageStanza   = 8,
//     BatchPresenceStanza = 9,
//     StreamErrorStanza   = 10,
//     HttpRequest         = 11,
//     HttpResponse        = 12,
//     BindAccountRequest  = 13,
//     BindAccountResponse = 14,
//     TalkMetadata        = 15,
//     NumProtoTypes       = 16,

const HEARTBEAT_PING_TAG: u8 = 0;
const HEARTBEAT_ACK_TAG: u8 = 1;
const LOGIN_REQUEST_TAG: u8 = 2;
const LOGIN_RESPONSE_TAG: u8 = 3;
const CLOSE_TAG: u8 = 4;
const DATA_MESSAGE_STANZA_TAG: u8 = 8;

pub struct FcmPushListener<FMessage : Fn(FcmMessage)> {
    registration: Registration,
    message_callback: FMessage,
    received_persistent_ids: Vec<String>,
}

pub struct FcmMessage {
    pub payload_json: String,
    pub persistent_id: Option<String>,
}

impl<FMessage> FcmPushListener<FMessage>
    where FMessage : Fn(FcmMessage) {
    pub fn create(
        registration: Registration,
        message_callback: FMessage,
        received_persistent_ids: Vec<String>) -> Self {
        FcmPushListener {
            registration,
            message_callback,
            received_persistent_ids,
        }
    }

    pub async fn connect(&mut self) -> Result<(), Error> {
        loop {
            let start = Instant::now();
            let result = self.connect_internal().await;
            let elapsed = start.elapsed();

            // If we quickly disconnected, propagate the error
            if elapsed.as_secs() < 20 {
                return result;
            }

            warn!("Connection failed. Retrying.");
            // Otherwise, try to connect again.
        }
    }

    async fn connect_internal(&mut self) -> Result<(), Error> {
        // First check in to let GCM know the device is still functioning
        gcm::check_in(Some(self.registration.gcm.android_id), Some(self.registration.gcm.security_token)).await?;

        let mut root_store = rustls::RootCertStore::empty();
        root_store.add_server_trust_anchors(
            webpki_roots::TLS_SERVER_ROOTS
                .0
                .iter()
                .map(|ta| {
                    rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                        ta.subject,
                        ta.spki,
                        ta.name_constraints,
                    )
                })
        );
    
        let config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store)
            .with_no_client_auth();
    
        let connector = TlsConnector::from(Arc::new(config));
        let server_name = "mtalk.google.com".try_into().expect("Google talk server name should resolve");
    
        let stream = TcpStream::connect("mtalk.google.com:5228").await?;
        let mut stream = connector.connect(server_name, stream).await?;
    
        let login_request = self.create_login_request();
    
        let initial_bytes: Vec<u8> = vec![MCS_VERSION, LOGIN_REQUEST_TAG];
        stream.write_all(&initial_bytes).await?;
    
        // Login bytes are preceded by a varint indicating their length
        let login_request_bytes = login_request.encode_length_delimited_to_vec();
        stream.write_all(&login_request_bytes).await?;
    
        // Buffer the reads to reduce sys calls
        let mut buffered_reader = BufReader::new(stream);
    
        // Read the version
        buffered_reader.read_i8().await?;
    
        // Read messages
        self.read_message_loop(buffered_reader).await?;
    
        Ok(())
    }

    async fn read_message_loop<'a>(&mut self, mut stream: BufReader<TlsStream<TcpStream>>) -> Result<(), Error> {
        loop {
            let tag: u8 = stream.read_u8().await?;

            debug!("Push message listener read tag: {}", tag);

            if tag == CLOSE_TAG {
                break;
            }

            // The tag is followed by a varint that indicates the message size
            // See https://protobuf.dev/programming-guides/encoding/#varints

            let mut size: usize = 0;
            let mut size_byte_shift: u32 = 0;
            loop {
                let size_byte = stream.read_u8().await?;

                // Strip the continuation bit
                let size_byte_payload = size_byte & 0x7f;

                // Least significant bytes are given first, so we shift left
                // further as we continue to read size bytes.
                let size_byte_amount = (size_byte_payload as usize) << size_byte_shift;

                // Add the 7 payload bytes to the size
                size += size_byte_amount;

                if size_byte & 0x80 > 0 {
                    // The continuation bit is set. Keep on reading bytes to determine the size.
                    size_byte_shift += 7;
                } else {
                    // No continuation bit. We're done determining the size.
                    break
                }
            }

            let mut payload_buffer = vec![0; size];

            stream.read_exact(&mut payload_buffer).await?;

            match tag {
                DATA_MESSAGE_STANZA_TAG => {
                    let data_message = mcs::DataMessageStanza::decode(&payload_buffer[..])?;

                    let mut persistent_id_2: Option<String> = None;
                    if let Some(ref persistent_id) = data_message.persistent_id {
                        persistent_id_2 = Some(String::from(persistent_id));
                    }

                    let decrypt_result = self.decrypt_message(data_message)?;

                    let message = FcmMessage { payload_json: decrypt_result, persistent_id: persistent_id_2 };
                    (self.message_callback)(message);
                },
                HEARTBEAT_PING_TAG => {
                    stream.write_u8(HEARTBEAT_ACK_TAG).await?;

                    let heartbeat_ack = mcs::HeartbeatAck::default();
                    let heartbeat_ack_bytes = heartbeat_ack.encode_length_delimited_to_vec();
                    stream.write_all(&heartbeat_ack_bytes).await?;
                },
                LOGIN_RESPONSE_TAG => {
                    self.received_persistent_ids.clear();
                },
                _ => {}
            }
        }

        Ok(())
    }

    fn decrypt_message(&self, message: mcs::DataMessageStanza) -> Result<String, Error> {
        let raw_data = message.raw_data.ok_or(Error::MissingMessagePayload)?;
    
        let crypto_key = find_app_data(&message.app_data, "crypto-key").ok_or(Error::MissingCryptoMetadata)?;
        let encryption = find_app_data(&message.app_data, "encryption").ok_or(Error::MissingCryptoMetadata)?;
    
        // crypto_key is in the format dh=abc...
        let dh_bytes = URL_SAFE.decode(&crypto_key[3..])?;
    
        // encryption is in the format salt=abc...
        let salt_bytes = URL_SAFE.decode(&encryption[5..])?;
    
        let keys = &self.registration.keys;
    
        let public_key_bytes = URL_SAFE_NO_PAD.decode(&keys.public_key)?;
        let private_key_bytes = URL_SAFE_NO_PAD.decode(&keys.private_key)?;
        let auth_secret_bytes = URL_SAFE_NO_PAD.decode(&keys.auth_secret)?;
    
        let components = EcKeyComponents::new(private_key_bytes, public_key_bytes);
    
        // The record size default is 4096 and doesn't seem to be overridden for FCM.
        let record_size: u32 = 4096;
        let encrypted_block = ece::legacy::AesGcmEncryptedBlock::new(&dh_bytes, &salt_bytes, record_size, raw_data)?;
        let data_bytes = ece::legacy::decrypt_aesgcm(&components, &auth_secret_bytes, &encrypted_block)?;
    
        let payload_json = String::from_utf8(data_bytes)?;
    
        Ok(payload_json)
    }

    fn create_login_request(&self) -> mcs::LoginRequest {
        let android_id = self.registration.gcm.android_id;
        let device_id = format!("android-{:x}", android_id);
    
        mcs::LoginRequest {
            adaptive_heartbeat: Some(false),
            auth_service: Some(2),
            auth_token: self.registration.gcm.security_token.to_string(),
            id: "chrome-63.0.3234.0".to_owned(),
            domain: "mcs.android.com".to_owned(),
            device_id: Some(device_id),
            network_type: Some(1),
            resource: android_id.to_string(),
            user: android_id.to_string(),
            use_rmq2: Some(true),
            setting: vec![mcs::Setting { name: "new_vc".to_owned(), value: "1".to_owned() }],
            client_event: Vec::new(),
            received_persistent_id: self.received_persistent_ids.clone(),
            ..mcs::LoginRequest::default()
        }
    }
}

fn find_app_data(app_data_list: &[mcs::AppData], key: &str) -> Option<String> {
    let app_data = app_data_list.iter().find(|app_data| app_data.key == key);
    match app_data {
        None => None,
        Some(app_data) => Some(String::from(&app_data.value))
    }
}
