//! WebSocket client that talks to the FluXTeX signaling server. Runs on its
//! own thread + tokio runtime; the rest of the app communicates with it via
//! crossbeam channels.

use crossbeam_channel::{unbounded, Receiver, Sender};
use datachannel::{IceCandidate, SessionDescription};
use fluxtex_signal::{ClientMessage, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalingRole {
    Host,
    Join,
}

#[derive(Debug)]
pub enum SignalingClientEvent {
    Connecting,
    Connected,
    RoomCreated { code: String },
    JoinAcknowledged,
    PeerJoined,
    PeerLeft,
    RelayedSdp(Box<SessionDescription>),
    RelayedIce(Box<IceCandidate>),
    Error(String),
    Disconnected,
}

pub enum SignalingClientCommand {
    SendSdp(Box<SessionDescription>),
    SendIce(Box<IceCandidate>),
    Disconnect,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RelayPayload {
    Sdp(Box<SessionDescription>),
    Ice(Box<IceCandidate>),
}

pub struct SignalingClient {
    pub events: Receiver<SignalingClientEvent>,
    cmd_tx: Sender<SignalingClientCommand>,
}

impl SignalingClient {
    pub fn connect(server_addr: String, role: SignalingRole, join_code: Option<String>) -> Self {
        let (event_tx, events) = unbounded::<SignalingClientEvent>();
        let (cmd_tx, cmd_rx) = unbounded::<SignalingClientCommand>();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ =
                        event_tx.send(SignalingClientEvent::Error(format!("tokio runtime: {e}")));
                    let _ = event_tx.send(SignalingClientEvent::Disconnected);
                    return;
                }
            };
            rt.block_on(client_loop(server_addr, role, join_code, cmd_rx, event_tx));
        });

        Self { events, cmd_tx }
    }

    pub fn send_sdp(&self, sdp: Box<SessionDescription>) {
        let _ = self.cmd_tx.send(SignalingClientCommand::SendSdp(sdp));
    }

    pub fn send_ice(&self, ice: Box<IceCandidate>) {
        let _ = self.cmd_tx.send(SignalingClientCommand::SendIce(ice));
    }

    pub fn disconnect(&self) {
        let _ = self.cmd_tx.send(SignalingClientCommand::Disconnect);
    }
}

