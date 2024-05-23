use crate::Error;
use serde::{Deserialize, Serialize};

pub struct Registration {
    pub fcm_token: String,
    pub keys: WebPushKeys,
}

impl Registration {
    pub async fn request(
        project_id: &str,
        api_key: &str,
        application_pub_key: Option<&str>,
        firebase_installation_auth_token: &str,
        gcm_token: &str,
    ) -> Result<Self, Error> {
        let endpoint = format!("https://fcm.googleapis.com/fcm/send/{gcm_token}");
        let push_keys = WebPushKeys::new()?;
        let request = RegisterRequest {
            web: WebRegistrationRequest {
                application_pub_key,
                auth: &push_keys.auth_secret,
                endpoint: &endpoint,
                p256dh: &push_keys.public_key,
            },
        };

        let client = reqwest::Client::new();
        let url = format!(
            "https://fcmregistrations.googleapis.com/v1/projects/{project_id}/registrations"
        );
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
        Ok(Self {
            fcm_token: response.token,
            keys: push_keys,
        })
    }
}

#[derive(Serialize)]
struct RegisterRequest<'a> {
    web: WebRegistrationRequest<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebRegistrationRequest<'a> {
    application_pub_key: Option<&'a str>,
    auth: &'a str,
    endpoint: &'a str,
    p256dh: &'a str,
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

impl WebPushKeys {
    fn new() -> Result<Self, ece::Error> {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::engine::Engine;

        let (key_pair, auth_secret) = ece::generate_keypair_and_auth_secret()?;
        let components = key_pair.raw_components()?;
        Ok(WebPushKeys {
            public_key: URL_SAFE_NO_PAD.encode(components.public_key()),
            private_key: URL_SAFE_NO_PAD.encode(components.private_key()),
            auth_secret: URL_SAFE_NO_PAD.encode(auth_secret),
        })
    }
}
