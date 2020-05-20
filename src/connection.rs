use std::sync::mpsc::{Sender, Receiver};
use websocket::ClientBuilder;
use websocket::header::{Header, HeaderFormat, Headers, Authorization, Basic};
use websocket::{OwnedMessage, Message};
use hyper::header::parsing::from_one_raw_str;
use std::thread;
use std::thread::JoinHandle;
use std::fmt;
use zmq;

use crate::models::{Request, ClientInformation};

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

pub fn initialize(
    client_information: ClientInformation,
    sender: Sender<Request>,
    receiver: Receiver<Request>,
    inbound_socket: zmq::Socket,
) -> (JoinHandle<()>, JoinHandle<()>) {
    websocket(client_information, sender, receiver, inbound_socket)
}

// TODO: add reconnect logic on disconnect
fn websocket(
    client_information: ClientInformation,
    sender: Sender<Request>,
    receiver: Receiver<Request>,
    inbound_socket: zmq::Socket,
) -> (JoinHandle<()>, JoinHandle<()>) {
    let mut headers = Headers::new();
    headers.set(
        Authorization(
            Basic {
                username: client_information.account_id.clone(),
                password: Some(client_information.api_key.clone()),
            }
        )
    );
    headers.set(DeviceIdHeader(client_information.device_id));
    headers.set(DeviceTypeIdHeader(client_information.device_type_id));
    let client = ClientBuilder::new("ws://localhost:8080/ws/")
        .unwrap()
        .custom_headers(&headers)
        .connect_insecure()
        .unwrap();

    let (mut client_receiver, mut client_sender) = client.split().unwrap();
    let sender_thread = thread::spawn(move || {
        loop {
            let request = receiver.recv().unwrap();

            match request {
                Request::Pong(data) => {
                    match client_sender.send_message(&OwnedMessage::Pong(data)) {
                        Ok(_) => println!("Successfully sent pong"),
                        Err(e) => {
                            println!("Error sending pong: {:?}", e);
                            let _ = client_sender.send_message(&Message::close());
                            return;
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
                    return;
                },
            }
        }
    });

    let mut inbound_msg = zmq::Message::new();

    let receiver_thread = thread::spawn(move || {
        for message in client_receiver.incoming_messages() {
            println!("message received: {:?}", message);
            let message = match message {
                Ok(m) => m,
                Err(e) => {
                    println!("Error in received message, closing connection: {:?}", e);
                    let _ = sender.send(Request::Close);
                    return;
                }
            };

            match message {
                OwnedMessage::Close(_) => {
                    let _ = sender.send(Request::Close);
                },
                OwnedMessage::Ping(data) => {
                    match sender.send(Request::Pong(data)) {
                        Ok(_) => (),
                        Err(e) => {
                            println!("Error in sending pong frame, closing connection: {:?}", e);
                            let _  = sender.send(Request::Close);
                            return;
                        }
                    };
                },
                OwnedMessage::Text(data) => {
                    println!("Received text message: {:?}", data);
                    match inbound_socket.send((&data).as_bytes(), 0) {
                        Ok(_) => (),
                        Err(e) => {
                            println!("ERROR SENDING: {:?}", e);
                            continue;
                        }
                    };
                },
                OwnedMessage::Binary(data) => {
                    println!("Received binary message: {:?}", data);
                    // inbound_socket.send(data, 0).unwrap();
                    // inbound_socket.recv(&mut inbound_msg, 0).unwrap();
                },
                _ => println!("Pong received"),
            }
        }
    });

    (sender_thread, receiver_thread)
}