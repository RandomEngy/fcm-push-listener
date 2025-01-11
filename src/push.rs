use crate::Error;
use bytes::{Bytes, BytesMut};
use ece::EcKeyComponents;
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

#[allow(dead_code)]
#[derive(PartialEq, Debug)]
pub enum MessageTag {
    HeartbeatPing = 0,
    HeartbeatAck,
    LoginRequest,
    LoginResponse,
    Close,
    MessageStanza,
    PresenceStanza,
    IqStanza,
    DataMessageStanza,
    BatchPresenceStanza,
    StreamErrorStanza,
    HttpRequest,
    HttpResponse,
    BindAccountRequest,
    BindAccountResponse,
    TalkMetadata,
    NumProtoTypes,
}

impl TryFrom<u8> for MessageTag {
    type Error = u8;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        if value < Self::NumProtoTypes as u8 {
            Ok(unsafe { std::mem::transmute(value) })
        } else {
            Err(value)
        }
    }
}

pub enum Message {
    HeartbeatPing,
    Data(DataMessage),
    Other(u8, Bytes),
}

pub struct DataMessage {
    pub body: Vec<u8>,
    pub persistent_id: Option<String>,
}

impl DataMessage {
    fn decode(eckey: &EcKeyComponents, auth_secret: &[u8], bytes: &[u8]) -> Result<Self, Error> {
        use base64::engine::general_purpose::URL_SAFE;
        use base64::Engine;
        use ece::legacy::AesGcmEncryptedBlock;
        use prost::Message;

        let message = crate::mcs::DataMessageStanza::decode(bytes)
            .map_err(|e| Error::ProtobufDecode("FCM data message", e))?;

        let bytes = match message.raw_data {
            Some(v) => v,
            None => {
                return Err(Error::EmptyPayload);
            }
        };

        let mut kex: Vec<u8> = Vec::default();
        let mut salt: Vec<u8> = Vec::default();
        for field in message.app_data {
            match field.key.as_str() {
                "crypto-key" => {
                    // crypto_key format: dh=abc...
                    kex = URL_SAFE
                        .decode(&field.value[3..])
                        .map_err(|e| Error::Base64Decode("FCM message crypto-key", e))?;

                    if !salt.is_empty() {
                        break;
                    }
                }
                "encryption" => {
                    // encryption format: salt=abc...
                    salt = URL_SAFE
                        .decode(&field.value[5..])
                        .map_err(|e| Error::Base64Decode("FCM message encryption params", e))?;

                    if !kex.is_empty() {
                        break;
                    }
                }
                _ => {}
            }
        }

        if kex.is_empty() {
            return Err(Error::MissingCryptoMetadata("crypto-key"));
        } else if salt.is_empty() {
            return Err(Error::MissingCryptoMetadata("encryption"));
        }

        // The record size default is 4096 and doesn't seem to be overridden for FCM.
        const RECORD_SIZE: u32 = 4096;
        const OPERATION: &str = "message decryption";
        let block = AesGcmEncryptedBlock::new(&kex, &salt, RECORD_SIZE, bytes)
            .map_err(|e| Error::Crypto(OPERATION, e))?;
        let body = ece::legacy::decrypt_aesgcm(eckey, auth_secret, &block)
            .map_err(|e| Error::Crypto(OPERATION, e))?;
        Ok(Self {
            body,
            persistent_id: message.persistent_id,
        })
    }
}

pin_project! {
    pub struct MessageStream<T> {
        #[pin]
        inner: T,
        eckey: EcKeyComponents,
        auth_secret: Vec<u8>,
        bytes_required: usize,
        receive_buffer: BytesMut,
    }
}

impl MessageStream<tokio_rustls::client::TlsStream<tokio::net::TcpStream>> {
    pub fn wrap(connection: crate::gcm::Connection, keys: &crate::fcm::WebPushKeys) -> Self {
        Self::new(connection.0, keys)
    }
}

impl<T> MessageStream<T> {
    fn new(inner: T, keys: &crate::fcm::WebPushKeys) -> Self {
        Self {
            inner,
            eckey: EcKeyComponents::new(keys.private_key.clone(), keys.public_key.clone()),
            auth_secret: keys.auth_secret.clone(),
            bytes_required: 2,
            receive_buffer: BytesMut::with_capacity(1024),
        }
    }

