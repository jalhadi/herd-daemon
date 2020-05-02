use serde::{Deserialize, Serialize};
use serde_json::{Value};

#[derive(Deserialize, Serialize)]
pub struct Data {
    component_id: String,
    json: Value,   
}

#[derive(Serialize)]
pub struct Event {
    pub seconds_since_unix: u64,
    pub nano_seconds: u32,
    pub data: Data
}

pub enum Request {
    Data(Event),
    Close,
    Pong(Vec<u8>)
}

pub struct ClientInformation {
    pub device_id: String,
    pub device_type_id: String,
    pub account_id: String,
    pub api_key: String,
}
