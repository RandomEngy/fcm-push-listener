pub mod contract {
    include!(concat!(env!("OUT_DIR"), "/checkin_proto.rs"));
}

use crate::Error;
use prost::bytes::BufMut;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use tokio_rustls::rustls::pki_types::ServerName;

fn require_some<T>(value: Option<T>, reason: &'static str) -> Result<T, Error> {
    match value {
        Some(value) => Ok(value),
        None => Err(Error::DependencyFailure("Android device check-in", reason)),
    }
}

const CHECKIN_URL: &str = "https://android.clients.google.com/checkin";
const REGISTER_URL: &str = "https://android.clients.google.com/c2dm/register3";

// Normal JSON serialization will lose precision and change the number, so we must
// force the i64/u64 to serialize to string.
#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub android_id: i64,

    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub security_token: u64,
}

impl Session {
    async fn request(
        http: &reqwest::Client,
        android_id: Option<i64>,
        security_token: Option<u64>,
    ) -> Result<Self, Error> {
        use prost::Message;

        let request = contract::AndroidCheckinRequest {
            version: Some(3),
            id: android_id,
            security_token,
            user_serial_number: Some(0),
            checkin: contract::AndroidCheckinProto {
                r#type: Some(3),
                chrome_build: Some(contract::ChromeBuildProto {
                    platform: Some(2),
                    channel: Some(1),
                    chrome_version: Some(String::from("63.0.3234.0")),
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        const API_NAME: &str = "GCM checkin";

        let response = http
            .post(CHECKIN_URL)
            .body(request.encode_to_vec())
            .header(reqwest::header::CONTENT_TYPE, "application/x-protobuf")
            .send()
            .await
            .map_err(|e| Error::Request(API_NAME, e))?;

        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| Error::Response(API_NAME, e))?;
        let response = contract::AndroidCheckinResponse::decode(response_bytes)
            .map_err(|e| Error::ProtobufDecode("android checkin response", e))?;

        let android_id = require_some(response.android_id, "response is missing android id")?;

        const BAD_ID: Result<i64, Error> = Err(Error::DependencyFailure(
            API_NAME,
            "responded with non-numeric android id",
        ));
        let android_id = i64::try_from(android_id).or(BAD_ID)?;
        let security_token = require_some(
            response.security_token,
            "response is missing security token",
        )?;

        Ok(Self {
            android_id,
            security_token,
        })
    }

    /// check in to the device registration service, possibly obtaining a new security token
    pub async fn checkin(&self, http: &reqwest::Client) -> Result<CheckedSession, Error> {
        let r = Self::request(http, Some(self.android_id), Some(self.security_token)).await?;
        Ok(CheckedSession(r))
    }

    /// check in to the device registration service for the first time
    pub fn create<'a>(
        http: &'a reqwest::Client,
    ) -> impl std::future::Future<Output = Result<Self, Error>> + 'a {
        Self::request(http, None, None)
    }

    pub async fn request_token(&self, app_id: &str) -> Result<String, Error> {
        /// Server key in URL-safe base64
        const SERVER_KEY: &str =
            "BDOU99-h67HcA6JeFXHbSNMu7e2yNNu3RzoMj8TM4W88jITfq7ZmPvIM1Iv-4_l2LxQcYwhqby2xGpWwzjfAnG4";

        let android_id = self.android_id.to_string();
        let auth_header = format!("AidLogin {}:{}", &android_id, &self.security_token);
        let mut params = std::collections::HashMap::with_capacity(4);
        params.insert("app", "org.chromium.linux");
        params.insert("X-subtype", app_id);
        params.insert("device", &android_id);
        params.insert("sender", SERVER_KEY);

        const API_NAME: &str = "GCM registration";
        let result = reqwest::Client::new()
            .post(REGISTER_URL)
            .form(&params)
            .header(reqwest::header::AUTHORIZATION, auth_header)
            .send()
            .await
            .map_err(|e| Error::Request(API_NAME, e))?;

        let response_text = result
            .text()
            .await
            .map_err(|e| Error::Response(API_NAME, e))?;

        const ERR_EOF: Error = Error::DependencyFailure(API_NAME, "malformed response");

        let mut tokens = response_text.split('=');
        match tokens.next() {
            Some("Error") => {
                return Err(Error::DependencyRejection(
                    API_NAME,
                    tokens.next().unwrap_or("no reasons given").into(),
                ))
            }
            None => return Err(ERR_EOF),
            _ => {}
        }

        match tokens.next() {
            Some(v) => Ok(String::from(v)),
            None => Err(ERR_EOF),
        }
    }
}

fn new_tls_initiator() -> tokio_rustls::TlsConnector {
    let root_store = tokio_rustls::rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };

