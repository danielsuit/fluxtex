use crate::document::buffer::DocumentBuffer;
use crate::document::network::SyncMessage;
use similar::{ChangeTag, TextDiff};

pub fn apply_text_diff(
    doc_buffer: &mut DocumentBuffer,
    old_text: &str,
    new_text: &str,
) -> Vec<SyncMessage> {
    let diff = TextDiff::from_chars(old_text, new_text);
    let mut messages = Vec::new();

    // Apply changes in reverse order or one by one.
    // Character diff gives us exact offsets.
    let mut offset = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => {
                let len = change.value().len();
                if let Some(deletion) = doc_buffer.delete_local(offset, len) {
                    messages.push(SyncMessage::Delete { deletion });
                }
            }
            ChangeTag::Insert => {
                let text = change.value();
                if let Some(insertion) = doc_buffer.insert_local(offset, text) {
                    messages.push(SyncMessage::Insert {
                        insertion,
                        text: text.to_string(),
                    });
                }
                offset += text.len();
            }
            ChangeTag::Equal => {
                offset += change.value().len();
            }
        }
    }

    messages
}
