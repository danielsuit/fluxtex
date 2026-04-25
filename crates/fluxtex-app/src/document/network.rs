use crate::document::buffer::DocumentBuffer;
use cola::{Deletion, Insertion};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncMessage {
    Insert { insertion: Insertion, text: String },
    Delete { deletion: Deletion },
}

pub fn encode_sync_message(message: &SyncMessage) -> Result<Vec<u8>, String> {
    bincode::serde::encode_to_vec(message, bincode::config::standard())
        .map_err(|err| format!("Failed to encode sync message: {err}"))
}

pub fn decode_sync_message(bytes: &[u8]) -> Result<SyncMessage, String> {
    bincode::serde::decode_from_slice(bytes, bincode::config::standard())
        .map(|(message, _)| message)
        .map_err(|err| format!("Failed to decode sync message: {err}"))
}

pub fn apply_sync_message(doc_buffer: &mut DocumentBuffer, message: SyncMessage) {
    match message {
        SyncMessage::Insert { insertion, text } => {
            doc_buffer.integrate_remote_insert(insertion, &text);
        }
        SyncMessage::Delete { deletion } => {
            doc_buffer.integrate_remote_delete(deletion);
        }
    }
}
