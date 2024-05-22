pub use fcm_push_listener::Error;
use fcm_push_listener::Registration;
use fcm_push_listener::GcmRegistration;
use fcm_push_listener::WebPushKeys;
use fcm_push_listener::{FcmPushListener, FcmMessage};
use tokio::task::JoinHandle;

pub mod checkin {
    include!(concat!(env!("OUT_DIR"), "/checkin_proto.rs"));
}

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
            let registration = Registration {
                fcm_token: "abc".to_owned(),
                gcm: GcmRegistration {
                    android_id: 123,
                    security_token: 456
                },
                keys: WebPushKeys {
                    auth_secret: "def".to_owned(),
                    private_key: "ghi".to_owned(),
                    public_key: "jkl".to_owned(),
                }
            };

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

#[tokio::main]
async fn main() {
    let mut push_service = PushService::new();
    push_service.start();

    println!("Listening for push messages. Press enter to exit.");

    let mut temp = String::new();
    std::io::stdin()
        .read_line(&mut temp)
        .expect("Failed to read line");
}
