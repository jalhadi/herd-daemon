use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;
use std::time::SystemTime;
use serde_json::{Result as SerdeResult};
use zmq;

use crate::models::{IncomingData, Event, Request};

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

pub fn initialize<'a>(
    sender: Sender<Request>,
    outbound_port: &'a str,
    context: zmq::Context
) -> JoinHandle<()> {
    let subscriber = context.socket(zmq::PAIR).unwrap();
    let outbound_tcp_port = format!("tcp://*:{}", outbound_port);
    assert!(subscriber.bind(&outbound_tcp_port).is_ok());

    // Sender thread: receives a message to be send over websocket
    let sender_thread = thread::spawn(move || {
        loop {
            let maybe_message = subscriber.recv_msg(0).unwrap();
            let time = match get_time() {
                Ok(t) => t,
                Err(e) => {
                    println!("{:?}", e);
                    continue;
                },
            };
            let message = match std::str::from_utf8(&maybe_message) {
                Ok(m) => m,
                Err(_) => continue,
            };

            println!("Message received: {:?}", message);

            match message {
                "Close" => {
                    println!("Closing connection.");
                    let _ = sender.send(Request::Close);
                    return;
                },
                _ => {
                    let data: SerdeResult<IncomingData> = serde_json::from_str(message);

                    let data = match data {
                        Ok(d) => d,
                        Err(e) => {
                            println!("Error deserializing data: {:?}", e);
                            continue;
                        }
                    };

                    let event: Event;
                    if data.event_type == "register" {
                        let topics = match data.data.get("topics") {
                            Some(t) => t.to_owned(),
                            None => {
                                println!("topics field not present");
                                continue;
                            }
                        };
                        event = Event::Register {
                            topics,
                        }

                    } else if data.event_type == "message" {
                        event = Event::Message {
                            seconds_since_unix: time.seconds_since_unix,
                            nano_seconds: time.nano_seconds,
                            topics: data.topics,
                            data: data.data,
                        };
                    } else {
                        println!("{:?} not a valid event type.", data.event_type);
                        continue;
                    }

                    let request_data = Request::Data(event);
                    match sender.send(request_data) {
                        Ok(_) => (),
                        Err(e) => println!("Error passing message: {:?}", e),
                    };
                },
            };
        };
    });

    return sender_thread;
}