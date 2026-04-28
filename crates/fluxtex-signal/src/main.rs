use fluxtex_signal::{ClientMessage, ServerMessage, DEFAULT_ADDR};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio::sync::Mutex as AsyncMutex;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use uuid::Uuid;

type ClientTx = UnboundedSender<ServerMessage>;

#[derive(Default)]
struct Room {
    host: Option<ClientTx>,
    peer: Option<ClientTx>,
}

type Rooms = Arc<AsyncMutex<HashMap<String, Room>>>;

#[derive(Copy, Clone, Debug)]
enum Role {
    Unset,
    Host,
    Peer,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let addr = std::env::var("FLUXTEX_SIGNAL_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    let listener = TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {addr}: {e}"));
    tracing::info!(%addr, "FluXTeX signaling server listening");

    let rooms: Rooms = Arc::new(AsyncMutex::new(HashMap::new()));

    while let Ok((stream, peer_addr)) = listener.accept().await {
        let rooms = rooms.clone();
        tokio::spawn(handle_connection(stream, peer_addr, rooms));
    }
}

async fn handle_connection(stream: TcpStream, addr: std::net::SocketAddr, rooms: Rooms) {
    let ws = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            tracing::warn!(%addr, error = %e, "websocket handshake failed");
            return;
        }
    };
    tracing::info!(%addr, "client connected");

    let (mut sink, mut source) = ws.split();
    let (tx, mut rx) = unbounded_channel::<ServerMessage>();

    // Writer task: pulls outbound ServerMessages and writes them to the socket.
    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let json = match serde_json::to_string(&msg) {
                Ok(s) => s,
                Err(_) => continue,
            };
            if sink.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
        let _ = sink.send(Message::Close(None)).await;
    });

    let mut my_room: Option<String> = None;
    let mut my_role = Role::Unset;

    while let Some(msg_result) = source.next().await {
        let text = match msg_result {
            Ok(Message::Text(t)) => t,
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(e) => {
                tracing::warn!(%addr, error = %e, "websocket read error");
                break;
            }
        };

        let parsed: ClientMessage = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                let _ = tx.send(ServerMessage::Error {
                    reason: format!("malformed message: {e}"),
                });
                continue;
            }
        };

        match parsed {
            ClientMessage::Host => {
                let code = generate_code();
                let mut rooms_lock = rooms.lock().await;
                rooms_lock.insert(
                    code.clone(),
                    Room {
                        host: Some(tx.clone()),
                        peer: None,
                    },
                );
                drop(rooms_lock);
                my_room = Some(code.clone());
                my_role = Role::Host;
                tracing::info!(%addr, %code, "hosting room");
                let _ = tx.send(ServerMessage::RoomCreated { code });
            }
            ClientMessage::Join { code } => {
                let code_norm = code.to_uppercase();
                let mut rooms_lock = rooms.lock().await;
                match rooms_lock.get_mut(&code_norm) {
                    Some(room) if room.host.is_some() && room.peer.is_none() => {
                        room.peer = Some(tx.clone());
                        let host_tx = room.host.clone();
                        drop(rooms_lock);
                        let _ = tx.send(ServerMessage::JoinAck);
                        if let Some(host_tx) = host_tx {
                            let _ = host_tx.send(ServerMessage::PeerJoined);
                        }
                        my_room = Some(code_norm.clone());
                        my_role = Role::Peer;
                        tracing::info!(%addr, code = %code_norm, "joined room");
                    }
                    Some(_) => {
                        let _ = tx.send(ServerMessage::Error {
                            reason: format!("room {code_norm} already has two peers"),
                        });
                    }
                    None => {
                        let _ = tx.send(ServerMessage::Error {
                            reason: format!("room {code_norm} not found"),
                        });
                    }
                }
            }
            ClientMessage::Signal { payload } => {
                let Some(code) = my_room.clone() else {
                    let _ = tx.send(ServerMessage::Error {
                        reason: "must Host or Join before signaling".to_string(),
                    });
                    continue;
                };
                let rooms_lock = rooms.lock().await;
                let Some(room) = rooms_lock.get(&code) else {
                    continue;
                };
                let target = match my_role {
                    Role::Host => room.peer.clone(),
                    Role::Peer => room.host.clone(),
                    Role::Unset => None,
                };
                drop(rooms_lock);
                if let Some(target_tx) = target {
                    let _ = target_tx.send(ServerMessage::Signal { payload });
                }
            }
        }
    }

    // Connection ended — notify peer and clean up the room.
    if let Some(code) = my_room {
        let mut rooms_lock = rooms.lock().await;
        if let Some(room) = rooms_lock.get_mut(&code) {
            match my_role {
                Role::Host => {
                    if let Some(peer_tx) = room.peer.take() {
                        let _ = peer_tx.send(ServerMessage::PeerLeft);
                    }
                    rooms_lock.remove(&code);
                    tracing::info!(%addr, %code, "host left, room closed");
                }
                Role::Peer => {
                    if let Some(host_tx) = &room.host {
                        let _ = host_tx.send(ServerMessage::PeerLeft);
                    }
                    room.peer = None;
                    tracing::info!(%addr, %code, "peer left room");
                }
                Role::Unset => {}
            }
        }
    }

    drop(tx);
    let _ = writer.await;
    tracing::info!(%addr, "client disconnected");
}

fn generate_code() -> String {
    let uuid = Uuid::new_v4();
    let bytes = uuid.as_bytes();
    format!("{:02X}{:02X}{:02X}", bytes[0], bytes[1], bytes[2])
}
