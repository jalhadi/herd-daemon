### Installation

#### Building

1. [Install Rust](https://www.rust-lang.org/tools/install) `1.43.0` or later
2. Run `git clone https://github.com/jalhadi/herd-daemon.git && cd herd-daemon`
3. Build the daemon from source `cargo build --release`
4. The binary can now be found as `/target/release/herd-daemon` and can be distributed to your device

#### Running

There are five command line arguments that can be passed into the daemon:

| Argument           | Required | Description                                                                                                                                                             |
| ------------------ | :------: | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| account_id (a)     |   true   | your account id from, comes from your dashboard                                                                                                                         |
| api_key (k)        |   true   | your api key, can be found on the settings page in your dashboard                                                                                                       |
| device_type_id (d) |   true   | the id of a device type that you registered on the dashboard, can be found on the devices page of your dashboard                                                        |
| outbound_port (o)  |  false   | Defaults to port 5555. For sending messages from your device. You will need to create a ZeroMQ connection to this port to send information from your device.            |
| inbound_port (i)   |  false   | Defaults to port 5556. For receiving messages sent to your device. You will need to create a ZeroMQ connection to this port to receive information sent to your device. |

To run the daemon, you just need to run the following command:
`./herd-daemon -a {ACCOUNT_ID} -k {API_KEY} -d {DEVICE_TYPE_ID} -o {INBOUND_PORT} -i {OUTBOUND_PORT}`

#### Communicating with daemon

Herd uses [ZeroMQ](https://zeromq.org/) for communication between your device and the daemon. ZeroMQ is an open source messaging library with many well supported [bindings](https://zeromq.org/get-started/) for popular languages. The Herd daemon opens two ZeroMQ sockets, an outbound an inbound socket. There are a few different types of messaging patterns available in ZeroMQ, but Herd uses only two of them (Pub/Sub and Push/Pull).

##### Outbound socket

The outbound socket uses the Push/Pull pattern. Once your daemon is running, you can communicate with it like follows:

```
# Example outbound communication using
# the python ZeroMQ library pyzmq

import zmq
import sys

# ZeroMQ Context
context = zmq.Context()

# Define the socket using the "Context"
sock = context.socket(zmq.PUSH)

# 5555 is the value of outbound_port specified when
# starting the daemon. The value below should reflect
# your custom value.
sock.connect("tcp://localhost:5555")

# Send a message using the socket
while True:
    # Wait for command line input
    message = input(">")

    # Send the string received from the
    # command line over the socket
    sock.send_string(message, encoding="utf-8")
```

###### Message types

There are three types of messages of messages that you can send to the daemon: Close, Register, Data.

**Close**:
This message tells the daemon to close the connection with the Herd servers. Sending the JSON with the type `Close` does this.

```
{
  "type": "Close"
}
```

**Register**:
Register allows your websocket to register to different topics that you have defined in your dashboard. This message type is a JSON with keys `type` and `topics`.

```
{
  "type": "Register",
  "topics": [
    "top_abc123",
    "top_foobar"
  ],
}
```

In the example above, we are saying that we want to register this device to all messages that are sent to topic "top_abc123". You can subscribe to any number of topics that you have made within your dashboard.

**Data**:
Message allows you to send data to other devices and webhooks. This messag type is a JSON with keys `type`, `topics`, and `data`.

```
{
  "type": "Data",
  "topics": [
    "top_abc123",
    "top_foobar"
  ],
  "data": {
    "hey":"there"
  }
}
```

In the example above, we are saying that we want to send `data` to all devices that are subscribed to either "top_abc123" or "top_foobar". **Note:** if a device is subscribed to multipled topics defined in a message, it will still only receive the message once.

##### Inbound socket

The inbound socket uses the Pub/Sub pattern. Once your daemon is running, you can receive messages with it like follows:

```
# Example inbound communication using
# the python ZeroMQ library pyzmq

import zmq

# ZeroMQ Context
context = zmq.Context()

# Define the socket using the "Context"
sock = context.socket(zmq.SUB)

# 5556 is the value of inbound_port specified when
# starting the daemon. The value below should reflect
# your custom value.
sock.connect("tcp://localhost:5556")

# Subscribe to all topics
sock.subscribe("")

# Send a "message" using the socket
while True:
    # Wait for new message from the socket
    inbound_message = sock.recv()

    # Print message received from socket
    print('Received message: {}'.format(inbound_message))
```

###### Message types

There are three different data message types that can be sent from the daemon to your application: data, restart, and close.

**data**:
The data message is a JSON representing data published by a device or websocket.

```
{
    "sender": {
        "device_id": "dev_abc123", # The id of the device that sent the message
        "device_type_id": "devt_foobar # The type of id that sent the message
    },
    "account_id": {your_account_id},
    "message": {
        "seconds_since_unix": 123456789, # The Unix timestamp when the message was sent
        "nano_seconds": 987654321 # The nano second precision part of the Unix timestamp when the message was sent
        "topics": [
            # List of topic ids that this message was published to
            "top_def456",
            "top_barbaz"
        ],
        "data": {
            # A JSON structure representing the data that was sent
            "json": "derulo"
        }
    }
}
```

**restart**:
The restart message is the JSON `{ type: "Restart" }`. The purpose of this message type is to inform the client when the daemon is attempting to restart the connection with the Herd servers. This message will be received upon sudden connection loss or new api server deployment. The daemon will attempt to restart the connection a maximum of 10 times, with 5 seconds of waiting between each attempt. If the daemon is unsuccessful in restarting the connection, it will eventually send the `close` message to the client.

**close**:
The close message is the JSON `{ type: "Close" }`. The purpose of this message is to notify the client when the daemon is shutting down, which can be due to the client sending `close` to the daemon or due to unsuccessfully connecting/restarting connection with the Herd servers.
