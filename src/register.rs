use serde::Deserialize;
use serde::Serialize;
use serde_with::{serde_as, DisplayFromStr};
use uuid::Uuid;

use crate::Error;
use crate::fcm::WebPushKeys;
use crate::gcm;
use crate::fcm;

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

pub async fn register(sender_id: &str) -> Result<Registration, Error> {
    let checkin_result = gcm::check_in(None, None).await?;

    let id = Uuid::new_v4();
    let app_id = format!("wp:receiver.push.com#{id}");

    let gcm_token = gcm::register(&app_id, checkin_result.android_id, checkin_result.security_token).await?;

    let fcm_subscribe_result = fcm::subscribe_fcm(sender_id, &gcm_token).await?;

    Ok(Registration {
        gcm: GcmRegistration {
            android_id: checkin_result.android_id,
            security_token: checkin_result.security_token
        },
        fcm_token: fcm_subscribe_result.fcm_token,
        keys: fcm_subscribe_result.keys,
    })
}