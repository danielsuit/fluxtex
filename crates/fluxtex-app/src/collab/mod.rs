pub mod signaling_client;

use std::sync::{Arc, Mutex};

use crate::document::buffer::DocumentBuffer;
use crate::document::buffer_diff::apply_text_diff;
use crate::document::network::{apply_sync_message, SyncMessage};
use crate::document::webrtc::{SignalingEvent, WebRtcEvent, WebRtcState};
pub use signaling_client::{SignalingClient, SignalingClientEvent, SignalingRole};

/// State exposed to the UI for the running collaboration session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollabStatus {
    Idle,
    ConnectingToSignaling,
    WaitingForPeer,
    Joining,
    Negotiating,
    Connected,
    Disconnected(String),
}

/// One drained iteration's worth of events, surfaced to the UI/edit pipeline.
#[derive(Debug, Clone)]
pub enum CollabEvent {
    StatusChanged(CollabStatus),
    RoomCode(String),
    PeerJoined,
    PeerLeft,
    /// A remote sync message that has been integrated into `DocumentBuffer`
    /// already; the caller is responsible for reflecting the buffer into the
    /// editor view.
    RemoteSyncApplied,
    Error(String),
    Info(String),
}

pub struct CollaborationSession {
    rtc: Arc<Mutex<WebRtcState>>,
    signaling: SignalingClient,
    role: SignalingRole,
    status: CollabStatus,
    data_channel_open: bool,
}

impl CollaborationSession {
    pub fn host(server_addr: String) -> Result<Self, String> {
        let signaling = SignalingClient::connect(server_addr, SignalingRole::Host, None);
        Ok(Self {
            rtc: WebRtcState::new()?,
            signaling,
            role: SignalingRole::Host,
            status: CollabStatus::ConnectingToSignaling,
            data_channel_open: false,
        })
    }

    pub fn join(server_addr: String, code: String) -> Result<Self, String> {
        let signaling = SignalingClient::connect(server_addr, SignalingRole::Join, Some(code));
        Ok(Self {
            rtc: WebRtcState::new()?,
            signaling,
            role: SignalingRole::Join,
            status: CollabStatus::ConnectingToSignaling,
            data_channel_open: false,
        })
    }

