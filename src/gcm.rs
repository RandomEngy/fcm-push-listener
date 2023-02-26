use prost::Message;
use std::collections::HashMap;
use reqwest::header::AUTHORIZATION;

use crate::Error;

pub mod checkin {
    include!(concat!(env!("OUT_DIR"), "/checkin_proto.rs"));
}

pub struct CheckInResult {
    pub android_id: i64,
    pub security_token: u64,
}

/// Server key in URL-safe base64
const SERVER_KEY: &str = "BDOU99-h67HcA6JeFXHbSNMu7e2yNNu3RzoMj8TM4W88jITfq7ZmPvIM1Iv-4_l2LxQcYwhqby2xGpWwzjfAnG4";

pub async fn check_in(android_id: Option<i64>, security_token: Option<u64>) -> Result<CheckInResult, Error> {
    // Build up the request object
    let mut chrome_build = checkin::ChromeBuildProto::default();
    chrome_build.platform = Some(2);
    chrome_build.chrome_version = Some(String::from("63.0.3234.0"));
    chrome_build.channel = Some(1);

    let mut checkin_object = checkin::AndroidCheckinProto::default();
    checkin_object.r#type = Some(3);
    checkin_object.chrome_build = Some(chrome_build);

    let mut request = checkin::AndroidCheckinRequest::default();
    request.user_serial_number = Some(0);
    request.checkin = checkin_object;
    request.version = Some(3);
    request.id = android_id;
    request.security_token = security_token;

    // Serialize via protobuf
    let buf = serialize_checkin_request(&request);

    // Send HTTP request
    let url = "https://android.clients.google.com/checkin";
    let client = reqwest::Client::new();
    let result = client.post(url)
        .body(buf)
        .header("Content-Type", "application/x-protobuf")
        .send()
        .await?;

    let response_bytes = result.bytes().await?;
    
    // Deserialize via protobuf
    let response_object = checkin::AndroidCheckinResponse::decode(response_bytes)?;

    // Make sure we got the security token and Android ID on the response.
    let raw_android_id = if let Some(id) = response_object.android_id {
        id
    } else {
        return Err(Error::InvalidResponse(String::from(url)))
    };

    let Ok(sanitized_android_id) = i64::try_from(raw_android_id) else { return Err(Error::InvalidResponse(String::from(url))) };

    let security_token = if let Some(token) = response_object.security_token {
        token
    } else {
        return Err(Error::InvalidResponse(String::from(url)))
    };

    Ok(CheckInResult { android_id: sanitized_android_id, security_token })
}

fn serialize_checkin_request(request: &checkin::AndroidCheckinRequest) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.reserve(request.encoded_len());
    // Unwrap is safe, since we have reserved sufficient capacity in the vector.
    request.encode(&mut buf).unwrap();

    buf
}

pub async fn register(app_id: &str, android_id: i64, security_token: u64) -> Result<String, Error> {
    let android_id_string = android_id.to_string();

    let mut params = HashMap::new();
    params.insert("app", "org.chromium.linux");
    params.insert("X-subtype", app_id);
    params.insert("device", &android_id_string);
    params.insert("sender", SERVER_KEY);

    let url = "https://android.clients.google.com/c2dm/register3";

    let client = reqwest::Client::new();
    let result = client.post(url)
        .form(&params)
        .header(AUTHORIZATION, format!("AidLogin {android_id}:{security_token}"))
        .send()
        .await?;

    let response_text = result.text().await?;
    let response_parts: Vec<&str> = response_text.split("=").collect();

    if response_parts.len() < 2 {
        return Err(Error::InvalidResponse(String::from(url)))
    }

    let key = response_parts[0];
    if key == "Error" {
        return Err(Error::ServerError(String::from(response_parts[1])))
    }

    Ok(String::from(response_parts[1]))
}
