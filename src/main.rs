extern crate websocket;
extern crate clap;
extern crate daemonize;
extern crate mac_address;

use uuid::Uuid;
use std::sync::mpsc::channel;
use std::fs::File;
use clap::Clap;
use daemonize::Daemonize;
use std::thread::JoinHandle;
use zmq;
use mac_address::get_mac_address;

mod connection;
mod models;
mod ipc;
mod utils;

use crate::models::{Request, ClientInformation, InboundMessage};


fn initialize<'a>(
    account_id: &'a str,
    api_key: &'a str,
    device_type_id: &'a str,
    device_id: &'a str,
    outbound_port: &'a str,
    inbound_port: &'a str,
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
    let inbound_socket = context.socket(zmq::PUB).unwrap();
    let inbound_tcp_port = format!("tcp://*:{}", inbound_port);
    assert!(inbound_socket.bind(&inbound_tcp_port).is_ok());

    // This is a PUSH socket such that the websocket thread
    // can tell the thread handles incoming messages
    // from the client to close
    let ipc_socket = context.socket(zmq::PUSH).unwrap();
    let ipc_socket_port = format!("tcp://localhost:{}", outbound_port);
    assert!(ipc_socket.connect(&ipc_socket_port).is_ok());

    // DEFINITIONS
    // outbound_: data and structures supporting data
    // moving from inside the system to the external server
    // inbound_: data and structures supporting data
    // moving from data received from the servers meant
    // for internal consumption
    let (outbound_sender, outbound_receiver) = channel::<Request>();
    let (inbound_sender, inbound_receiver) = channel::<InboundMessage>();

    let (outbound_message_thread, inbound_message_thead) = crate::ipc::initialize(
        outbound_sender.clone(),
        inbound_receiver,
        outbound_port,
        context.clone(),
        inbound_socket,
    );

    let client_information = ClientInformation::new(
        device_id,
        device_type_id,
        account_id,
        api_key,
    );
    let websocket_handler = crate::connection::initialize(
        client_information,
        outbound_sender,
        outbound_receiver,
        inbound_sender,
        // inbound_socket,
        // ipc_socket,
    );

    (websocket_handler, outbound_message_thread, inbound_message_thead)
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

    let (websocket_handler, outbound_message_thread, inbound_message_thead) = initialize(
        &opts.account_id,
        &opts.api_key,
        &opts.device_type_id,
        &device_id,
        &opts.outbound_port,
        &opts.inbound_port,
    );

    println!("Waiting for join");
    let _ = websocket_handler.join();
    let _ = inbound_message_thead.join();
    let _ = outbound_message_thread.join();
    println!("Daemon exited");
}
