use uuid::Uuid;

use crate::Error;
use crate::fcm::WebPushKeys;
use crate::gcm;
use crate::fcm;

pub struct GcmRegistration {
    pub android_id: i64,
    pub security_token: u64,
}

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