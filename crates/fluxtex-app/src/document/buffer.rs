use cola::{Deletion, EncodedReplica, Insertion, Replica, ReplicaId};

pub struct DocumentBuffer {
    replica: Replica,
    replica_id: ReplicaId,
    pub content: String,
}

impl DocumentBuffer {
    pub fn with_replica_id(replica_id: ReplicaId) -> Self {
        Self {
            replica: Replica::new(replica_id, 0),
            replica_id,
            content: String::new(),
        }
    }

    /// Capture an encoded view of the CRDT state. Used by the host on
    /// connection-open to seed the joining peer's replica so subsequent
    /// inserts and deletions integrate against a shared baseline.
    pub fn snapshot(&self) -> EncodedReplica {
        self.replica.encode()
    }

    /// Replace local replica + content with the host's snapshot. The local
    /// replica id is preserved so future ops are still uniquely tagged.
    pub fn restore_snapshot(&mut self, encoded: &EncodedReplica, content: String) {
        match Replica::decode(self.replica_id, encoded) {
            Ok(replica) => {
                self.replica = replica;
                self.content = content;
            }
            Err(e) => {
                tracing::warn!(error = ?e, "CRDT snapshot restore failed");
            }
        }
    }

    pub fn insert_local(&mut self, offset: usize, text: &str) -> Option<Insertion> {
        if offset <= self.content.len() {
            self.content.insert_str(offset, text);
            Some(self.replica.inserted(offset, text.len()))
        } else {
            None
        }
    }

    pub fn delete_local(&mut self, offset: usize, len: usize) -> Option<Deletion> {
        if offset + len <= self.content.len() {
            self.content.replace_range(offset..offset + len, "");
            Some(self.replica.deleted(offset..offset + len))
        } else {
            None
        }
    }

    pub fn integrate_remote_insert(&mut self, insertion: Insertion, text: &str) {
        if let Some(offset) = self.replica.integrate_insertion(&insertion) {
            // In a real editor we need to map CRDT offsets to string indices accurately
            self.content.insert_str(offset, text);
        }
    }

    pub fn integrate_remote_delete(&mut self, deletion: Deletion) {
        let ranges = self.replica.integrate_deletion(&deletion);
        for range in ranges.into_iter().rev() {
            self.content.replace_range(range, "");
        }
    }
}
