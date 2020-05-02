use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;
use std::time::SystemTime;
use serde_json::{Result as SerdeResult};
use zmq;

use crate::models::{Data, Event, Request};

struct CreatedAt {
    seconds_since_unix: u64,
    nano_seconds: u32,
}

fn get_time() -> Result<CreatedAt, &'static str> {
    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH);

    let time = match time {
        Ok(t) => t,
        Err(_) => return Err("Error getting system time."),
    };

    Ok(
        CreatedAt {
            seconds_since_unix: time.as_secs(),
            nano_seconds: time.subsec_nanos(),
        }
    )
}

pub fn initialize<'a>(sender: Sender<Request>, port: &'a str) -> JoinHandle<()> {
    let context = zmq::Context::new();
    let responder = context.socket(zmq::REP).unwrap();
    let mut msg = zmq::Message::new();
    let tcp_port = format!("tcp://*:{}", port);
    assert!(responder.bind(&tcp_port).is_ok());

    thread::spawn(move || {
        loop {
            // in ZeroMQ, send and receive happen in a pair
            responder.recv(&mut msg, 0).unwrap();
            responder.send("", 0).unwrap();
            let time = match get_time() {
                Ok(t) => t,
                Err(e) => {
                    println!("{:?}", e);
                    continue;
                },
            };
            let message = match msg.as_str() {
                Some(m) => m,
                None => continue,
            };

            println!("Message received: {:?}", message);

            match message {
                "Close" => {
                    println!("Closing connection.");
                    let _ = sender.send(Request::Close);
                    return;
                },
                _ => {
                    let data: SerdeResult<Data> = serde_json::from_str(message);

                    let data = match data {
                        Ok(d) => d,
                        Err(e) => {
                            println!("Error deserializing data: {:?}", e);
                            continue;
                        }
                    };

                    let event = Request::Data(Event {
                        seconds_since_unix: time.seconds_since_unix,
                        nano_seconds: time.nano_seconds,
                        data,
                    });
                    match sender.send(event) {
                        Ok(_) => (),
                        Err(e) => println!("Error passing message: {:?}", e),
                    };
                },
            };
        };
    })
}