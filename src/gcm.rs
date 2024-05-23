pub mod contract {
    include!(concat!(env!("OUT_DIR"), "/checkin_proto.rs"));
}

use crate::Error;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

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
    pub async fn create(
        android_id: Option<i64>,
        security_token: Option<u64>,
    ) -> Result<Self, Error> {
        use prost::Message;

        let request = contract::AndroidCheckinRequest {
            user_serial_number: Some(0),
            checkin: contract::AndroidCheckinProto {
                r#type: Some(3),
                chrome_build: Some(contract::ChromeBuildProto {
                    platform: Some(2),
                    chrome_version: Some(String::from("63.0.3234.0")),
                    channel: Some(1),
                    ..Default::default()
                }),
                ..Default::default()
            },
            version: Some(3),
            id: android_id,
            security_token: security_token,
            ..Default::default()
        };

        let response = reqwest::Client::new()
            .post(CHECKIN_URL)
            .body(request.encode_to_vec())
            .header(reqwest::header::CONTENT_TYPE, "application/x-protobuf")
            .send()
            .await?;

        let response_bytes = response.bytes().await?;
        let response = contract::AndroidCheckinResponse::decode(response_bytes)?;

        let android_id = require_some(response.android_id, "response is missing android id")?;

        const BAD_ID: Result<i64, Error> = Err(Error::DependencyFailure(
            "GCM checkin",
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

        let result = reqwest::Client::new()
            .post(REGISTER_URL)
            .form(&params)
            .header(reqwest::header::AUTHORIZATION, auth_header)
            .send()
            .await?;

        let response_text = result.text().await?;
        let mut tokens = response_text.split("=");

        const API_NAME: &str = "GCM registration";
        const ERR_EOF: Error = Error::DependencyFailure(API_NAME, "malformed response");
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
