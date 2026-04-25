use cola::{Insertion, Deletion};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum SyncMessage {
    Insert {
        insertion: Insertion,
        text: String,
    },
    Delete {
        deletion: Deletion,
    }
}
