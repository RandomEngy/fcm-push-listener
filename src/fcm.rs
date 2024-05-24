use crate::Error;
use serde::{Deserialize, Serialize};

fn to_base64<S: serde::ser::Serializer>(v: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    use base64::Engine;

    let str = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v);
    serializer.serialize_str(&str)
}

fn from_base64<'de, D: serde::de::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<u8>, D::Error> {
    use base64::Engine;

    <&str>::deserialize(deserializer).and_then(|s| {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s)
            .map_err(serde::de::Error::custom)
    })
}

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
                endpoint: &endpoint,
                auth: &push_keys.auth_secret,
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
    endpoint: &'a str,

    #[serde(serialize_with = "to_base64")]
    auth: &'a [u8],

    #[serde(serialize_with = "to_base64")]
    p256dh: &'a [u8],
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
    #[serde(deserialize_with = "from_base64")]
    pub public_key: Vec<u8>,

    /// Private key with URL safe base64 encoding, no padding
    #[serde(deserialize_with = "from_base64")]
    pub private_key: Vec<u8>,

    /// Generated random auth secret, with URL safe base64 encoding, no padding
    #[serde(deserialize_with = "from_base64")]
    pub auth_secret: Vec<u8>,
}

impl WebPushKeys {
    fn new() -> Result<Self, ece::Error> {
        let (key_pair, auth_secret) = ece::generate_keypair_and_auth_secret()?;
        let components = key_pair.raw_components()?;
        Ok(WebPushKeys {
            public_key: components.public_key().into(),
            private_key: components.private_key().into(),
            auth_secret: auth_secret.into(),
        })
    }
}
