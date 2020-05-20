use std::sync::mpsc::{Sender, Receiver};
use std::thread::JoinHandle;
use zmq;

use crate::models::{Request, ClientInformation};

pub fn initialize<'a>(
    account_id: &'a str,
    api_key: &'a str,
    device_type_id: &'a str,
    outbound_port: &'a str,
    inbound_port: &'a str,
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
    let context = zmq::Context::new();
    // For messages that come into the websocket, this is a channel
    // to comunicate with the process outside
    let inbound_socket = context.socket(zmq::PAIR).unwrap();
    let inbound_tcp_port = format!("tcp://localhost:{}", inbound_port);
    assert!(inbound_socket.connect(&inbound_tcp_port).is_ok());


    let client_information = ClientInformation::new(
        "dev_1234",
        device_type_id,
        account_id,
        api_key,
    );
    let (sender_thread, receiver_thread) = crate::connection::initialize(
        client_information,
        sender.clone(),
        receiver,
        inbound_socket,
    );
    let ipc_thread = crate::ipc::initialize(sender, outbound_port, context.clone());
    (sender_thread, receiver_thread, ipc_thread)
}