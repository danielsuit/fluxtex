use cola::{Deletion, Insertion, Replica, ReplicaId};

pub struct DocumentBuffer {
    replica: Replica,
    pub content: String,
}

impl DocumentBuffer {
    pub fn new() -> Self {
        Self::with_replica_id(1)
    }

    pub fn with_replica_id(replica_id: ReplicaId) -> Self {
        Self {
            replica: Replica::new(replica_id, 0),
            content: String::new(),
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