async fn client_loop(
    server_addr: String,
    role: SignalingRole,
    join_code: Option<String>,
    cmd_rx: Receiver<SignalingClientCommand>,
    event_tx: Sender<SignalingClientEvent>,
) {
    let _ = event_tx.send(SignalingClientEvent::Connecting);

    let url = if server_addr.starts_with("ws://") || server_addr.starts_with("wss://") {
        server_addr.clone()
    } else {
        format!("ws://{server_addr}")
    };

    let ws = match tokio_tungstenite::connect_async(&url).await {
        Ok((ws, _)) => ws,
        Err(e) => {
            let _ = event_tx.send(SignalingClientEvent::Error(format!("connect failed: {e}")));
            let _ = event_tx.send(SignalingClientEvent::Disconnected);
            return;
        }
    };
    let _ = event_tx.send(SignalingClientEvent::Connected);

    let (mut sink, mut source) = ws.split();

    // Initial Host or Join.
    let initial = match role {
        SignalingRole::Host => ClientMessage::Host,
        SignalingRole::Join => ClientMessage::Join {
            code: join_code.clone().unwrap_or_default(),
        },
    };
    if let Err(e) = send_client_message(&mut sink, &initial).await {
        let _ = event_tx.send(SignalingClientEvent::Error(format!("send hello: {e}")));
        let _ = event_tx.send(SignalingClientEvent::Disconnected);
        return;
    }

    // Bridge crossbeam cmd_rx into a tokio mpsc so we can select! on it.
    let (bridge_tx, mut bridge_rx) =
        tokio::sync::mpsc::unbounded_channel::<SignalingClientCommand>();
    tokio::task::spawn_blocking(move || {
        while let Ok(cmd) = cmd_rx.recv() {
            if bridge_tx.send(cmd).is_err() {
                break;
            }
        }
    });

    loop {
        tokio::select! {
            incoming = source.next() => {
                let msg = match incoming {
                    Some(Ok(Message::Text(t))) => t,
                    Some(Ok(Message::Close(_))) | None => {
                        let _ = event_tx.send(SignalingClientEvent::Disconnected);
                        return;
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => {
                        let _ = event_tx
                            .send(SignalingClientEvent::Error(format!("read: {e}")));
                        let _ = event_tx.send(SignalingClientEvent::Disconnected);
                        return;
                    }
                };

                let server_msg: ServerMessage = match serde_json::from_str(&msg) {
                    Ok(m) => m,
                    Err(e) => {
                        let _ = event_tx
                            .send(SignalingClientEvent::Error(format!("bad server msg: {e}")));
                        continue;
                    }
                };

                match server_msg {
                    ServerMessage::RoomCreated { code } => {
                        let _ = event_tx.send(SignalingClientEvent::RoomCreated { code });
                    }
                    ServerMessage::PeerJoined => {
                        let _ = event_tx.send(SignalingClientEvent::PeerJoined);
                    }
                    ServerMessage::JoinAck => {
                        let _ = event_tx.send(SignalingClientEvent::JoinAcknowledged);
                    }
                    ServerMessage::PeerLeft => {
                        let _ = event_tx.send(SignalingClientEvent::PeerLeft);
                    }
                    ServerMessage::Error { reason } => {
                        let _ = event_tx.send(SignalingClientEvent::Error(reason));
                    }
                    ServerMessage::Signal { payload } => {
                        match serde_json::from_value::<RelayPayload>(payload) {
                            Ok(RelayPayload::Sdp(sdp)) => {
                                let _ = event_tx.send(SignalingClientEvent::RelayedSdp(sdp));
                            }
                            Ok(RelayPayload::Ice(ice)) => {
                                let _ = event_tx.send(SignalingClientEvent::RelayedIce(ice));
                            }
                            Err(e) => {
                                let _ = event_tx.send(SignalingClientEvent::Error(format!(
                                    "bad relay payload: {e}"
                                )));
                            }
                        }
                    }
                }
            }

            cmd = bridge_rx.recv() => {
                let Some(cmd) = cmd else { return; };
                match cmd {
                    SignalingClientCommand::SendSdp(sdp) => {
                        let payload = match serde_json::to_value(RelayPayload::Sdp(sdp)) {
                            Ok(v) => v,
                            Err(e) => {
                                let _ = event_tx.send(SignalingClientEvent::Error(format!(
                                    "encode sdp: {e}"
                                )));
                                continue;
                            }
                        };
                        if let Err(e) =
                            send_client_message(&mut sink, &ClientMessage::Signal { payload }).await
                        {
                            let _ = event_tx
                                .send(SignalingClientEvent::Error(format!("send sdp: {e}")));
                        }
                    }
                    SignalingClientCommand::SendIce(ice) => {
                        let payload = match serde_json::to_value(RelayPayload::Ice(ice)) {
                            Ok(v) => v,
                            Err(e) => {
                                let _ = event_tx.send(SignalingClientEvent::Error(format!(
                                    "encode ice: {e}"
                                )));
                                continue;
                            }
                        };
                        if let Err(e) =
                            send_client_message(&mut sink, &ClientMessage::Signal { payload }).await
                        {
                            let _ = event_tx
                                .send(SignalingClientEvent::Error(format!("send ice: {e}")));
                        }
                    }
                    SignalingClientCommand::Disconnect => {
                        let _ = sink.send(Message::Close(None)).await;
                        let _ = event_tx.send(SignalingClientEvent::Disconnected);
                        return;
                    }
                }
            }
        }
    }
}

async fn send_client_message<S>(sink: &mut S, msg: &ClientMessage) -> Result<(), String>
where
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let json = serde_json::to_string(msg).map_err(|e| e.to_string())?;
    sink.send(Message::Text(json))
        .await
        .map_err(|e| e.to_string())
}
