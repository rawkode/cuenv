#[cfg(test)]
mod format_tests {
    use crate::commands::task::graph::formats::{json, tree};
    use crate::commands::task::graph::{CharSet, GraphFormat};

    #[test]
    fn test_graph_format_from_option() {
        assert_eq!(GraphFormat::from_option(None), GraphFormat::Tree);
        assert_eq!(
            GraphFormat::from_option(Some("tree".to_string())),
            GraphFormat::Tree
        );
        assert_eq!(
            GraphFormat::from_option(Some("json".to_string())),
            GraphFormat::Json
        );
        assert_eq!(
            GraphFormat::from_option(Some("invalid".to_string())),
            GraphFormat::Tree
        );
    }

    #[test]
    fn test_charset_from_str() {
        assert_eq!(CharSet::from_str("unicode"), CharSet::Unicode);
        assert_eq!(CharSet::from_str("ascii"), CharSet::Ascii);
        assert_eq!(CharSet::from_str("invalid"), CharSet::Unicode); // defaults to unicode
    }

    #[test]
    fn test_tree_formatter_creation() {
        let unicode_formatter = tree::TreeFormatter::new(CharSet::Unicode);
        let ascii_formatter = tree::TreeFormatter::new(CharSet::Ascii);

        // Test that formatters are created successfully without panicking
        let _ = unicode_formatter;
        let _ = ascii_formatter;
    }

    #[test]
    fn test_json_formatter_creation() {
        let formatter = json::JsonFormatter::new();
        // Test that formatter is created successfully without panicking
        let _ = formatter;
    }

    // Integration test for format selection
    #[test]
    fn test_format_selection_logic() {
        let test_cases = vec![
            (None, GraphFormat::Tree),
            (Some("".to_string()), GraphFormat::Tree),
            (Some("tree".to_string()), GraphFormat::Tree),
            (Some("json".to_string()), GraphFormat::Json),
            (Some("TREE".to_string()), GraphFormat::Tree), // Case insensitive not implemented yet
        ];

        for (input, expected) in test_cases {
            let result = GraphFormat::from_option(input.clone());
            assert_eq!(result, expected, "Failed for input: {input:?}");
        }
    }

    #[test]
    fn test_charset_selection_logic() {
        let test_cases = vec![
            ("unicode", CharSet::Unicode),
            ("ascii", CharSet::Ascii),
            ("UNICODE", CharSet::Unicode), // Case sensitive for now
            ("", CharSet::Unicode),        // Default
            ("invalid", CharSet::Unicode), // Default
        ];

        for (input, expected) in test_cases {
            let result = CharSet::from_str(input);
            assert_eq!(result, expected, "Failed for input: {input}");
        }
    }

    #[test]
    fn test_graph_format_debug() {
        // Test that the debug formatting works for GraphFormat enum
        let formats = vec![GraphFormat::Tree, GraphFormat::Json];

        for format in formats {
            let debug_string = format!("{format:?}");
            assert!(!debug_string.is_empty(), "Debug string should not be empty");
        }
    }

    #[test]
    fn test_charset_debug() {
        // Test that the debug formatting works for CharSet enum
        let charsets = vec![CharSet::Unicode, CharSet::Ascii];

        for charset in charsets {
            let debug_string = format!("{charset:?}");
            assert!(!debug_string.is_empty(), "Debug string should not be empty");
        }
    }
}