    /// returns a decoded protobuf varint or a state change if there is insufficient data
    fn try_read_varint<'a>(mut bytes: impl Iterator<Item = &'a u8>) -> (usize, usize) {
        let mut result = 0;
        let mut bytes_read = 0;

        loop {
            let byte = match bytes.next() {
                // since data is little endian, partially read sizes will always be smaller than
                // the actual message size, on average we expect size / fragmentation + 1 reads
                None => return (result, 2 + bytes_read),
                Some(v) => v,
            };

            // Strip the continuation bit
            let value_part = byte & !0x80u8;

            // accumulate little endian bits
            result += (value_part as usize) << (bytes_read * 7);

            // IFF equal -> No continuation bit -> Varint has concluded
            if value_part.eq(byte) {
                return (result, 2 + bytes_read);
            }

            bytes_read += 1;
        }
    }
}

impl<T> tokio_stream::Stream for MessageStream<T>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    type Item = Result<Message, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use bytes::Buf;
        use std::future::Future;
        use tokio::io::AsyncReadExt;

        loop {
            let mut bytes = self.receive_buffer.iter();
            if let Some(tag_value) = bytes.next() {
                let tag_value = *tag_value;
                let tag = MessageTag::try_from(tag_value);
                if matches!(tag, Ok(MessageTag::Close)) {
                    self.bytes_required = 0;
                    self.receive_buffer.clear();
                    return Poll::Ready(None);
                }

                // determine size of the message
                let (size, offset) = Self::try_read_varint(bytes);
                let bytes_required = offset + size;
                if bytes_required <= self.receive_buffer.len() {
                    // sizeof next_message is unknown, if sizeof next_message < sizeof this_message
                    // && we don't resetting expectations -> we block despite having received the
                    // smaller message in its entirety
                    self.bytes_required = 2;

                    self.receive_buffer.advance(offset);
                    let bytes = self.receive_buffer.split_to(size);
                    return Poll::Ready(Some(Ok(match tag {
                        Ok(MessageTag::DataMessageStanza) => {
                            match DataMessage::decode(&self.eckey, &self.auth_secret, &bytes) {
                                Err(e) => return Poll::Ready(Some(Err(e))),
                                Ok(m) => Message::Data(m),
                            }
                        }
                        Ok(MessageTag::HeartbeatPing) => Message::HeartbeatPing,
                        _ => Message::Other(tag_value, bytes.into()),
                    })));
                }

                // ensure buffer can contain at least the current message
                let capacity = self.receive_buffer.capacity();
                if bytes_required > capacity {
                    self.receive_buffer.reserve(bytes_required - capacity);
                }

                self.bytes_required = bytes_required;
            } else if self.bytes_required == 0 {
                return Poll::Ready(None);
            }

            loop {
                // insufficient data in the buffer, fill from inner
                let mut that = self.as_mut().project();
                let task = that.inner.read_buf(that.receive_buffer);
                tokio::pin!(task);
                match task.poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(e)) => {
                        // failfast
                        self.bytes_required = 0;
                        self.receive_buffer.clear();
                        return Poll::Ready(Some(Err(Error::Socket(e))));
                    }
                    Poll::Ready(Ok(0)) => {
                        // probably a broken pipe, which means whatever incomplete
                        // message we have buffered will just have to be chucked
                        self.bytes_required = 0;
                        self.receive_buffer.clear();
                        return Poll::Ready(None);
                    }
                    _ => {
                        if self.receive_buffer.len() >= self.bytes_required {
                            break;
                        }
                    }
                }
            }
        }
    }
}

impl<T> std::ops::Deref for MessageStream<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> std::ops::DerefMut for MessageStream<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

pub fn new_heartbeat_ack() -> BytesMut {
    use bytes::BufMut;

    let ack = crate::mcs::HeartbeatAck::default();
    let mut bytes = BytesMut::with_capacity(prost::Message::encoded_len(&ack) + 5);
    bytes.put_u8(MessageTag::HeartbeatAck as u8);
    prost::Message::encode_length_delimited(&ack, &mut bytes)
        .expect("heartbeat ack serialization should succeed");

    bytes
}
