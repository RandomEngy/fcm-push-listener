mod mcs {
    include!(concat!(env!("OUT_DIR"), "/mcs_proto.rs"));
}

mod error;
mod fcm;
mod firebase;
mod gcm;
mod push;
mod register;

pub use error::Error;
pub use fcm::WebPushKeys;
pub use gcm::Session;
pub use push::Message;
pub use push::MessageStream;
pub use push::MessageTag;
pub use register::register;
pub use register::Registration;

fn to_base64<S: serde::ser::Serializer>(v: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    use base64::Engine;

    let str = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v);
    serializer.serialize_str(&str)
}

fn from_base64<'de, D: serde::de::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<u8>, D::Error> {
    use base64::Engine;
    use serde::de::Deserialize;

    <&str>::deserialize(deserializer).and_then(|s| {
        base64::engine::general_purpose::STANDARD
            .decode(s)
            .map_err(serde::de::Error::custom)
    })
}
