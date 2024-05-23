pub use fcm_push_listener::Error;
use fcm_push_listener::{MessageStream, Registration, Session as GcmSession, WebPushKeys};

async fn run(registration: Registration) -> Result<(), fcm_push_listener::Error> {
    let session = registration.gcm.checkin().await?;
    let connection = session.new_connection().await?;
    let stream = MessageStream::new(connection, &registration.keys)?;

    while let Some(message) = stream.next().await {
        println!("Captured state: {}", some_state);

        println!("Message JSON: {}", message.payload_json);
        println!("Persistent ID: {:?}", message.persistent_id);
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
    let buf = [0u8; 1];
    std::io::stdin().read(&mut buf).expect("read error");
}
