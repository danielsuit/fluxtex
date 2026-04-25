use std::sync::{Arc, Mutex};
use datachannel::{
    RtcPeerConnection, RtcConfig, RtcDataChannel,
    SessionDescription, DataChannelInfo, IceCandidate,
    SdpType
};
use crossbeam_channel::{Sender, Receiver, unbounded};

pub struct MyPcHandler {
    pub sdp_tx: Sender<String>,
}

impl datachannel::PeerConnectionHandler for MyPcHandler {
    type DCH = MyDcHandler;

    fn data_channel_handler(&mut self, _info: DataChannelInfo) -> Self::DCH {
        MyDcHandler
    }

    fn on_candidate(&mut self, _cand: IceCandidate) {
    }
    
    fn on_gathering_state_change(&mut self, state: datachannel::GatheringState) {
        if state == datachannel::GatheringState::Complete {
        }
    }
}

pub struct MyDcHandler;

impl datachannel::DataChannelHandler for MyDcHandler {
    fn on_message(&mut self, _msg: &[u8]) {
    }
}

pub struct WebRtcState {
    pub pc: Box<RtcPeerConnection<MyPcHandler>>,
    pub dc: Option<Box<RtcDataChannel<MyDcHandler>>>,
    pub rx: Receiver<String>,
}

impl WebRtcState {
    pub fn new() -> Result<Arc<Mutex<Self>>, String> {
        let config = RtcConfig::new::<&str>(&[]);
        let (tx, rx) = unbounded();
        
        let pc = RtcPeerConnection::new(&config, MyPcHandler { sdp_tx: tx })
            .map_err(|e| format!("Failed to create PC: {:?}", e))?;
            
        Ok(Arc::new(Mutex::new(Self { pc, dc: None, rx })))
    }
    
    pub fn host(&mut self) -> Result<(), String> {
        let dc = self.pc.create_data_channel("fluxtex_collab", MyDcHandler)
             .map_err(|e| format!("Could not create DC: {:?}", e))?;
             
        self.dc = Some(dc);
        
        self.pc.set_local_description(SdpType::Offer)
            .map_err(|e| format!("Could not set offer: {:?}", e))?;
            
        Ok(())
    }
}