    /// Pull pending events from the signaling client + WebRTC state, advance
    /// internal state, and apply remote sync messages to `doc_buffer`. Returns
    /// the externally-visible events the caller should react to.
    pub fn drain(&mut self, doc_buffer: &mut DocumentBuffer) -> Vec<CollabEvent> {
        let mut out = Vec::new();
        let mut new_status: Option<CollabStatus> = None;
        let mut remote_applied = false;

        // Inbound signaling.
        while let Ok(ev) = self.signaling.events.try_recv() {
            match ev {
                SignalingClientEvent::Connecting => {
                    new_status = Some(CollabStatus::ConnectingToSignaling);
                }
                SignalingClientEvent::Connected => {
                    new_status = Some(match self.role {
                        SignalingRole::Host => CollabStatus::ConnectingToSignaling,
                        SignalingRole::Join => CollabStatus::Joining,
                    });
                }
                SignalingClientEvent::RoomCreated { code } => {
                    new_status = Some(CollabStatus::WaitingForPeer);
                    out.push(CollabEvent::RoomCode(code));
                }
                SignalingClientEvent::JoinAcknowledged => {
                    new_status = Some(CollabStatus::Negotiating);
                    out.push(CollabEvent::Info("paired with host".to_string()));
                }
                SignalingClientEvent::PeerJoined => {
                    out.push(CollabEvent::PeerJoined);
                    new_status = Some(CollabStatus::Negotiating);
                    if matches!(self.role, SignalingRole::Host) {
                        match self.rtc.lock().unwrap().host() {
                            Ok(()) => {}
                            Err(e) => {
                                out.push(CollabEvent::Error(format!("host failed: {e}")));
                            }
                        }
                    }
                }
                SignalingClientEvent::PeerLeft => {
                    self.data_channel_open = false;
                    out.push(CollabEvent::PeerLeft);
                    new_status = Some(CollabStatus::Disconnected("peer left".to_string()));
                }
                SignalingClientEvent::RelayedSdp(sdp) => {
                    if let Err(e) = self.rtc.lock().unwrap().apply_remote_description(&sdp) {
                        out.push(CollabEvent::Error(format!("apply sdp: {e}")));
                    }
                }
                SignalingClientEvent::RelayedIce(ice) => {
                    if let Err(e) = self.rtc.lock().unwrap().add_remote_candidate(&ice) {
                        out.push(CollabEvent::Error(format!("apply ice: {e}")));
                    }
                }
                SignalingClientEvent::Error(reason) => {
                    out.push(CollabEvent::Error(reason));
                }
                SignalingClientEvent::Disconnected => {
                    out.push(CollabEvent::Info(
                        "signaling channel closed (peer connection unaffected if open)".to_string(),
                    ));
                    if !self.data_channel_open {
                        new_status =
                            Some(CollabStatus::Disconnected("signaling closed".to_string()));
                    }
                }
            }
        }

        // Inbound from WebRTC (data channel events + signaling events to forward).
        let webrtc_events = self.rtc.lock().unwrap().drain_events();
        for ev in webrtc_events {
            match ev {
                WebRtcEvent::Signaling(SignalingEvent::LocalDescription(sdp)) => {
                    self.signaling.send_sdp(sdp);
                }
                WebRtcEvent::Signaling(SignalingEvent::IceCandidate(ice)) => {
                    self.signaling.send_ice(ice);
                }
                WebRtcEvent::Signaling(SignalingEvent::DataChannelOpen) => {
                    self.data_channel_open = true;
                    new_status = Some(CollabStatus::Connected);
                    out.push(CollabEvent::Info("data channel open".to_string()));
                    // Host seeds the joiner so both peers share a baseline replica.
                    if matches!(self.role, SignalingRole::Host) {
                        let snapshot = SyncMessage::Snapshot {
                            encoded: doc_buffer.snapshot(),
                            content: doc_buffer.content.clone(),
                        };
                        if let Err(e) = self.rtc.lock().unwrap().send_sync_message(&snapshot) {
                            out.push(CollabEvent::Error(format!("send snapshot: {e}")));
                        }
                    }
                }
                WebRtcEvent::Signaling(SignalingEvent::DataChannelClosed) => {
                    self.data_channel_open = false;
                    new_status = Some(CollabStatus::Disconnected(
                        "data channel closed".to_string(),
                    ));
                }
                WebRtcEvent::Signaling(SignalingEvent::DataChannelError(e)) => {
                    out.push(CollabEvent::Error(format!("data channel: {e}")));
                }
                WebRtcEvent::Signaling(SignalingEvent::ConnectionState(state)) => {
                    out.push(CollabEvent::Info(format!("ice state: {state:?}")));
                }
                WebRtcEvent::SyncMessage(msg) => {
                    apply_sync_message(doc_buffer, msg);
                    remote_applied = true;
                }
            }
        }

        if remote_applied {
            out.push(CollabEvent::RemoteSyncApplied);
        }

        if let Some(status) = new_status {
            if status != self.status {
                self.status = status.clone();
                out.push(CollabEvent::StatusChanged(status));
            }
        }

        out
    }

    /// Compute and broadcast local edits. Updates the local CRDT state
    /// regardless of whether the data channel is open.
    pub fn broadcast_local_edit(
        &mut self,
        doc_buffer: &mut DocumentBuffer,
        old_text: &str,
        new_text: &str,
    ) -> Vec<SyncMessage> {
        let messages = apply_text_diff(doc_buffer, old_text, new_text);
        if !self.data_channel_open || messages.is_empty() {
            return messages;
        }
        let mut rtc = self.rtc.lock().unwrap();
        for msg in &messages {
            if let Err(e) = rtc.send_sync_message(msg) {
                tracing::warn!(error = %e, "failed to broadcast sync message");
            }
        }
        messages
    }

    pub fn shutdown(&mut self) {
        self.signaling.disconnect();
        self.data_channel_open = false;
        self.status = CollabStatus::Disconnected("disconnected by user".to_string());
    }
}
