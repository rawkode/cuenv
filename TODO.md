# cuenv Refactoring TODO List

## Critical Issues - Must Fix

### 1. Error Handling Refactor 🚨
- [ ] Replace ALL instances of `?` operator with explicit match expressions or if-let
- [ ] Create custom error types implementing `std::error::Error`
- [ ] Add `#[must_use]` to all Result-returning functions
- [ ] Implement proper error context with actionable messages

**Files to refactor:**
- `src/main.rs` - 6+ instances
- `src/env_manager.rs` - 8+ instances  
- `src/secrets.rs` - 6+ instances
- `src/cue_parser.rs` - 10+ instances

### 2. Type Safety Improvements 🔒
- [ ] Replace `HashMap<String, String>` with newtype wrappers
- [ ] Replace `serde_json::Value` with strongly typed structs
- [ ] Implement builder pattern for `EnvManager`, `SecretManager`, `CommandResolver`
- [ ] Use phantom types for compile-time guarantees
- [ ] Add const generics where applicable

### 3. Memory & Performance Optimization 🚀
- [ ] Replace string allocations with `Cow<'_, str>` in hot paths
- [ ] Implement zero-copy patterns for parsing
- [ ] Add bounded concurrency to secret resolution
- [ ] Use `Arc<str>` instead of `String` for immutable strings
- [ ] Implement object pooling for frequently allocated types
- [ ] Add criterion benchmarks for critical paths

### 4. Security Hardening 🛡️
- [ ] Implement constant-time comparison for secrets
- [ ] Use `zeroize` crate for secret memory management
- [ ] Replace naive string filtering with proper secret masking
- [ ] Add security audit logging
- [ ] Implement rate limiting for secret resolution
- [ ] Add input validation and sanitization

### 5. Async/Await Refactor 🔄
- [ ] Remove `tokio::runtime::Runtime::new()` from sync contexts
- [ ] Make `EnvManager::run_command` properly async
- [ ] Implement async builder patterns
- [ ] Add proper cancellation support
- [ ] Use `tokio::select!` for concurrent operations

### 6. Testing Overhaul 🧪
- [ ] Remove all mocking - test behavior not implementation
- [ ] Add comprehensive property-based tests with proptest
- [ ] Test error paths and edge cases
- [ ] Add fuzzing tests for parsers
- [ ] Implement integration tests for all commands
- [ ] Add performance regression tests

## High Priority

### 7. Documentation 📚
- [ ] Add comprehensive module-level documentation
- [ ] Document the "why" not just the "what"
- [ ] Add usage examples for all public APIs
- [ ] Create architecture decision records (ADRs)
- [ ] Add inline examples with `doc_test`

### 8. Platform Abstraction 🖥️
- [ ] Replace conditional compilation with trait-based design
- [ ] Extract platform-specific code to separate crates
- [ ] Add WASM support for Cloudflare Workers
- [ ] Implement proper Windows support

### 9. Code Quality 🎯
- [ ] Add `const fn` wherever possible
- [ ] Implement `From`/`TryFrom` instead of custom conversions
- [ ] Use `#[inline]` judiciously for performance
- [ ] Add `#[cold]` to error paths
- [ ] Implement proper Drop traits for cleanup

## Medium Priority

### 10. Feature Enhancements 🌟
- [ ] Add support for encrypted CUE files
- [ ] Implement secret rotation notifications
- [ ] Add environment inheritance chains
- [ ] Support for remote CUE file sources
- [ ] Add dry-run mode for commands

### 11. Observability 📊
- [ ] Add structured logging with `tracing`
- [ ] Implement OpenTelemetry support
- [ ] Add metrics for secret resolution times
- [ ] Create health check endpoints
- [ ] Add debug mode with detailed traces

### 12. CLI Improvements 💻
- [ ] Add shell completion generation
- [ ] Implement interactive mode
- [ ] Add progress indicators for long operations
- [ ] Support for parallel environment loading
- [ ] Add environment diff visualization

## Low Priority

### 13. Ecosystem Integration 🔗
- [ ] Create GitHub Action
- [ ] Add Docker image
- [ ] Create Homebrew formula
- [ ] Add VSCode extension
- [ ] Create language-specific SDKs

### 14. Performance Monitoring 📈
- [ ] Add flamegraph generation
- [ ] Implement memory profiling
- [ ] Add startup time optimization
- [ ] Create performance dashboard
- [ ] Add continuous benchmarking

## Breaking Changes Required

1. **Error Handling**: Moving from `?` to explicit handling will change function signatures
2. **Async API**: Making operations properly async will break current sync API
3. **Type Safety**: Introducing newtypes will break direct string usage
4. **Builder Pattern**: Will change how objects are constructed

## Migration Guide Needed For:
- Error handling changes
- Async API migration
- New type system
- Platform-specific changes

## Estimated Effort:
- Critical Issues: 2-3 weeks
- High Priority: 1-2 weeks  
- Medium Priority: 1 week
- Low Priority: 1 week

Total: ~6 weeks for complete refactor

## Success Metrics:
- Zero uses of `?` operator
- 100% of public APIs have examples
- 90%+ test coverage with property tests
- Zero memory allocations in hot paths
- All secrets properly zeroed after use
- Criterion benchmarks for all critical operations