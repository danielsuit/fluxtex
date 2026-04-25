use std::sync::{Arc, Mutex};

use crate::document::buffer::DocumentBuffer;
use crate::document::buffer_diff::apply_text_diff;
use crate::document::network::{apply_sync_message, SyncMessage};
use crate::document::webrtc::{WebRtcEvent, WebRtcState};
use datachannel::{IceCandidate, SessionDescription};

pub struct CollaborationSession {
    rtc: Arc<Mutex<WebRtcState>>,
}

impl CollaborationSession {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            rtc: WebRtcState::new()?,
        })
    }

    pub fn host(&self) -> Result<(), String> {
        self.rtc.lock().unwrap().host()
    }

    pub fn apply_remote_description(&self, sess_desc: &SessionDescription) -> Result<(), String> {
        self.rtc.lock().unwrap().apply_remote_description(sess_desc)
    }

    pub fn add_remote_candidate(&self, candidate: &IceCandidate) -> Result<(), String> {
        self.rtc.lock().unwrap().add_remote_candidate(candidate)
    }

    pub fn broadcast_text_diff(
        &self,
        doc_buffer: &mut DocumentBuffer,
        old_text: &str,
        new_text: &str,
    ) -> Result<Vec<SyncMessage>, String> {
        let messages = apply_text_diff(doc_buffer, old_text, new_text);
        let mut rtc = self.rtc.lock().unwrap();

        for message in &messages {
            rtc.send_sync_message(message)?;
        }

        Ok(messages)
    }

    pub fn drain_remote_changes(&self, doc_buffer: &mut DocumentBuffer) -> Vec<WebRtcEvent> {
        let mut rtc = self.rtc.lock().unwrap();
        let events = rtc.drain_events();
        drop(rtc);

        for event in &events {
            if let WebRtcEvent::SyncMessage(message) = event {
                apply_sync_message(doc_buffer, message.clone());
            }
        }

        events
    }
}
