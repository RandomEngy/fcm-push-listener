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
pub use register::register;
pub use register::Registration;
