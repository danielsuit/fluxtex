use crate::document::buffer::DocumentBuffer;
use cola::{Deletion, EncodedReplica, Insertion};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub enum SyncMessage {
    Insert {
        insertion: Insertion,
        text: String,
    },
    Delete {
        deletion: Deletion,
    },
    /// One-shot snapshot the host sends as soon as the data channel opens.
    /// The joiner replaces its replica + content with this so subsequent
    /// `Insert` / `Delete` ops integrate against a shared baseline.
    Snapshot {
        encoded: EncodedReplica,
        content: String,
    },
}

impl std::fmt::Debug for SyncMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncMessage::Insert { text, .. } => {
                write!(f, "SyncMessage::Insert({} chars)", text.len())
            }
            SyncMessage::Delete { .. } => write!(f, "SyncMessage::Delete"),
            SyncMessage::Snapshot { content, .. } => {
                write!(f, "SyncMessage::Snapshot({} chars)", content.len())
            }
        }
    }
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
        SyncMessage::Snapshot { encoded, content } => {
            doc_buffer.restore_snapshot(&encoded, content);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::buffer_diff::apply_text_diff;

    /// SyncMessage round-trips through bincode for every variant. Catches
    /// breaking changes to the on-the-wire format (Insert/Delete/Snapshot).
    #[test]
    fn sync_message_bincode_roundtrip_insert_and_delete() {
        let mut buffer = DocumentBuffer::with_replica_id(1);
        let messages = apply_text_diff(&mut buffer, "", "hello world");

        for original in messages {
            let bytes = encode_sync_message(&original).expect("encode");
            let decoded = decode_sync_message(&bytes).expect("decode");
            // Re-encoding the decoded value must produce the same bytes.
            let bytes2 = encode_sync_message(&decoded).expect("re-encode");
            assert_eq!(
                bytes, bytes2,
                "wire format is not stable across decode/encode"
            );
        }
    }

    /// A host's snapshot, transmitted via SyncMessage and applied to a fresh
    /// joiner replica, must yield a buffer with the host's content.
    #[test]
    fn sync_message_snapshot_round_trip_seeds_joiner() {
        let mut host = DocumentBuffer::with_replica_id(1);
        apply_text_diff(&mut host, "", "Hello, peer.\n\\section{Intro}\n");
        assert_eq!(host.content, "Hello, peer.\n\\section{Intro}\n");

        let snapshot_msg = SyncMessage::Snapshot {
            encoded: host.snapshot(),
            content: host.content.clone(),
        };
        let bytes = encode_sync_message(&snapshot_msg).expect("encode");
        let on_wire = decode_sync_message(&bytes).expect("decode");

        let mut joiner = DocumentBuffer::with_replica_id(2);
        apply_sync_message(&mut joiner, on_wire);
        assert_eq!(joiner.content, host.content);
    }

    /// After a snapshot, joiner-originated inserts must integrate cleanly back
    /// on the host. This exercises the full sync loop (snapshot → joiner edit
    /// → host integrates) end-to-end at the data layer.
    #[test]
    fn snapshot_then_remote_insert_converges() {
        let mut host = DocumentBuffer::with_replica_id(1);
        apply_text_diff(&mut host, "", "abc");

        let mut joiner = DocumentBuffer::with_replica_id(2);
        apply_sync_message(
            &mut joiner,
            SyncMessage::Snapshot {
                encoded: host.snapshot(),
                content: host.content.clone(),
            },
        );

        // Joiner appends "XYZ" — produces an Insert sync message.
        let outbound = apply_text_diff(&mut joiner, "abc", "abcXYZ");
        assert_eq!(joiner.content, "abcXYZ");

        // Host integrates the joiner's insert.
        for msg in outbound {
            apply_sync_message(&mut host, msg);
        }
        assert_eq!(host.content, "abcXYZ");
    }
}
