# Overview

This crate will listen for push messages from Firebase Cloud Messaging (FCM).

# Prerequisites

1. **Firebase App ID** - Firebase console -> Project settings -> General -> Your apps -> App ID

Make this an Android app, since we will be calling the Android device checkin API.

2. **Firebase Project ID** - Firebase console -> Project settings -> General -> Project ID
3. **Firebase API Key** - Google Cloud console -> APIs and Services -> Credentials -> API Keys

Needed permissions for the API key: Firebase Cloud Messaging API, Cloud Messaging, Firebase Installations API, FCM Registration API.

# Registration and basic usage

```rust
use fcm_push_listener::FcmPushListener;

let http = reqwest::Client::new();
let firebase_app_id = "1:1001234567890:android:2665128ba997ffab830a24";
let firebase_project_id = "myapp-1234567890123";
let firebase_api_key = "aBcDeFgHiJkLmNoPqRsTu01234_aBcD0123456789";

let registration = fcm_push_listener::register(
    &http,
    firebase_app_id,
    firebase_project_id,
    firebase_api_key,
    None).await?;

// Send registration.fcm_token to the server to allow it to send push messages to you.

let http = reqwest::Client::new();
let session = registration.gcm.checkin(&http).await?;
let connection = session.new_connection(vec!["0:1677356129944104%7031b2e6f9fd7ecd"]).await?;
let mut stream = MessageStream::wrap(connection, &registration.keys);

while let Some(message) = stream.next().await {
    match message? {
        fcm_push_listener::Message::Data(data) => {
            println!("Message {:?} Data: {:?}", data.persistent_id, data.body);
        }
        fcm_push_listener::Message::HeartbeatPing => {
            println!("Heartbeat");
            let result = stream.write_all(&new_heartbeat_ack()).await;
            if let Err(e) = result {
                println!("Error sending heartbeat ack: {:?}", e);
            }
        }
        fcm_push_listener::Message::Other(tag, bytes) => {
            println!("Got non-data message: {tag:?}, {bytes:?}");
        }
    }
}
```

You need to save the persistent IDs of the messages you receive, then pass them in on the next call to `connect()`. That way you acknowledge receipt of the messages and avoid firing them again.

The push service sends heartbeats every 30 minutes to make sure the client is still connected. Right now you need to manually acknowledge them via `new_heartbeat_ack()`, but a future version of the library may automate this. If you don't ack the heartbeats, push messages will cease within an hour.

The registration has secrets needed the decrypt the push messages; store it in a secure location and re-use it on the next call to `connect()`. `Registration` is marked as `Serialize` and `Deserialize` so you can directly use it.

Example `body`:
```json
{
    "data": {
        "myProp": "myValue"
    },
    "from": "1001234567890",
    "priority": "normal",
    "fcmMessageId": "2cca9428-b164-401c-be3b-e01d8bce6dcd"
}
```

You can do JSON parsing with whatever library you choose. Since `body` is a byte array, you can use `serde_json::from_slice(&message.body)` to directly JSON parse the bytes into the expected types. The `data` property holds the object that was pushed.

## Cancellation, tracking, and message parsing

Since `connect()` returns a `Future` and runs for a long time, I recommend creating and starting the listener from a task. Then you can cancel/abort the task to stop the push listener, and it leaves your app free to do other activities on the main thread.

For example, you could set up a service to manage the push listener:

