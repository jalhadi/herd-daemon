use std::sync::mpsc::{Sender, Receiver};
use websocket::ClientBuilder;
use websocket::header::{Header, HeaderFormat, Headers, Authorization, Basic};
use websocket::{OwnedMessage, Message};
use hyper::header::parsing::from_one_raw_str;
use std::{thread, time};
use std::thread::JoinHandle;
use std::fmt;
use zmq;
use std::sync::{Arc, Mutex};

use crate::models::{Request, ClientInformation, InboundMessage};
use crate::utils::maybe_error;

#[derive(Debug, Clone)]
struct DeviceIdHeader(String);

impl HeaderFormat for DeviceIdHeader {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let DeviceIdHeader(ref value) = *self;
        write!(fmt, "{}", value)
    }
}

impl Header for DeviceIdHeader {
    fn header_name() -> &'static str {
        "device-id"
    }

    fn parse_header(raw: &[Vec<u8>]) -> hyper::Result<DeviceIdHeader> {
        from_one_raw_str(raw).map(DeviceIdHeader)
    }
}

#[derive(Debug, Clone)]
struct DeviceTypeIdHeader(String);

impl HeaderFormat for DeviceTypeIdHeader {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let DeviceTypeIdHeader(ref value) = *self;
        write!(fmt, "{}", value)
    }
}

impl Header for DeviceTypeIdHeader {
    fn header_name() -> &'static str {
        "device-type-id"
    }

    fn parse_header(raw: &[Vec<u8>]) -> hyper::Result<DeviceTypeIdHeader> {
        from_one_raw_str(raw).map(DeviceTypeIdHeader)
    }
}

impl ClientInformation {
    pub fn new<'a>(device_id: &'a str, device_type_id: &'a str, account_id: &'a str, api_key: &'a str) -> ClientInformation {
        ClientInformation {
            device_id: device_id.to_owned(),
            device_type_id: device_type_id.to_owned(),
            account_id: account_id.to_owned(),
            api_key: api_key.to_owned(),
        }
    }
}

const MAX_RETRIES: u32 = 10;
// TODO: change back to 5000 when ready
const RETRY_SLEEP_DURATION_MILLIS: u64 = 1000;

pub fn initialize(
    client_information: ClientInformation,
    sender: Sender<Request>,
    receiver: Receiver<Request>,
    inbound_sender: Sender<InboundMessage>,
    // inbound_socket: zmq::Socket,
    ipc_socket: zmq::Socket,
) -> JoinHandle<()> {
    let receiver_arc = Arc::new(Mutex::new(receiver));
    // let inbound_socket_arc = Arc::new(Mutex::new(inbound_socket));
    thread::spawn(move || {
        let mut retries = 0;
        loop {
            // This seems to work as a basic restarting mechanism.
            // Currently, if there is no server up, it looks like
            // the websocket conenction (expectantly) fails, but
            // there not sure how the threads exit then...  
            let result = websocket(
                client_information.clone(),
                sender.clone(),
                receiver_arc.clone(),
                inbound_sender.clone(),
                // inbound_socket_arc.clone()
            );

            let (sender_thread, receiver_thread) = match result {
                Ok(x) => {
                    retries = 0;
                    x
                },
                Err(_) => {
                    if retries < MAX_RETRIES {
                        retries += 1;
                        eprintln!(
                            "Error starting websocket connection. Retries {}/{}.",
                            retries,
                            MAX_RETRIES
                        );
                        match inbound_sender.send(InboundMessage::Restart) {
                            Ok(_) => (),
                            Err(e) => eprintln!("{:?}", e),
                        };
                        thread::sleep(time::Duration::from_millis(RETRY_SLEEP_DURATION_MILLIS));
                        continue;
                    } else {
                        eprintln!(
                            "Error starting websocket connection. Max retries ({}) exceeded.",
                            MAX_RETRIES
                        );
                        maybe_error(ipc_socket.send("WebsocketClose".as_bytes(), 0));
                        maybe_error(inbound_sender.send(InboundMessage::Close));
                        return;
                    }
                }
            };
            let sender_output = match sender_thread.join() {
                Ok(res) => res,
                Err(e) => {
                    println!("Error in joining sender: {:?}", e);
                    false
                },
            };
            let receiver_output = match receiver_thread.join() {
                Ok(res) => res,
                Err(e) => {
                    println!("Error in joining receiver: {:?}", e);
                    false
                },
            };
            if !sender_output && !receiver_output {
                println!("Returning websocket thread");
                return;
            }
            println!("Restarting websocket connection...");
            thread::sleep(time::Duration::from_millis(RETRY_SLEEP_DURATION_MILLIS));
        }
    }) 
}

