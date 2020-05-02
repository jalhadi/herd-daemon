use std::sync::mpsc::{Sender, Receiver};
use std::thread::JoinHandle;

use crate::models::{Request, ClientInformation};

pub fn initialize<'a>(
    account_id: &'a str,
    api_key: &'a str,
    device_type_id: &'a str,
    port: &'a str,
    sender: Sender<Request>,
    receiver: Receiver<Request>
) -> (JoinHandle<()>, JoinHandle<()>, JoinHandle<()>) {
    /*
        Steps:
        - store account_id, api_key
        - create device_id from hardware information (MAC address?)
        - initialize MPSC channel
        - initialize websocket handler
    */
    let client_information = ClientInformation::new(
        "dev_123",
        device_type_id,
        account_id,
        api_key,
    );
    let (sender_thread, receiver_thread) = crate::connection::initialize(
        client_information,
        sender.clone(),
        receiver
    );
    let ipc_thread = crate::ipc::initialize(sender, port);
    (sender_thread, receiver_thread, ipc_thread)
}