use crate::Error;
use serde::{Deserialize, Serialize};

pub struct Registration {
    pub fcm_token: String,
    pub keys: WebPushKeys,
}

#[derive(Serialize)]
struct RegisterRequest {
    web: WebRegistrationRequest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebRegistrationRequest {
    application_pub_key: String,
    auth: String,
    endpoint: String,
    p256dh: String,
}

#[derive(Deserialize)]
struct RegisterResponse {
    // name: String,
    token: String,
    // web: WebRegistrationResponse,
}

// #[derive(Deserialize)]
// #[serde(rename_all = "camelCase")]
// struct WebRegistrationResponse {
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
) -> Result<Registration, Error> {
    let endpoint = format!("https://fcm.googleapis.com/fcm/send/{gcm_token}");

    let push_keys: WebPushKeys = create_key_pair()?;

    let request = RegisterRequest {
        web: WebRegistrationRequest {
            application_pub_key: String::from(application_pub_key),
            auth: push_keys.auth_secret.clone(),
            endpoint: endpoint.clone(),
            p256dh: push_keys.public_key.clone(),
        },
    };

    let client = reqwest::Client::new();
    let url =
        format!("https://fcmregistrations.googleapis.com/v1/projects/{project_id}/registrations");
    let response = client
        .post(url)
        .json(&request)
        .header("x-goog-api-key", api_key)
        .header(
            "x-goog-firebase-installations-auth",
            firebase_installation_auth_token,
        )
        .send()
        .await?;

    let response: RegisterResponse = response.json().await?;

    Ok(Registration {
        fcm_token: response.token,
        keys: push_keys,
    })
}

fn create_key_pair() -> Result<WebPushKeys, ece::Error> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::engine::Engine;

    let (keypair, auth_secret) = ece::generate_keypair_and_auth_secret()?;

    let components = keypair.raw_components()?;

    Ok(WebPushKeys {
        public_key: URL_SAFE_NO_PAD.encode(components.public_key()),
        private_key: URL_SAFE_NO_PAD.encode(components.private_key()),
        auth_secret: URL_SAFE_NO_PAD.encode(auth_secret),
    })
}
