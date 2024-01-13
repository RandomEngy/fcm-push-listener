use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

use crate::Error;

pub struct FcmRegisterResult {
    pub fcm_token: String,
    pub keys: WebPushKeys,
}

#[derive(Serialize)]
struct FcmRegisterRequest {
    web: FcmRegisterRequestWeb,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FcmRegisterRequestWeb {
    application_pub_key: String,
    auth: String,
    endpoint: String,
    p256dh: String,
}

#[derive(Deserialize)]
struct FcmRegisterResponse {
    // name: String,
    token: String,
    // web: FcmRegisterResponseWeb,
}

// #[derive(Deserialize)]
// #[serde(rename_all = "camelCase")]
// struct FcmRegisterResponseWeb {
//     application_pub_key: String,
//     auth: String,
//     endpoint: String,
//     p256dh: String,
// }

#[derive(Clone, Serialize, Deserialize)]
pub struct WebPushKeys {
    /// Public key with URL safe base64 encoding, no padding
    pub public_key: String,

    /// Private key with URL safe base64 encoding, no padding
    pub private_key: String,

    /// Generated random auth secret, with URL safe base64 encoding, no padding
    pub auth_secret: String,
}

pub async fn register_fcm(
    project_id: &str,
    api_key: &str,
    application_pub_key: &str,
    firebase_installation_auth_token: &str,
    gcm_token: &str,
) -> Result<FcmRegisterResult, Error> {
    let endpoint = format!("https://fcm.googleapis.com/fcm/send/{gcm_token}");

    let push_keys: WebPushKeys = create_key_pair()?;

    let request = FcmRegisterRequest {
        web: FcmRegisterRequestWeb {
            application_pub_key: application_pub_key.to_owned(),
            auth: push_keys.auth_secret.to_owned(),
            endpoint: endpoint.to_owned(),
            p256dh: push_keys.public_key.to_owned(),
        }
    };

    let client = reqwest::Client::new();
    let url = format!("https://fcmregistrations.googleapis.com/v1/projects/{project_id}/registrations");
    let response = client.post(url)
        .json(&request)
        .header("x-goog-api-key", api_key)
        .header("x-goog-firebase-installations-auth", firebase_installation_auth_token)
        .send()
        .await?;

    let response_object = response.json::<FcmRegisterResponse>().await?;

    Ok(FcmRegisterResult {
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