// TODO: add reconnect logic on disconnect
fn websocket(
    client_information: ClientInformation,
    sender: Sender<Request>,
    receiver_arc: Arc<Mutex<Receiver<Request>>>,
    inbound_sender: Sender<InboundMessage>,
    // inbound_socket_arc: Arc<Mutex<zmq::Socket>>,
) -> Result<(JoinHandle<bool>, JoinHandle<bool>), &'static str> {
    let mut headers = Headers::new();
    headers.set(
        Authorization(
            Basic {
                username: client_information.account_id.clone(),
                password: Some(client_information.api_key.clone()),
            }
        )
    );
    headers.set(DeviceIdHeader(client_information.device_id.clone()));
    headers.set(DeviceTypeIdHeader(client_information.device_type_id.clone()));
    let client = ClientBuilder::new("ws://localhost:8080/ws/")
        .unwrap()
        .custom_headers(&headers)
        .connect_insecure();

    let client = match client {
        Ok(c) => c,
        Err(_) => {
            return Err("Error connecting to server.");
        }
    };

    let (mut client_receiver, mut client_sender) = match client.split() {
        Ok(c) => c,
        Err(_) => {
            return Err("Error splitting client.");
        }
    };
    let sender_thread = thread::spawn(move || {
        // Unwrapping and locking the receiver portion
        // over the thread life should be fine as only one
        // websocket connection is used at a time

        let receiver = receiver_arc.lock().unwrap();

        loop {
            let request = receiver.recv().unwrap();

            match request {
                Request::Pong(data) => {
                    match client_sender.send_message(&OwnedMessage::Pong(data)) {
                        Ok(_) => println!("Successfully sent pong"),
                        Err(e) => {
                            // Should restart connection
                            println!("Error sending pong: {:?}", e);
                            let _ = client_sender.send_message(&Message::close());
                            return true;
                        }
                    }
                }
                Request::Data(data) => {
                    let json_string = serde_json::to_string(&data).expect("Error parsing data.");
                    match client_sender.send_message(&OwnedMessage::Text(json_string)) {
                        Ok(()) => println!("Sent message!"),
                        Err(e) => println!("Send Loop: {:?}", e),
                    };

                },
                Request::Close => {
                    println!("Close request received!!!!");
                    match client_sender.send_message(&Message::close()) {
                        Ok(_) => println!("Successfully closed connection"),
                        Err(e) => println!("Error while closing connection: {:?}", e),
                    };
                    // this thread doesn't know if should restart, ask receiver_thread
                    return false;
                },
            }
        }
    });

    let receiver_thread = thread::spawn(move || {
        // Unwrapping and locking should be fine over the
        // duration of the thread life, given that there is only
        // on thread handling the receiving from the socket
        // let inbound_socket = inbound_socket_arc.lock().unwrap();

        for message in client_receiver.incoming_messages() {
            println!("message received: {:?}", message);
            let message = match message {
                Ok(m) => m,
                Err(e) => {
                    println!("Error in received message, closing connection: {:?}", e);
                    let _ = sender.send(Request::Close);
                    let _ = inbound_sender.send(InboundMessage::Close);
                    // TODO: don't restart for now, if it gets here,
                    // daemon was probably given a close code and the sender_thread
                    // already send close
                    return false;
                }
            };

            match message {
                OwnedMessage::Close(_) => {
                    let _ = sender.send(Request::Close);
                    // TODO: Depending on code, maybe restart
                    return true;
                },
                OwnedMessage::Ping(data) => {
                    match sender.send(Request::Pong(data)) {
                        Ok(_) => (),
                        Err(e) => {
                            println!("Error in sending pong frame, closing connection: {:?}", e);
                            let _  = sender.send(Request::Close);
                            // If we unexpectedly can't send data, we never received a close code
                            // in the first place, so notify to restart
                            return true;
                        }
                    };
                },
                OwnedMessage::Text(data) => {
                    println!("Received text message: {:?}", data);
                    maybe_error(inbound_sender.send(InboundMessage::Data(data)));
                },
                OwnedMessage::Binary(data) => {
                    println!("Received binary message: {:?}", data);
                    // inbound_socket.send(data, 0).unwrap();
                    // inbound_socket.recv(&mut inbound_msg, 0).unwrap();
                },
                _ => println!("Pong received"),
            }
        }
        // Base case, don't instruct to restart
        return false;
    });

    Ok((sender_thread, receiver_thread))
}