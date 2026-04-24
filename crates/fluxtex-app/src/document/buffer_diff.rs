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
                // Assuming doc_buffer.delete_local works via char offsets if we use bytes
                // Let's assume byte offsets for now
                doc_buffer.delete_local(offset, len);
                // We do NOT advance offset because the text shifted left
            }
            ChangeTag::Insert => {
                let text = change.value();
                doc_buffer.insert_local(offset, text);
                offset += text.len();
            }
            ChangeTag::Equal => {
                offset += change.value().len();
            }
        }
    }
}
