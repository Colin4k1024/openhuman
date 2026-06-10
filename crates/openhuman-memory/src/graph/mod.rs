//! Memory graph — knowledge graph with derived and explicit relations.
//!
//! Two layers:
//!
//! 1. **Derived** (`query.rs`): co-occurrence edges computed on-the-fly from
//!    `mem_tree_entity_index`. Read-only, no extra tables.
//!
//! 2. **Persistent** (`persistent_store.rs`): explicit nodes + typed edges
//!    stored in `mem_graph_nodes` / `mem_graph_edges` tables. Writable.
//!    Supports typed relations (works_at, located_in, follows, etc.).
//!
//! 3. **Discovery** (`discovery.rs`): automatic relation extraction from chunks
//!    — co-occurrence, temporal, and pattern-based rules.

pub mod discovery;
pub mod persistent_store;
pub mod query;
pub mod types;

pub use discovery::{discover_and_persist, discover_co_occurrences, discover_pattern_relations};
pub use persistent_store::{
    delete_node, edges_from, edges_involving, get_node, increment_edge_weight, list_nodes,
    remove_edge, upsert_edge, upsert_node, EvidenceRef, GraphEdgePersistent, GraphNode,
};
pub use query::{co_occurring_entities, neighbors};
pub use types::GraphEdge;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_edge_reexport_is_constructible() {
        let edge = GraphEdge {
            subject: "person:alice".into(),
            object: "topic:phoenix".into(),
            weight: 2,
        };
        assert_eq!(edge.weight, 2);
        assert_eq!(edge.subject, "person:alice");
    }
}
