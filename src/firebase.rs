use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallationRequest {
    app_id: String,
    auth_version: String,
    fid: String,
    sdk_version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallationResponse {
    auth_token: InstallationAuthToken,
    // fid: String,
    // name: String,
    // refresh_token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallationAuthToken {
    // expires_in: String,
    token: String,
}

pub async fn get_installation(
    app_id: &str,
    project_id: &str,
    api_key: &str,
) -> Result<String, Error> {
    let request = InstallationRequest {
        app_id: app_id.to_owned(),
        auth_version: "FIS_v2".to_owned(),
        fid: generate_firebase_fid(),
        sdk_version: "w:0.6.4".to_owned(),
    };

    let heartbeat_json = "{\"heartbeats\": [], \"version\": 2}";
    let heartbeat_header_value = URL_SAFE_NO_PAD.encode(heartbeat_json.as_bytes());

    let client = reqwest::Client::new();
    let response = client
        .post(format!(
            "https://firebaseinstallations.googleapis.com/v1/projects/{project_id}/installations"
        ))
        .json(&request)
        .header("x-firebase-client", heartbeat_header_value)
        .header("x-goog-api-key", api_key)
        .send()
        .await?;

    let response_object = response.json::<InstallationResponse>().await?;

    Ok(response_object.auth_token.token)
}

fn generate_firebase_fid() -> String {
    let mut fid: [u8; 17] = [0; 17];
    rand::thread_rng().fill_bytes(&mut fid);

    fid[0] = 0b01110000 + (fid[0] % 0b00010000);

    return URL_SAFE_NO_PAD.encode(fid);
}
