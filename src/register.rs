use serde::Deserialize;
use serde::Serialize;
use serde_with::{serde_as, DisplayFromStr};
use uuid::Uuid;

use crate::fcm;
use crate::fcm::WebPushKeys;
use crate::firebase;
use crate::gcm;
use crate::Error;

// Normal JSON serialization will lose precision and change the number, so we must
// force the i64/u64 to serialize to string.
#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct GcmRegistration {
    #[serde_as(as = "DisplayFromStr")]
    pub android_id: i64,

    #[serde_as(as = "DisplayFromStr")]
    pub security_token: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Registration {
    pub gcm: GcmRegistration,
    pub fcm_token: String,
    pub keys: WebPushKeys,
}

pub async fn register(
    firebase_app_id: &str,
    firebase_project_id: &str,
    firebase_api_key: &str,
    vapid_key: &str,
) -> Result<Registration, Error> {
    log::debug!("Checking in to GCM");
    let checkin_result = gcm::CheckIn::request(None, None).await?;

    let id = Uuid::new_v4();
    let gcm_app_id = format!("wp:receiver.push.com#{id}");

    log::debug!("Registering to GCM");
    let gcm_token = gcm::register(&gcm_app_id, &checkin_result).await?;

    log::debug!("Getting Firebase installation token");
    let firebase_installation_token = firebase::InstallationAuthToken::request(
        firebase_app_id,
        firebase_project_id,
        firebase_api_key,
    )
    .await?;

    log::debug!("Calling FCM register");
    let fcm_register_result = fcm::Registration::request(
        firebase_project_id,
        firebase_api_key,
        vapid_key,
        &firebase_installation_token.value,
        &gcm_token,
    )
    .await?;

    log::debug!("Registration complete");

    Ok(Registration {
        gcm: GcmRegistration {
            android_id: checkin_result.android_id,
            security_token: checkin_result.security_token,
        },
        fcm_token: fcm_register_result.fcm_token,
        keys: fcm_register_result.keys,
    })
}
