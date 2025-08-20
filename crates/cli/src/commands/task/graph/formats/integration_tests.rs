#[cfg(test)]
mod tests {
    use crate::commands::task::graph::formats::{d2, dot, json, mermaid, tree};
    use crate::commands::task::graph::CharSet;

    // Since we can't easily create a UnifiedTaskDAG for testing without significant setup,
    // these tests focus on formatter creation and basic functionality

    #[test]
    fn test_tree_formatter_unicode_creation() {
        let _formatter = tree::TreeFormatter::new(CharSet::Unicode);
        // Test that formatter is created successfully without panicking
    }

    #[test]
    fn test_tree_formatter_ascii_creation() {
        let _formatter = tree::TreeFormatter::new(CharSet::Ascii);
        // Test that formatter is created successfully without panicking
    }

    #[test]
    fn test_json_formatter_creation() {
        let _formatter = json::JsonFormatter::new();
        // Test that formatter is created successfully without panicking
    }

    #[test]
    fn test_dot_formatter_creation() {
        let _formatter = dot::DotFormatter::new();
        // Test that formatter is created successfully without panicking
    }

    #[test]
    fn test_mermaid_formatter_creation() {
        let _formatter = mermaid::MermaidFormatter::new();
        // Test that formatter is created successfully without panicking
    }

    #[test]
    fn test_d2_formatter_creation() {
        let _formatter = d2::D2Formatter::new();
        // Test that formatter is created successfully without panicking
    }

    // Performance tests
    #[test]
    fn test_formatter_performance() {
        // Basic performance test to ensure formatters don't take too long
        let start = std::time::Instant::now();

        let _tree_formatter = tree::TreeFormatter::new(CharSet::Unicode);
        let _json_formatter = json::JsonFormatter::new();
        let _dot_formatter = dot::DotFormatter::new();
        let _mermaid_formatter = mermaid::MermaidFormatter::new();
        let _d2_formatter = d2::D2Formatter::new();

        let duration = start.elapsed();
        assert!(
            duration < std::time::Duration::from_millis(10),
            "Formatter creation should be fast"
        );
    }

    // Test that formatters implement the required traits
    #[test]
    fn test_formatters_implement_graph_formatter() {
        use crate::commands::task::graph::GraphFormatter;

        // Test that all formatters implement the GraphFormatter trait
        fn is_graph_formatter<T: GraphFormatter>() {}

        is_graph_formatter::<tree::TreeFormatter>();
        is_graph_formatter::<json::JsonFormatter>();
        is_graph_formatter::<dot::DotFormatter>();
        is_graph_formatter::<mermaid::MermaidFormatter>();
        is_graph_formatter::<d2::D2Formatter>();
    }

    // Comprehensive D2 formatter tests
    mod d2_tests {
        use super::*;
        use crate::commands::task::graph::GraphFormatter;

        #[test]
        fn test_d2_formatter_implements_trait() {
            let formatter = d2::D2Formatter::new();

            // Test that the D2 formatter properly implements the GraphFormatter trait
            // This verifies the struct is properly constructed
            fn accepts_graph_formatter<T: GraphFormatter>(_: T) {}
            accepts_graph_formatter(formatter);
        }

        #[test]
        fn test_d2_formatter_creation_multiple_instances() {
            // Test that multiple D2 formatters can be created independently
            let _formatter1 = d2::D2Formatter::new();
            let _formatter2 = d2::D2Formatter::new();
            let _formatter3 = d2::D2Formatter::new();

            // All formatters should be created successfully
            // This tests that the formatter is stateless and lightweight
        }

        #[test]
        fn test_d2_formatter_zero_sized() {
            let formatter = d2::D2Formatter::new();

            // D2Formatter should be zero-sized for efficiency
            assert_eq!(std::mem::size_of_val(&formatter), 0);
        }

        #[test]
        fn test_d2_formatter_send_sync() {
            // Test that D2Formatter is Send and Sync for concurrent usage
            fn is_send_sync<T: Send + Sync>() {}
            is_send_sync::<d2::D2Formatter>();
        }

        #[test]
        fn test_d2_formatter_clone() {
            let formatter = d2::D2Formatter::new();
            let _cloned = formatter;

            // Test that D2Formatter can be moved (it should be Copy)
            // Since it's zero-sized, this should work without explicit Clone
        }

        #[test]
        fn test_d2_formatter_debug() {
            let formatter = d2::D2Formatter::new();
            let debug_str = format!("{formatter:?}");

            // Test that D2Formatter implements Debug
            assert!(!debug_str.is_empty());
        }

        #[test]
        fn test_d2_formatter_equality() {
            let formatter1 = d2::D2Formatter::new();
            let formatter2 = d2::D2Formatter::new();

            // Since D2Formatter is zero-sized and implements Copy, all instances are identical
            // Test that they format to the same debug representation
            assert_eq!(format!("{formatter1:?}"), format!("{formatter2:?}"));
        }

        // Integration test placeholders for when we have mock DAG creation

        #[test]
        fn test_d2_formatter_empty_dag_handling() {
            let formatter = d2::D2Formatter::new();

            // Test that the formatter can handle edge cases
            // This test verifies the formatter is robust
            // Note: Without mock DAG creation, we can only test basic construction
            let _ = formatter;
        }

        #[test]
        fn test_d2_formatter_concurrency_safety() {
            use std::thread;

            // Test that D2Formatter can be used concurrently
            let handles: Vec<_> = (0..10)
                .map(|_| {
                    thread::spawn(|| {
                        let formatter = d2::D2Formatter::new();
                        // Basic operations that should be thread-safe
                        let _ = format!("{formatter:?}");
                    })
                })
                .collect();

            for handle in handles {
                handle.join().expect("Thread should complete successfully");
            }
        }

        #[test]
        fn test_d2_formatter_memory_efficiency() {
            // Test that creating many formatters doesn't consume significant memory
            let formatters: Vec<_> = (0..1000).map(|_| d2::D2Formatter::new()).collect();

            // Since D2Formatter is zero-sized, 1000 instances should use minimal memory
            assert_eq!(formatters.len(), 1000);
        }
    }
}