```rust
struct PushService {
    task: Option<JoinHandle<()>>,
    some_state: String,
}

impl PushService {
    pub fn new() -> Self {
        PushService {
            task: None,
            some_state: "abc".to_owned()
        }
    }

    pub fn start(&mut self) {
        let registration = /* Get registration from storage or call fcm_push_listener::register() */;
        let received_persistent_ids = /* Get persistent IDs received from last time */;

        self.task = Some(tauri::async_runtime::spawn(run_outer(registration, received_persistent_ids)));
    }

    pub fn stop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
            self.task = None;
        }
    }

    pub fn get_status(&self) -> PushServiceStatus {
        if let Some(task) = &self.task {
            if !task.inner().is_finished() {
                return PushServiceStatus::Running;
            }
        }

        PushServiceStatus::Stopped
    }
}

async fn run_outer(registration: Registration, received_persistent_ids: Vec<String>) {
    let result = run(registration, received_persistent_ids, app_handle).await;
    if let Err(err) = result {
        error!("Error running push service: {:?}", err);
    }
}

async fn run(registration: Registration, received_persistent_ids: Vec<String>) -> Result<(), fcm_push_listener::Error> {
    use tokio_stream::StreamExt;

    let http = reqwest::Client::new();
    let session = registration.gcm.checkin(&http).await?;
    let connection = session.new_connection(received_persistent_ids).await?;
    let mut stream = MessageStream::wrap(connection, &registration.keys);

    while let Some(message) = stream.next().await {
        match message? {
            fcm_push_listener::Message::Data(data_message) => {
                println!("Message arrived with ID {:?}", data_message.persistent_id);

                // PushMessagePayload is your custom type with #[derive(Deserialize)]
                let message_payload: PushMessagePayload = serde_json::from_slice(&message.body)?;

                println!("Message arrived with property {:?}", message_payload.data.my_prop);
            }
            _ => {}
        }
    }

    Ok(())
}
```

Then keep an instance of PushService around and call `stop()` on it when you need to cancel.

# Implementation

## Dependencies

* `tokio` for async/TCP.
* `rustls` / `tokio-rustls` for the push listener TLS connection.
* `reqwest` for HTTP calls.
* `prost` for protobuf.
* `ece` for creating the web push key pair and decrypting messages.

## `register()`

1) Calls https://android.clients.google.com/checkin to get an android ID.
2) Calls https://android.clients.google.com/c2dm/register3 to register with GCM. Gives you a GCM token and a security token. (The GCM token is sometimes called an ACG token by other libraries)
3) Calls https://firebaseinstallations.googleapis.com/v1/projects/{project_id}/installations to get a Firebase installation token.
4) Creates an encryption key pair using the legacy `aesgcm` mode of the `ece` crate.
5) Calls https://fcmregistrations.googleapis.com/v1/projects/{project_id}/registrations to do the final FCM registration and get the FCM token.

## `registration.gcm.checkin()`

Makes another checkin call to keep our "device" up to date.

## `new_connection()`

1) Makes a TLS/TCP connection to `mtalk.google.com:5228` and sends information encoded via protobuf to log in with our generated device ID and the list of persistent IDs that we have seen.
2) Keeps the socket connection open to listen for push messages.

## Messages

When a push message arrives, it uses protobuf to parse out the payload and metadata, then uses the private key and auth secret stored in the registration to decrypt the payload and decode to a UTF-8 string. It then invokes the provided closure with the JSON payload and persistent ID.

## Reconnection

If the connection is closed after successfully establishing, it will automatically try and re-open the connection.

# Acknowledgements

The original version is based on the NPM package [push-reciever](https://github.com/MatthieuLemoine/push-receiver) by Matthieu Lemoine. His [reverse-engineering effort](https://medium.com/@MatthieuLemoine/my-journey-to-bring-web-push-support-to-node-and-electron-ce70eea1c0b0) was quite heroic!

The changes for v3 were based on [@aracna/fcm](https://aracna.dariosechi.it/fcm/get-started/) by Dario Sechi.

v4 (async overhaul) was written by [WXY](https://github.com/unreadablewxy).

# Minimum version

Registration for versions older than 3.0.0 have stopped working as of June 20, 2024, since Google shut down an API it calls.

# Build setup

1) Go to https://github.com/protocolbuffers/protobuf/releases , find the latest stable, then extract protoc.exe from protoc-{version}-{platform}.zip and put it in path.
2) Install CMake from https://cmake.org/download/
3) Set up OpenSSL. For Windows, install from https://slproweb.com/products/Win32OpenSSL.html and set the environment variable `OPENSSL_DIR` to `C:\Program Files\OpenSSL-Win64` (or wherever you installed it)

// If you encounter ``could not find native static library `libssl`, perhaps an -L flag is missing`` or a similar compilation error - try to set the environment variable `OPENSSL_LIB_DIR` to `C:\Program Files\OpenSSL-Win64\lib\VC\x64\MD`
