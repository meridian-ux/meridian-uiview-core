// ConversationModel — the renderer-neutral streaming model. Ingests
// ConversationEvents (dedup by seq, upsert blocks by block_id in arrival order,
// track the transient status + last error) and exposes an ordered block list.
//
// The Rust twin of the TypeScript model in @savvifi/meridian-chat (src/model.ts).
// Both tiers ingest the SAME `meridian.ui.v1.ConversationEvent` stream, so the
// upsert semantics have to match: a `tool` block that flips running→ok must
// replace in place — not append a second row — in the terminal exactly as it does
// in the browser.

use std::collections::{HashMap, HashSet};

use crate::proto::{conversation_event, status, Block, ConversationEvent, Status};

/// Whether a status is a live indicator (vs. idle/cleared).
pub fn is_active_status(state: i32) -> bool {
    matches!(
        status::State::try_from(state),
        Ok(status::State::Thinking) | Ok(status::State::Working)
    )
}

/// The streaming transcript state for one conversation.
#[derive(Debug, Default, Clone)]
pub struct ConversationModel {
    order: Vec<String>,
    by_id: HashMap<String, Block>,
    seen: HashSet<u64>,
    status: Option<Status>,
    error: Option<String>,
}

impl ConversationModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply one event. Returns true if it changed state, false if it was a
    /// duplicate (already-seen seq) and was ignored.
    ///
    /// `seq == 0` means UNSEQUENCED, not "sequence zero": proto3-JSON omits a
    /// zero-valued scalar, so a producer that doesn't sequence its stream is
    /// indistinguishable from one sending 0. The TS model skips dedup when `seq`
    /// is absent; doing the same here keeps an unsequenced stream from collapsing
    /// to a single block.
    pub fn ingest(&mut self, event: &ConversationEvent) -> bool {
        if event.seq != 0 && !self.seen.insert(event.seq) {
            return false;
        }
        match &event.event {
            Some(conversation_event::Event::Block(block)) => {
                if self
                    .by_id
                    .insert(block.block_id.clone(), block.clone())
                    .is_none()
                {
                    self.order.push(block.block_id.clone());
                }
            }
            Some(conversation_event::Event::Status(status)) => {
                self.status = is_active_status(status.state).then(|| status.clone());
            }
            Some(conversation_event::Event::Done(_)) => {
                self.status = None;
            }
            Some(conversation_event::Event::Error(error)) => {
                self.error = Some(error.message.clone());
                self.status = None;
            }
            None => {}
        }
        true
    }

    /// Blocks in arrival order (each the latest version for its block_id).
    pub fn blocks(&self) -> Vec<&Block> {
        self.order
            .iter()
            .filter_map(|id| self.by_id.get(id))
            .collect()
    }

    /// The current live status, or None when idle/cleared.
    pub fn status(&self) -> Option<&Status> {
        self.status.as_ref()
    }

    /// The last turn error, or None.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn reset(&mut self) {
        self.order.clear();
        self.by_id.clear();
        self.seen.clear();
        self.status = None;
        self.error = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{block, TurnDone, TurnError};

    fn markdown_block(id: &str, text: &str) -> Block {
        Block {
            block_id: id.into(),
            role: "assistant".into(),
            kind: Some(block::Kind::Markdown(block::Markdown { text: text.into() })),
        }
    }

    fn event(seq: u64, ev: conversation_event::Event) -> ConversationEvent {
        ConversationEvent {
            seq,
            event: Some(ev),
        }
    }

    fn block_event(seq: u64, block: Block) -> ConversationEvent {
        event(seq, conversation_event::Event::Block(block))
    }

    #[test]
    fn blocks_arrive_in_order() {
        let mut model = ConversationModel::new();
        model.ingest(&block_event(1, markdown_block("a", "first")));
        model.ingest(&block_event(2, markdown_block("b", "second")));
        let ids: Vec<&str> = model.blocks().iter().map(|b| b.block_id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn same_block_id_upserts_in_place() {
        let mut model = ConversationModel::new();
        model.ingest(&block_event(1, markdown_block("a", "partial")));
        model.ingest(&block_event(2, markdown_block("b", "other")));
        model.ingest(&block_event(3, markdown_block("a", "complete")));
        // Replaced, not appended — and it keeps its original position.
        let ids: Vec<&str> = model.blocks().iter().map(|b| b.block_id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b"]);
        match &model.blocks()[0].kind {
            Some(block::Kind::Markdown(m)) => assert_eq!(m.text, "complete"),
            other => panic!("expected markdown, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_seq_is_ignored() {
        let mut model = ConversationModel::new();
        assert!(model.ingest(&block_event(1, markdown_block("a", "first"))));
        assert!(!model.ingest(&block_event(1, markdown_block("b", "replay"))));
        assert_eq!(model.blocks().len(), 1);
    }

    #[test]
    fn unsequenced_events_never_dedup() {
        // seq=0 is "unset" in proto3, not a real sequence number.
        let mut model = ConversationModel::new();
        assert!(model.ingest(&block_event(0, markdown_block("a", "first"))));
        assert!(model.ingest(&block_event(0, markdown_block("b", "second"))));
        assert_eq!(model.blocks().len(), 2);
    }

    #[test]
    fn active_status_is_held_and_idle_clears_it() {
        let mut model = ConversationModel::new();
        model.ingest(&event(
            1,
            conversation_event::Event::Status(Status {
                state: status::State::Thinking as i32,
                detail: "Calling forge·list_repos".into(),
            }),
        ));
        assert_eq!(model.status().map(|s| s.detail.as_str()), Some("Calling forge·list_repos"));
        model.ingest(&event(
            2,
            conversation_event::Event::Status(Status {
                state: status::State::Idle as i32,
                detail: String::new(),
            }),
        ));
        assert!(model.status().is_none());
    }

    #[test]
    fn done_clears_status_and_error_is_captured() {
        let mut model = ConversationModel::new();
        model.ingest(&event(
            1,
            conversation_event::Event::Status(Status {
                state: status::State::Working as i32,
                detail: "…".into(),
            }),
        ));
        model.ingest(&event(
            2,
            conversation_event::Event::Done(TurnDone {
                stop_reason: "end_turn".into(),
            }),
        ));
        assert!(model.status().is_none());

        model.ingest(&event(
            3,
            conversation_event::Event::Error(TurnError {
                message: "boom".into(),
            }),
        ));
        assert_eq!(model.error(), Some("boom"));
    }

    #[test]
    fn reset_clears_everything_including_seen_seqs() {
        let mut model = ConversationModel::new();
        model.ingest(&block_event(1, markdown_block("a", "first")));
        model.reset();
        assert!(model.blocks().is_empty());
        // The seq is forgettable too — a fresh stream restarts at 1.
        assert!(model.ingest(&block_event(1, markdown_block("a", "again"))));
        assert_eq!(model.blocks().len(), 1);
    }
}
