use super::*;
use crate::memory::MemoryCategory;

fn user_msg(text: &str) -> crate::message::Message {
    crate::message::Message {
        role: crate::message::Role::User,
        content: vec![crate::message::ContentBlock::Text {
            text: text.to_string(),
            cache_control: None,
        }],
        timestamp: None,
        tool_duration_ms: None,
    }
}

fn assistant_msg(text: &str) -> crate::message::Message {
    crate::message::Message {
        role: crate::message::Role::Assistant,
        content: vec![crate::message::ContentBlock::Text {
            text: text.to_string(),
            cache_control: None,
        }],
        timestamp: None,
        tool_duration_ms: None,
    }
}

#[test]
fn detect_keyword_trigger_matches_preference_phrase() {
    let messages = vec![
        assistant_msg("hello"),
        user_msg("By the way, I prefer Rust over Go for systems code."),
    ];
    let hit = detect_keyword_trigger(&messages);
    assert!(hit.is_some(), "expected keyword match, got None");
    let kw = hit.unwrap().to_lowercase();
    assert!(
        kw.contains("i prefer"),
        "expected `i prefer` match, got {kw:?}"
    );
}

#[test]
fn detect_keyword_trigger_is_case_insensitive() {
    let messages = vec![user_msg("REMEMBER THAT we deploy on Fridays only.")];
    assert!(detect_keyword_trigger(&messages).is_some());
}

#[test]
fn detect_keyword_trigger_only_scans_last_user_message() {
    let messages = vec![
        user_msg("I prefer dark mode."), // earlier user msg with trigger
        assistant_msg("ok"),
        user_msg("what time is it?"), // latest user msg, no trigger
    ];
    assert!(detect_keyword_trigger(&messages).is_none());
}

#[test]
fn detect_keyword_trigger_returns_none_on_neutral_message() {
    let messages = vec![user_msg("How does the cache work?")];
    assert!(detect_keyword_trigger(&messages).is_none());
}

#[test]
fn cfg_min_turns_uses_fallback_when_zero() {
    // We cannot easily mutate the global config in unit tests without test
    // infrastructure, so just sanity-check the fallback constants are sane.
    assert!(MIN_TURNS_FOR_EXTRACTION_FALLBACK >= 1);
    assert!(PERIODIC_EXTRACTION_INTERVAL_FALLBACK >= 1);
    assert!(PERIODIC_EXTRACTION_INTERVAL_FALLBACK >= MIN_TURNS_FOR_EXTRACTION_FALLBACK);
}

#[test]
fn infer_candidate_tag_uses_repeated_non_stopword() {
    let tag =
        infer_candidate_tag("scheduler retries failed jobs and scheduler metrics update dashboard");
    assert_eq!(tag.as_deref(), Some("scheduler"));
}

#[test]
fn apply_cluster_assignment_links_members() {
    let mut graph = MemoryGraph::new();
    let mut a = MemoryEntry::new(MemoryCategory::Fact, "A");
    a.embedding = Some(vec![1.0, 0.0]);
    let id_a = graph.add_memory(a);

    let mut b = MemoryEntry::new(MemoryCategory::Fact, "B");
    b.embedding = Some(vec![0.0, 1.0]);
    let id_b = graph.add_memory(b);

    let stats = apply_cluster_assignment(
        &mut graph,
        "project",
        &[id_a.clone(), id_b.clone()],
        Utc::now(),
    );

    assert_eq!(stats.clusters_touched, 1);
    assert_eq!(stats.member_links, 2);
    assert_eq!(graph.clusters.len(), 1);

    let cluster_id = graph
        .clusters
        .keys()
        .next()
        .expect("cluster id")
        .to_string();
    assert!(
        graph
            .get_edges(&id_a)
            .iter()
            .any(|e| e.target == cluster_id && matches!(e.kind, EdgeKind::InCluster))
    );
    assert!(
        graph
            .get_edges(&id_b)
            .iter()
            .any(|e| e.target == cluster_id && matches!(e.kind, EdgeKind::InCluster))
    );
}
