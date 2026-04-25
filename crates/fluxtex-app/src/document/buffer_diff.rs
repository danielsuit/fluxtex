use crate::document::buffer::DocumentBuffer;
use similar::{ChangeTag, TextDiff};

pub fn apply_text_diff(doc_buffer: &mut DocumentBuffer, old_text: &str, new_text: &str) {
    let diff = TextDiff::from_chars(old_text, new_text);
    
    // Apply changes in reverse order or one by one.
    // Character diff gives us exact offsets.
    let mut offset = 0;
    
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => {
                let len = change.value().len();
                if let Some(deletion) = doc_buffer.delete_local(offset, len) {
                    // TODO: Serialize and broadcast deletion to network
                    // let _msg = SyncMessage::Delete { deletion: serialize(&deletion) };
                }
            }
            ChangeTag::Insert => {
                let text = change.value();
                if let Some(insertion) = doc_buffer.insert_local(offset, text) {
                    // TODO: Serialize and broadcast insertion to network
                    // let _msg = SyncMessage::Insert { insertion: serialize(&insertion), text: text.to_string() };
                }
                offset += text.len();
            }
            ChangeTag::Equal => {
                offset += change.value().len();
            }
        }
    }
}
