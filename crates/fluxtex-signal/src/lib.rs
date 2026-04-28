//! Wire-protocol types shared by the FluXTeX signaling server and the app
//! client. Messages are JSON-encoded over a single WebSocket connection.

use serde::{Deserialize, Serialize};

/// Default address the server listens on and clients connect to.
pub const DEFAULT_ADDR: &str = "127.0.0.1:9000";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Ask the server to create a new room and act as host.
    Host,
    /// Join the room with the given short code.
    Join { code: String },
    /// Forward an opaque payload to the other peer in this room.
    /// Used to relay SDP descriptions and ICE candidates.
    Signal { payload: serde_json::Value },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Confirms host registration; clients display this code.
    RoomCreated { code: String },
    /// Sent to the host when a second peer joins.
    PeerJoined,
    /// Sent to the joining peer once the room match succeeds.
    JoinAck,
    /// Sent to the surviving peer when the other side disconnects.
    PeerLeft,
    /// Forwarded relay payload from the other peer.
    Signal { payload: serde_json::Value },
    /// Out-of-band error (bad code, malformed message, etc.).
    Error { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn client_message_host_shape() {
        let msg = ClientMessage::Host;
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&msg).unwrap()).unwrap();
        assert_eq!(value, json!({ "type": "host" }));
    }

    #[test]
    fn client_message_join_shape() {
        let msg = ClientMessage::Join {
            code: "ABC123".to_string(),
        };
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&msg).unwrap()).unwrap();
        assert_eq!(value, json!({ "type": "join", "code": "ABC123" }));
    }

    #[test]
    fn server_message_room_created_shape() {
        let msg = ServerMessage::RoomCreated {
            code: "ABC123".to_string(),
        };
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&msg).unwrap()).unwrap();
        assert_eq!(value, json!({ "type": "room_created", "code": "ABC123" }));
    }

    #[test]
    fn signal_payload_is_opaque_json() {
        // The server forwards `Signal { payload }` blobs untouched between
        // peers. This guards against accidentally constraining the payload
        // schema (which would break peer-to-peer SDP/ICE relay).
        let msg = ClientMessage::Signal {
            payload: json!({ "kind": "sdp", "anything": [1, 2, 3] }),
        };
        let bytes = serde_json::to_string(&msg).unwrap();
        let decoded: ClientMessage = serde_json::from_str(&bytes).unwrap();
        match decoded {
            ClientMessage::Signal { payload } => {
                assert_eq!(payload["kind"], "sdp");
                assert_eq!(payload["anything"][2], 3);
            }
            _ => panic!("expected Signal"),
        }
    }
}
