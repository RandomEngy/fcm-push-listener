use crate::Error;
use serde::{Deserialize, Serialize};

const INSTALL_API: &str = "https://firebaseinstallations.googleapis.com/v1";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallationRequest<'a> {
    app_id: &'a str,
    auth_version: &'a str,
    fid: &'a str,
    sdk_version: &'a str,
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
pub struct InstallationAuthToken {
    // expires_in: String,
    #[serde(rename = "token")]
    pub value: String,
}

impl InstallationAuthToken {
    pub async fn request(
        http: &reqwest::Client,
        application_id: &str,
        project_id: &str,
        api_key: &str,
    ) -> Result<Self, Error> {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD as Base64;
        use base64::engine::Engine;

        let fid = generate_fid();

        let request = InstallationRequest {
            app_id: application_id,
            auth_version: "FIS_v2",
            fid: &fid,
            sdk_version: "w:0.6.4",
        };

        let heartbeat_json = "{\"heartbeats\": [], \"version\": 2}";
        let heartbeat_header_value = Base64.encode(heartbeat_json.as_bytes());

        const API: &str = "Firebase installation";

        let response = http
            .post(format!("{INSTALL_API}/projects/{project_id}/installations"))
            .json(&request)
            .header("x-firebase-client", heartbeat_header_value)
            .header("x-goog-api-key", api_key)
            .send()
            .await
            .map_err(|e| Error::Request(API, e))?;

        let response: InstallationResponse =
            response.json().await.map_err(|e| Error::Response(API, e))?;

        Ok(response.auth_token)
    }
}

fn generate_fid() -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD as Base64;
    use base64::engine::Engine;
    use rand::RngCore;

    let mut fid: [u8; 17] = [0; 17];
    rand::rng().fill_bytes(&mut fid);
    fid[0] = 0b01110000 + (fid[0] % 0b00010000);
    Base64.encode(fid)
}
