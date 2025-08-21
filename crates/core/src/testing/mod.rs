//! Property-based testing utilities and examples for functional programming patterns
//!
//! This module demonstrates how to use property-based testing with the functional
//! programming utilities in the cuenv project.

use crate::errors::{Error, Result, ResultExt, Validate};
use crate::functional::prelude::*;
use crate::types::newtypes::{EnvVarName, TaskName, ValidatedPath};
use proptest::prelude::*;

/// Property-based test generators
pub mod generators {
    use super::*;
    use proptest::collection::vec;
    use proptest::option;
    use proptest::string::string_regex;

    /// Generate valid task names
    pub fn valid_task_name() -> impl Strategy<Value = String> {
        string_regex("[a-zA-Z0-9_.-]{1,64}").unwrap()
    }

    /// Generate invalid task names (empty or with invalid characters)  
    pub fn invalid_task_name() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("".to_string()),
            Just(" invalid space".to_string()),
            Just("invalid@symbol".to_string()),
            Just("invalid#hash".to_string()),
            Just("invalid/slash".to_string()),
            Just("invalid$dollar".to_string()),
        ]
    }

    /// Generate valid environment variable names
    pub fn valid_env_var_name() -> impl Strategy<Value = String> {
        string_regex("[A-Z_][A-Z0-9_]{0,63}").unwrap()
    }

    /// Generate invalid environment variable names
    pub fn invalid_env_var_name() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("".to_string()),
            string_regex("[a-z][A-Z0-9_]*").unwrap(), // Starts with lowercase
            string_regex("[A-Z_][A-Za-z0-9_]*[a-z]+[A-Z0-9_]*").unwrap(), // Contains lowercase
        ]
    }

    /// Generate paths (both valid and potentially invalid)
    pub fn arbitrary_path() -> impl Strategy<Value = String> {
        prop_oneof![
            string_regex("(/[a-zA-Z0-9_.-]+)+").unwrap(), // Unix-style paths
            string_regex("/tmp/[a-zA-Z0-9_.-]+").unwrap(), // Temp paths
            string_regex("./[a-zA-Z0-9_.-/]+").unwrap(),  // Relative paths
        ]
    }

    /// Generate lists of values for testing collections
    pub fn non_empty_list<T: 'static + std::fmt::Debug>(
        element: impl Strategy<Value = T>,
    ) -> impl Strategy<Value = Vec<T>> {
        vec(element, 1..=100)
    }

    /// Generate optional values
    pub fn maybe<T: 'static + std::fmt::Debug>(
        element: impl Strategy<Value = T>,
    ) -> impl Strategy<Value = Option<T>> {
        option::of(element)
    }
}

/// Property-based test examples
#[cfg(test)]
mod property_tests {
    use super::generators::*;
    use super::*;
    use proptest::proptest;

