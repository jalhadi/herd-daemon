extern crate websocket;
extern crate clap;
extern crate daemonize;
extern crate mac_address;

use uuid::Uuid;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};
use std::fs::File;
use clap::Clap;
use daemonize::Daemonize;
use std::thread::JoinHandle;
use zmq;
use mac_address::get_mac_address;

mod connection;
mod models;
mod ipc;

use crate::models::{Request, ClientInformation};


fn initialize<'a>(
    account_id: &'a str,
    api_key: &'a str,
    device_type_id: &'a str,
    device_id: &'a str,
    outbound_port: &'a str,
    inbound_port: &'a str,
    sender: Sender<Request>,
    receiver: Receiver<Request>
) -> (JoinHandle<()>, JoinHandle<()>) {
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

    let ipc_socket = context.socket(zmq::PUSH).unwrap();
    let ipc_socket_port = format!("tcp://localhost:{}", outbound_port);
    assert!(ipc_socket.connect(&ipc_socket_port).is_ok());

    let ipc_thread = crate::ipc::initialize(
        sender.clone(),
        outbound_port,
        context.clone(),
    );

    let client_information = ClientInformation::new(
        device_id,
        device_type_id,
        account_id,
        api_key,
    );
    let websocket_handler = crate::connection::initialize(
        client_information,
        sender,
        receiver,
        inbound_socket,
        ipc_socket,
    );

    (websocket_handler, ipc_thread)
}

#[derive(Debug, Clap)]
struct Opts {
    #[clap(short = "a", long = "account_id")]
    account_id: String,
    #[clap(short = "k", long = "api_key")]
    api_key: String,
    #[clap(short = "d", long = "device_type_id")]
    device_type_id: String,
    #[clap(short = "o", long = "outbound_port", default_value = "5555")]
    outbound_port: String,
    #[clap(short = "i", long = "inbound_port", default_value = "5556")]
    inbound_port: String,
}

fn main() {
    // cargo run -- -a acct -k key -p 1234 -d dev_abc123
    let opts = Opts::parse();

    let stdout = File::create("/tmp/daemon.out").expect("Failed to create output file.");
    let stderr = File::create("/tmp/daemon.err").expect("Faile to create input file.");

    let addr = match get_mac_address() {
        Ok(Some(ma)) => ma.bytes(),
        Ok(None) => {
            println!("No MAC address found, can't compute unique id.");
            return;
        },
        Err(e) => {
            println!("Error obtaining mac address, can't compute unique id. {}", e);
            return;
        },
    };

    let uuid = Uuid::new_v5(&Uuid::NAMESPACE_DNS, &addr);
    let mut buffer: [u8; 45] = Uuid::encode_buffer();
    let uuid = uuid.to_simple().encode_lower(&mut buffer);
    let device_id = format!("dev_{}", uuid);

    let daemonize = Daemonize::new()
        .stdout(stdout)
        .stderr(stderr);

    match daemonize.start() {
        Ok(_) => println!("Daemon started."),
        Err(e) => {
            eprintln!("Error starting daemon: {}", e);
            return;
        },
    }

    let (sender, receiver) = channel::<Request>();

    let (websocket_handler, ipc_thread) = initialize(
        &opts.account_id,
        &opts.api_key,
        &opts.device_type_id,
        &device_id,
        &opts.outbound_port,
        &opts.inbound_port,
        sender,
        receiver
    );

    println!("Waiting for join");
    let _ = websocket_handler.join();
    let _ = ipc_thread.join();
    println!("Daemon exited");
}
