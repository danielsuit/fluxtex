use crossbeam_channel::{unbounded, Receiver, Sender};
use datachannel::{
    ConnectionState, DataChannelInfo, IceCandidate, RtcConfig, RtcDataChannel, RtcPeerConnection,
    SdpType, SessionDescription,
};
use std::sync::{Arc, Mutex};

use crate::document::network::{decode_sync_message, encode_sync_message, SyncMessage};

#[derive(Debug)]
pub enum SignalingEvent {
    LocalDescription(SessionDescription),
    IceCandidate(IceCandidate),
    ConnectionState(ConnectionState),
    DataChannelOpen,
    DataChannelClosed,
    DataChannelError(String),
}

#[derive(Debug)]
pub enum WebRtcEvent {
    Signaling(SignalingEvent),
    SyncMessage(SyncMessage),
}

pub struct MyPcHandler {
    pub event_tx: Sender<WebRtcEvent>,
    pub dc_tx: Sender<Box<RtcDataChannel<MyDcHandler>>>,
}

impl datachannel::PeerConnectionHandler for MyPcHandler {
    type DCH = MyDcHandler;

    fn data_channel_handler(&mut self, _info: DataChannelInfo) -> Self::DCH {
        MyDcHandler {
            event_tx: self.event_tx.clone(),
        }
    }

    fn on_description(&mut self, sess_desc: SessionDescription) {
        let _ = self
            .event_tx
            .send(WebRtcEvent::Signaling(SignalingEvent::LocalDescription(
                sess_desc,
            )));
    }

    fn on_candidate(&mut self, cand: IceCandidate) {
        let _ = self
            .event_tx
            .send(WebRtcEvent::Signaling(SignalingEvent::IceCandidate(cand)));
    }

    fn on_connection_state_change(&mut self, state: ConnectionState) {
        let _ = self
            .event_tx
            .send(WebRtcEvent::Signaling(SignalingEvent::ConnectionState(
                state,
            )));
    }

    fn on_data_channel(&mut self, data_channel: Box<RtcDataChannel<Self::DCH>>) {
        let _ = self.dc_tx.send(data_channel);
    }
}

pub struct MyDcHandler {
    pub event_tx: Sender<WebRtcEvent>,
}

impl datachannel::DataChannelHandler for MyDcHandler {
    fn on_open(&mut self) {
        let _ = self
            .event_tx
            .send(WebRtcEvent::Signaling(SignalingEvent::DataChannelOpen));
    }

    fn on_closed(&mut self) {
        let _ = self
            .event_tx
            .send(WebRtcEvent::Signaling(SignalingEvent::DataChannelClosed));
    }

    fn on_error(&mut self, err: &str) {
        let _ = self
            .event_tx
            .send(WebRtcEvent::Signaling(SignalingEvent::DataChannelError(
                err.to_string(),
            )));
    }

    fn on_message(&mut self, msg: &[u8]) {
        match decode_sync_message(msg) {
            Ok(message) => {
                let _ = self.event_tx.send(WebRtcEvent::SyncMessage(message));
            }
            Err(err) => {
                let _ =
                    self.event_tx
                        .send(WebRtcEvent::Signaling(SignalingEvent::DataChannelError(
                            err,
                        )));
            }
        }
    }
}

pub struct WebRtcState {
    pub pc: Box<RtcPeerConnection<MyPcHandler>>,
    pub dc: Option<Box<RtcDataChannel<MyDcHandler>>>,
    event_tx: Sender<WebRtcEvent>,
    event_rx: Receiver<WebRtcEvent>,
    dc_rx: Receiver<Box<RtcDataChannel<MyDcHandler>>>,
}

impl WebRtcState {
    pub fn new() -> Result<Arc<Mutex<Self>>, String> {
        let config = RtcConfig::new::<&str>(&[]);
        let (event_tx, event_rx) = unbounded();
        let (dc_tx, dc_rx) = unbounded();

        let pc = RtcPeerConnection::new(
            &config,
            MyPcHandler {
                event_tx: event_tx.clone(),
                dc_tx,
            },
        )
        .map_err(|e| format!("Failed to create PC: {:?}", e))?;

        Ok(Arc::new(Mutex::new(Self {
            pc,
            dc: None,
            event_tx,
            event_rx,
            dc_rx,
        })))
    }

    pub fn host(&mut self) -> Result<(), String> {
        let dc = self
            .pc
            .create_data_channel(
                "fluxtex_collab",
                MyDcHandler {
                    event_tx: self.event_tx.clone(),
                },
            )
            .map_err(|e| format!("Could not create DC: {:?}", e))?;

        self.dc = Some(dc);

        self.pc
            .set_local_description(SdpType::Offer)
            .map_err(|e| format!("Could not set offer: {:?}", e))?;

        Ok(())
    }

    pub fn apply_remote_description(
        &mut self,
        sess_desc: &SessionDescription,
    ) -> Result<(), String> {
        let should_answer = matches!(sess_desc.sdp_type, SdpType::Offer);
        self.pc
            .set_remote_description(sess_desc)
            .map_err(|e| format!("Could not apply remote description: {:?}", e))?;

        if should_answer {
            self.pc
                .set_local_description(SdpType::Answer)
                .map_err(|e| format!("Could not create answer: {:?}", e))?;
        }

        self.sync_data_channel();
        Ok(())
    }

    pub fn add_remote_candidate(&mut self, candidate: &IceCandidate) -> Result<(), String> {
        self.pc
            .add_remote_candidate(candidate)
            .map_err(|e| format!("Could not add ICE candidate: {:?}", e))
    }

    pub fn send_sync_message(&mut self, message: &SyncMessage) -> Result<(), String> {
        self.sync_data_channel();
        let payload = encode_sync_message(message)?;
        let dc = self
            .dc
            .as_mut()
            .ok_or_else(|| "No open data channel available".to_string())?;

        dc.send(&payload)
            .map_err(|e| format!("Could not send sync message: {:?}", e))
    }

    pub fn drain_events(&mut self) -> Vec<WebRtcEvent> {
        self.sync_data_channel();
        self.event_rx.try_iter().collect()
    }

    fn sync_data_channel(&mut self) {
        while let Ok(dc) = self.dc_rx.try_recv() {
            self.dc = Some(dc);
        }
    }
}
