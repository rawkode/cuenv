//! Tests for eviction policies

use super::*;

#[test]
fn test_lru_eviction() {
    let policy = LruPolicy::new(1000);

    // Insert entries
    policy.on_insert("a", 100);
    policy.on_insert("b", 200);
    policy.on_insert("c", 300);
    policy.on_insert("d", 500); // Total: 1100, over limit

    // Should evict 'a' (least recently used)
    assert_eq!(policy.next_eviction(), Some("a".to_string()));

    // Access 'b' to make it more recent
    policy.on_access("b", 200);
    policy.on_remove("a", 100); // Total: 1000

    policy.on_insert("e", 200); // Total: 1200, over limit

    // Should evict 'c' (now least recently used)
    assert_eq!(policy.next_eviction(), Some("c".to_string()));
}

#[test]
fn test_lfu_eviction() {
    let policy = LfuPolicy::new(1000);

    // Insert and access entries
    policy.on_insert("a", 300);
    policy.on_insert("b", 300);
    policy.on_insert("c", 300);

    // Access 'a' and 'b' more frequently
    policy.on_access("a", 300);
    policy.on_access("a", 300);
    policy.on_access("b", 300);

    policy.on_insert("d", 300); // Total: 1200, over limit

    // Should evict either 'c' or 'd' (both have frequency 1, least frequently used)
    let evicted = policy.next_eviction();
    assert!(evicted == Some("c".to_string()) || evicted == Some("d".to_string()));
}

#[test]
fn test_arc_adaptation() {
    let policy = ArcPolicy::new(1000);

    // Insert entries
    policy.on_insert("a", 250);
    policy.on_insert("b", 250);
    policy.on_insert("c", 250);
    policy.on_insert("d", 250);

    // Access 'a' and 'b' to move to T2
    policy.on_access("a", 250);
    policy.on_access("b", 250);

    policy.on_insert("e", 250); // Total: 1250, over limit

    // Should evict from T1 (c or d)
    let evicted = policy.next_eviction().unwrap();
    assert!(evicted == "c" || evicted == "d");
}