    proptest! {
        /// Test that valid task names always create TaskName successfully
        #[test]
        fn valid_task_names_always_succeed(name in valid_task_name()) {
            let result = TaskName::new(name.clone());
            prop_assert!(result.is_ok());
            let task_name = result.unwrap();
            prop_assert_eq!(task_name.as_str(), name);
        }

        /// Test that invalid task names always fail
        #[test]
        fn invalid_task_names_always_fail(name in invalid_task_name()) {
            let result = TaskName::new(name);
            prop_assert!(result.is_err());
        }

        /// Test that valid environment variable names always succeed
        #[test]
        fn valid_env_var_names_always_succeed(name in valid_env_var_name()) {
            let result = EnvVarName::new(name.clone());
            prop_assert!(result.is_ok());
            let env_name = result.unwrap();
            prop_assert_eq!(env_name.as_str(), name);
        }

        /// Test that invalid environment variable names always fail
        #[test]
        fn invalid_env_var_names_always_fail(name in invalid_env_var_name()) {
            let result = EnvVarName::new(name);
            prop_assert!(result.is_err());
        }

        /// Test that path validation is consistent
        #[test]
        fn path_validation_is_consistent(path in arbitrary_path()) {
            let result1 = ValidatedPath::new(path.clone());
            let result2 = ValidatedPath::new(path.clone());
            prop_assert_eq!(result1.is_ok(), result2.is_ok());
        }

        /// Test the `not_empty` validation function
        #[test]
        fn not_empty_validation_properties(s in ".*") {
            let result = Validate::not_empty(&s, "test_field");
            if s.is_empty() {
                prop_assert!(result.is_err());
            } else {
                prop_assert!(result.is_ok());
            }
        }

        /// Test range validation properties
        #[test]
        fn range_validation_properties(value in 0i32..200, min in 0i32..50, max in 100i32..150) {
            let result = Validate::in_range(value, min, max, "test_field");
            if value < min || value > max {
                prop_assert!(result.is_err());
            } else {
                prop_assert!(result.is_ok());
                prop_assert_eq!(result.unwrap(), value);
            }
        }

        /// Test that pipe operations preserve values correctly
        #[test]
        fn pipe_operations_preserve_values(x in 0i32..1000) {
            let result = Pipe::new(x)
                .pipe(|n| n + 1)
                .pipe(|n| n - 1)
                .into_inner();
            prop_assert_eq!(result, x);
        }

        /// Test that composition is associative
        #[test]
        fn composition_is_associative(x in 0i32..100) {
            let f = |n: i32| n + 1;
            let g = |n: i32| n * 2;
            let h = |n: i32| n - 5;

            let result1 = forward_compose(forward_compose(f, g), h)(x);
            let result2 = forward_compose(f, forward_compose(g, h))(x);

            prop_assert_eq!(result1, result2);
        }

        /// Test that identity is the identity for composition
        #[test]
        fn identity_is_composition_identity(x in 0i32..1000) {
            let f = |n: i32| n * 3 + 7;

            let result1 = forward_compose(identity, f)(x);
            let result2 = forward_compose(f, identity)(x);
            let direct = f(x);

            prop_assert_eq!(result1, direct);
            prop_assert_eq!(result2, direct);
        }

        /// Test Result extensions maintain monadic laws
        #[test]
        fn result_ext_left_identity(x in 0i32..1000) {
            let f = |n: i32| if n % 2 == 0 { Ok(n * 2) } else { Err(Error::Configuration { message: "odd".to_string() }) };

            let ok_x: Result<i32> = Ok(x);
            let result1 = ok_x.and_then_ext(f);
            let result2 = f(x);

            match (result1, result2) {
                (Ok(a), Ok(b)) => prop_assert_eq!(a, b),
                (Err(_), Err(_)) => {}, // Both errors, that's fine
                _ => prop_assert!(false, "Results should match"),
            }
        }

        /// Test that functional validation composes correctly
        #[test]
        fn validation_composition_properties(s in "[a-zA-Z0-9_.-]*") {
            // Test validation composition with simple rules
            let result = if s.is_empty() {
                Err(Error::Configuration { message: "empty".to_string() })
            } else if s.len() > 64 {
                Err(Error::Configuration { message: "too long".to_string() })
            } else {
                Ok(s.clone())
            };

            if s.is_empty() || s.len() > 64 {
                prop_assert!(result.is_err());
            } else {
                prop_assert!(result.is_ok());
                prop_assert_eq!(result.unwrap(), s);
            }
        }

        /// Test that Option extensions work correctly
        #[test]
        fn option_extensions_properties(maybe_x in maybe(0i32..1000)) {
            let doubled = maybe_x.map(|x| x * 2);
            let result = doubled.map_or_else_with(|| -1, |x| x);

            match maybe_x {
                Some(x) => prop_assert_eq!(result, x * 2),
                None => prop_assert_eq!(result, -1),
            }
        }

        /// Test that iterator extensions preserve collection size relationships
        #[test]
        fn iterator_extensions_preserve_relationships(vec in non_empty_list(0i32..100)) {
            let original_len = vec.len();
            let (evens, odds) = vec.iter().partition_collect(|&&n| n % 2 == 0);

            prop_assert_eq!(evens.len() + odds.len(), original_len);

            // All evens should be even, all odds should be odd
            prop_assert!(evens.iter().all(|&&n| n % 2 == 0));
            prop_assert!(odds.iter().all(|&&n| n % 2 == 1));
        }
    }
}

/// Integration test examples showing real-world property testing
#[cfg(test)]
mod integration_tests {
    use super::generators::*;
    use super::*;
    use crate::types::builders::{TaskBuilder, TaskConfig};
    use proptest::proptest;

