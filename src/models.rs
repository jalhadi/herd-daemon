use serde::{Serialize, Deserialize};
use serde_json::{Value};

#[derive(Serialize)]
pub struct Data {
    pub seconds_since_unix: u64,
    pub nano_seconds: u32,
    pub data: Value
}

#[derive(Deserialize)]
pub struct OutgoingData {
    pub event_type: String,
    pub topics: Vec<String>,
    pub data: Value,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InboundMessage {
    Data(String),
    Restart,
    Close,
}

#[derive(Serialize, Debug, Clone)]
pub enum Event {
    Message {
        seconds_since_unix: u64,
        nano_seconds: u32,
        topics: Vec<String>,
        data: Value
    },
    Register {
        // TODO: as vector of strings
        topics: Value
    }
}

pub enum Request {
    Data(Event),
    Close,
    Pong(Vec<u8>)
}

#[derive(Clone)]
pub struct ClientInformation {
    pub device_id: String,
    pub device_type_id: String,
    pub account_id: String,
    pub api_key: String,
}
