extern crate websocket;
extern crate clap;
extern crate daemonize;

use std::sync::mpsc::channel;
use std::fs::File;
use clap::Clap;
use daemonize::Daemonize;

mod connection;
mod models;
mod ipc;
mod analytics;

use crate::models::{Request};

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

    let (sender_thread, receiver_thread, ipc_thread) = crate::analytics::initialize(
        &opts.account_id,
        &opts.api_key,
        &opts.device_type_id,
        &opts.outbound_port,
        &opts.inbound_port,
        sender,
        receiver
    );

    println!("Waiting for join");
    let _ = sender_thread.join();
    let _ = receiver_thread.join();
    let _ = ipc_thread.join();
    println!("Daemon exited");
}