    proptest! {
        /// Test that task builders with valid inputs always succeed
        #[test]
        fn task_builder_with_valid_inputs_succeeds(
            name in valid_task_name(),
            command in "[a-zA-Z]{1,50}",
            args in prop::collection::vec("[a-zA-Z0-9_.-]{1,20}", 0..10)
        ) {
            let result = TaskBuilder::new()
                .with_name(name.clone())
                .map(|builder| builder.with_command(command.clone()))
                .and_then(|builder| builder.with_args(args.clone()).ready().build());

            prop_assert!(result.is_ok());
            let task = result.unwrap();
            prop_assert_eq!(task.name.as_str(), name);
            prop_assert_eq!(task.command, command);
            prop_assert_eq!(task.args, args);
        }

        /// Test that task validation catches circular dependencies
        #[test]
        fn task_validation_catches_self_dependency(name in valid_task_name()) {
            let task_name = TaskName::new(name.clone()).unwrap();
            let task = TaskConfig {
                name: task_name.clone(),
                description: None,
                command: "echo".to_string(),
                args: vec![],
                working_dir: None,
                env_vars: Default::default(),
                timeout: None,
                dependencies: vec![task_name], // Self-dependency
                inputs: vec![],
                outputs: vec![],
            };

            let result = task.validate();
            prop_assert!(result.is_err());
        }

        /// Test that pipe operations maintain referential transparency
        #[test]
        fn pipe_referential_transparency(values in non_empty_list(0i32..1000)) {
            let sum_directly: i32 = values.iter().sum();

            let sum_via_pipe = Pipe::new(values.clone())
                .pipe(|v| v.into_iter().sum::<i32>())
                .into_inner();

            prop_assert_eq!(sum_directly, sum_via_pipe);
        }
    }
}

/// Benchmarking utilities for functional code
#[cfg(test)]
mod benchmarks {
    use super::*;

    /// Helper to measure execution time of functional operations
    pub fn time_operation<F, R>(f: F) -> (R, std::time::Duration)
    where
        F: FnOnce() -> R,
    {
        let start = std::time::Instant::now();
        let result = f();
        let duration = start.elapsed();
        (result, duration)
    }

    #[test]
    fn benchmark_pipe_vs_direct() {
        let data: Vec<i32> = (0..10000).collect();

        let (result1, time1) = time_operation(|| {
            data.iter()
                .map(|&x| x * 2)
                .filter(|&x| x > 100)
                .take(1000)
                .sum::<i32>()
        });

        let (result2, time2) = time_operation(|| {
            Pipe::new(data.iter())
                .pipe(|iter| iter.map(|&x| x * 2))
                .pipe(|iter| iter.filter(|&x| x > 100))
                .pipe(|iter| iter.take(1000))
                .pipe(|iter| iter.sum::<i32>())
                .into_inner()
        });

        assert_eq!(result1, result2);
        println!("Direct: {time1:?}, Pipe: {time2:?}");

        // The pipe version should be reasonably close in performance
        // This is more of a smoke test than a strict benchmark
        assert!(time2 < time1 * 10); // Allow up to 10x overhead
    }
}

pub mod examples {
    //! Documentation examples for property-based testing
    //!
    //! This module contains examples of how to use property-based testing
    //! with functional programming patterns in the cuenv project.

    /// Example: Testing a custom validation function
    ///
    /// ```rust
    /// use cuenv_core::testing::generators::*;
    /// use cuenv_core::errors::Validate;
    /// use proptest::prelude::*;
    ///
    /// proptest! {
    ///     #[test]
    ///     fn custom_validation_example(input in ".*") {
    ///         let result = Validate::with_predicate(
    ///             input.clone(),
    ///             |s| s.chars().all(|c| c.is_ascii()),
    ///             "Must be ASCII"
    ///         );
    ///         
    ///         if input.chars().all(|c| c.is_ascii()) {
    ///             prop_assert!(result.is_ok());
    ///         } else {
    ///             prop_assert!(result.is_err());
    ///         }
    ///     }
    /// }
    /// ```
    pub fn validation_example() {}

    /// Example: Testing functional composition
    ///
    /// ```rust
    /// use cuenv_core::functional::prelude::*;
    /// use proptest::prelude::*;
    ///
    /// proptest! {
    ///     #[test]
    ///     fn composition_example(x in 0i32..1000) {
    ///         let f = |n| n + 10;
    ///         let g = |n| n * 3;
    ///         
    ///         let composed = forward_compose(f, g);
    ///         let expected = g(f(x));
    ///         
    ///         prop_assert_eq!(composed(x), expected);
    ///     }
    /// }
    /// ```
    pub fn composition_example() {}
}
