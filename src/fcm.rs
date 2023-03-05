use std::collections::HashMap;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

use crate::Error;

pub struct FcmSubscribeResult {
    pub fcm_token: String,
    pub keys: WebPushKeys,
}

#[derive(Deserialize)]
struct FcmSubscribeResponse {
    token: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WebPushKeys {
    /// Public key with URL safe base64 encoding, no padding
    pub public_key: String,

    /// Private key with URL safe base64 encoding, no padding
    pub private_key: String,

    /// Generated random auth secret, with URL safe base64 encoding, no padding
    pub auth_secret: String,
}

pub async fn subscribe_fcm(sender_id: &str, gcm_token: &str) -> Result<FcmSubscribeResult, Error> {
    let push_keys = create_key_pair()?;

    let endpoint = format!("https://fcm.googleapis.com/fcm/send/{gcm_token}");
    let mut params = HashMap::new();
    params.insert("authorized_entity", sender_id);
    params.insert("endpoint", &endpoint);
    params.insert("encryption_key", &push_keys.public_key);
    params.insert("encryption_auth", &push_keys.auth_secret);

    let client = reqwest::Client::new();
    let response = client.post("https://fcm.googleapis.com/fcm/connect/subscribe")
        .form(&params)
        .send()
        .await?;

    let response_object = response.json::<FcmSubscribeResponse>().await?;

    Ok(FcmSubscribeResult {
        fcm_token: response_object.token,
        keys: push_keys
    })
}

fn create_key_pair() -> Result<WebPushKeys, ece::Error> {
    let (keypair, auth_secret) = ece::generate_keypair_and_auth_secret()?;

    let components = keypair.raw_components()?;

    Ok(WebPushKeys {
        public_key: URL_SAFE_NO_PAD.encode(components.public_key()),
        private_key: URL_SAFE_NO_PAD.encode(components.private_key()),
        auth_secret: URL_SAFE_NO_PAD.encode(auth_secret)
    })
}
