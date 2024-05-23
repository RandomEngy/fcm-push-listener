pub use fcm_push_listener::Error;
use fcm_push_listener::{MessageStream, Registration, Session as GcmSession, WebPushKeys};

async fn run(registration: Registration) -> Result<(), fcm_push_listener::Error> {
    use tokio_stream::StreamExt;

    let session = registration.gcm.checkin().await?;
    let connection = session.new_connection(vec![]).await?;
    let mut stream = MessageStream::wrap(connection, &registration.keys)?;

    while let Some(message) = stream.next().await {
        match message? {
            fcm_push_listener::Message::Data(d) => {
                println!("Message {:?} JSON: {:?}", d.persistent_id, d.body);
            }
            fcm_push_listener::Message::HeartbeatPing => {
                println!("heartbeat");
            }
            fcm_push_listener::Message::Other(tag, bytes) => {
                println!("Unrecognized tag: {tag:?}, {bytes:?}");
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    use std::io::Read;

    let registration = Registration {
        fcm_token: "abc".to_owned(),
        gcm: GcmSession {
            android_id: 123,
            security_token: 456,
        },
        keys: WebPushKeys {
            auth_secret: "def".to_owned(),
            private_key: "ghi".to_owned(),
            public_key: "jkl".to_owned(),
        },
    };

    tokio::spawn(run(registration));

    println!("Listening for push messages. Press any key to exit");
    let mut buf = [0u8; 1];
    std::io::stdin().read(&mut buf).expect("read error");
}
