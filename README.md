# IMPORTANT

[An endpoint that this library is dependent upon will be shut down on June 20, 2024](https://firebase.google.com/support/faq#fcm-23-deprecation).

I'm seeing if I can figure out an alternate implementation before then, but no guarantees. Let me know if you want to help.

# Overview

This crate will listen for push messages from Firebase Cloud Messaging (FCM).

# Usage

```rust
use fcm_push_listener::FcmPushListener;

let registration = fcm_push_listener::register("1001234567890").await?;

// Send registration.fcm_token to the server to allow it to send push messages to you.

let mut listener = FcmPushListener::create(
    registration,
    |message: FcmMessage| {
        println!("Message JSON: {}", message.payload_json);
        println!("Persistent ID: {:?}", message.persistent_id);
    },
    |err| { eprintln!("{:?}", err) },
    vec!["0:1677356129944104%7031b2e6f9fd7ecd".to_owned()]);
listener.connect().await?;
```

You need to save the persistent IDs of the messages you receive, then pass them in on the next call to `connect()`. That way you acknowledge receipt of the messages and avoid firing them again.

The registration has secrets needed the decrypt the push messages; store it in a secure location and re-use it on the next call to `connect()`. `Registration` is marked as `Serialize` and `Deserialize` so you can directly use it.

Example `payload_json`:
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

The `data` property holds the object that was pushed. You can do JSON parsing with whatever library you choose.

## Cancellation

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
        let some_state = self.some_state.clone();

        self.task = Some(tokio::task::spawn(async move {
            let registration = /* Get registration from storage or call fcm_push_listener::register() */;

            let mut listener = FcmPushListener::create(
                registration,
                |message: FcmMessage| {
                    println!("Captured state: {}", some_state);
        
                    println!("Message JSON: {}", message.payload_json);
                    println!("Persistent ID: {:?}", message.persistent_id);
                },
                vec![]);

            let result = listener.connect().await;
            if let Err(err) = result {
                eprintln!("{:?}", err);
            }
        }));
    }

    pub fn stop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
            self.task = None;
        }
    }
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
2) Calls https://android.clients.google.com/c2dm/register3 to register with GCM. Gives you a GCM token and a security token.
3) Creates an encryption key pair using the legacy `aesgcm` mode of the `ece` crate.
4) Calls https://fcm.googleapis.com/fcm/connect/subscribe with the GCM token, the sender ID that you supply on the `register()` call and the public key and auth secret created in the previous step.

## `FcmPushListener.connect()`

1) Makes another checkin call to keep our "device" up to date.
2) Makes a TLS/TCP connection to `mtalk.google.com:5228` and sends information encoded via protobuf to log in with our generated device ID and the list of persistent IDs that we have seen.
3) Keeps the socket connection open to listen for push messages.

## Messages

When a push message arrives, it uses protobuf to parse out the payload and metadata, then uses the private key and auth secret stored in the registration to decrypt the payload and decode to a UTF-8 string. It then invokes the provided closure with the JSON payload and persistent ID.

## Reconnection

If the connection is closed after successfully establishing, it will automatically try and re-open the connection.

# Acknowledgements

This is based on the NPM package [push-reciever](https://github.com/MatthieuLemoine/push-receiver) by Matthieu Lemoine. His [reverse-engineering effort](https://medium.com/@MatthieuLemoine/my-journey-to-bring-web-push-support-to-node-and-electron-ce70eea1c0b0) was quite heroic!

# Build setup

1) Go to https://github.com/protocolbuffers/protobuf/releases , find the latest stable, then extract protoc.exe from protoc-{version}-{platform}.zip and put it in path.
2) Set up OpenSSL. For Windows, install from https://slproweb.com/products/Win32OpenSSL.html and set the environment variable `OPENSSL_DIR` to `C:\Program Files\OpenSSL-Win64` (or wherever you installed it)