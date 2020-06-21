use std::sync::mpsc::{Sender, Receiver};
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use std::thread;
use std::thread::JoinHandle;
use std::time::SystemTime;
use serde_json::{Result as SerdeResult};
use zmq;

use crate::utils::maybe_error;

use crate::models::{
    ClientMessage,
    Event,
    Request,
    InboundMessage,
};

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

// TODO: create new "receiver thread" (handles inbound connections
// to be passed to client) with receiver channel. Would allow
// for messages to be sent more easily for information concerning
// connection status, retry logic, shutdown
pub fn initialize<'a>(
    sender: Sender<Request>,
    receiver: Receiver<InboundMessage>,
    outbound_port: &'a str,
    context: zmq::Context,
    inbound_socket: zmq::Socket,
    registered_topics: Arc<Mutex<HashSet::<String>>>,
) -> (JoinHandle<()>, JoinHandle<()>) {
    let subscriber = context.socket(zmq::PULL).unwrap();
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

            let client_message: SerdeResult<ClientMessage> = serde_json::from_str(message);

            let client_message = match client_message {
                Ok(d) => d,
                Err(e) => {
                    println!("Error deserializing data: {:?}", e);
                    continue;
                }
            };

            match client_message {
                ClientMessage::Close => {
                    println!("Closing connection.");
                    let _ = sender.send(Request::Close);
                    return;
                },
                ClientMessage::WebsocketClose => {
                    println!("Websocket closed. Closing connection.");
                    return;
                },
                ClientMessage::Register { topics } => {
                    {
                        let mut data = registered_topics.lock().unwrap();
                        for topic in &topics {
                            data.insert(topic.clone());
                        }
                    }

                    let event = Event::Register {
                        topics,
                    };

                    let request_data = Request::Data(event);
                    maybe_error(sender.send(request_data));

                }
                ClientMessage::Data { topics, data } => {
                    let event = Event::Message {
                        seconds_since_unix: time.seconds_since_unix,
                        nano_seconds: time.nano_seconds,
                        topics,
                        data,
                    };

                    let request_data = Request::Data(event);
                    maybe_error(sender.send(request_data));
                },
            };
        };
    });

    let receiver_thread = thread::spawn(move || {
        loop {
            let message = match receiver.recv() {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error sending message: {:?}", e);
                    continue;
                }
            };

            let send_result = match message {
                InboundMessage::Data(d) => inbound_socket.send(d.as_bytes(), 0),
                InboundMessage::Restart =>
                    inbound_socket.send(serde_json::to_string(&InboundMessage::Restart).unwrap().as_bytes(), 0),
                InboundMessage::Close => {
                    let _ = inbound_socket.send(serde_json::to_string(&InboundMessage::Close).unwrap().as_bytes(), 0);
                    return;
                },
            };
            match send_result {
                Ok(_) => (),
                Err(e) => eprintln!("Error sending inbound message: {:?}", e),
            }
        }
    });

    return (sender_thread, receiver_thread);
}