    let config = tokio_rustls::rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    tokio_rustls::TlsConnector::from(std::sync::Arc::new(config))
}

pub struct CheckedSession(Session);

impl CheckedSession {
    const MCS_VERSION: u8 = 41;
    const LOGIN_REQUEST_TAG: u8 = 2;

    pub fn changed(&self, from: &Session) -> bool {
        self.0.security_token != from.security_token || self.0.android_id != from.android_id
    }

    fn new_mcs_login_request(
        &self,
        received_persistent_id: Vec<String>,
    ) -> crate::mcs::LoginRequest {
        let android_id = self.0.android_id.to_string();
        crate::mcs::LoginRequest {
            adaptive_heartbeat: Some(false),
            auth_service: Some(2),
            auth_token: self.0.security_token.to_string(),
            id: "chrome-63.0.3234.0".into(),
            domain: "mcs.android.com".into(),
            device_id: Some(format!("android-{:x}", self.0.android_id)),
            network_type: Some(1),
            resource: android_id.clone(),
            user: android_id,
            use_rmq2: Some(true),
            setting: vec![crate::mcs::Setting {
                name: "new_vc".into(),
                value: "1".into(),
            }],
            received_persistent_id,
            ..Default::default()
        }
    }

    async fn try_connect(
        domain: ServerName<'static>,
        login_bytes: &[u8],
    ) -> Result<Connection, tokio::io::Error> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let stream = tokio::net::TcpStream::connect("mtalk.google.com:5228").await?;
        let tls = new_tls_initiator();
        let mut stream = tls.connect(domain, stream).await?;

        stream.write_all(login_bytes).await?;

        // Read the version
        stream.read_i8().await?;

        Ok(Connection(stream))
    }

    pub async fn new_connection(
        &self,
        received_persistent_id: Vec<String>,
    ) -> Result<Connection, Error> {
        use prost::Message;

        // Install the default crypto provider. If a different one is already registered, this
        // will do nothing.
        let _ = rustls::crypto::ring::default_provider().install_default();

        const ERR_RESOLVE: Error =
            Error::DependencyFailure("name resolution", "unable to resolve google talk host name");

        let domain = ServerName::try_from("mtalk.google.com").or(Err(ERR_RESOLVE))?;

        let login_request = self.new_mcs_login_request(received_persistent_id);

        let mut login_bytes = bytes::BytesMut::with_capacity(2 + login_request.encoded_len() + 4);
        login_bytes.put_u8(Self::MCS_VERSION);
        login_bytes.put_u8(Self::LOGIN_REQUEST_TAG);
        login_request
            .encode_length_delimited(&mut login_bytes)
            .expect("login request encoding failure");

        Self::try_connect(domain.clone(), &login_bytes)
            .await
            .map_err(Error::Socket)
    }
}

impl std::ops::Deref for CheckedSession {
    type Target = Session;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct Connection(pub(crate) tokio_rustls::client::TlsStream<tokio::net::TcpStream>);

impl std::ops::Deref for Connection {
    type Target = tokio_rustls::client::TlsStream<tokio::net::TcpStream>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Connection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
