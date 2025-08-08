# TODO: Next Steps for Cross-Package Task Dependencies

## 🔴 Critical - Must Fix Before Merge

### 1. Fix Remaining Test Failures

**Status**: Two tests still failing despite fixes

- [ ] `test_run_task_from_subdirectory` - Cross-package dependency resolution not working from subdirectories
- [ ] `test_task_with_staged_inputs` - Environment variables not being passed to scripts correctly

**Action Required**:

- Debug why environment variables aren't reaching the script execution
- Verify the staging directory is created and symlinks are correct
- Ensure the cross-package discovery is working when running from subdirectories

### 2. Compilation Warnings

- [ ] Fix `unused_mut` warning in `tests/dependency_staging_test.rs:8`
- [ ] Remove unused imports in test files

## 🟡 Important - Should Complete

### 3. Test Coverage Gaps

- [ ] Add test for invalid output reference syntax (e.g., `task#` without output)
- [ ] Add test for deeply nested package hierarchies with outputs
- [ ] Add test for error messages when referenced output doesn't exist
- [ ] Add test for circular dependencies with outputs
- [ ] Add test for multiple outputs from same task

### 4. Documentation Updates

- [ ] Update main README.md with cross-package task examples
- [ ] Document the `#` separator syntax for outputs
- [ ] Add monorepo examples to documentation
- [ ] Document the difference between `dependencies` and `inputs`
- [ ] Update CHANGELOG.md with new features

### 5. Example Updates

- [ ] Update `examples/monorepo/` to demonstrate working cross-package tasks
- [ ] Add example showing output staging and environment variables
- [ ] Create a tutorial for monorepo setup

## 🟢 Nice to Have - Future Improvements

### 6. Performance Optimizations

- [ ] Cache discovered packages to avoid repeated filesystem walks
- [ ] Parallelize cross-package task execution where possible
- [ ] Optimize staging for large files (consider hard links vs symlinks)

### 7. Enhanced Error Messages

- [ ] Better error when task output doesn't exist
- [ ] Clearer message when cross-package reference can't be resolved
- [ ] Suggest corrections for typos in package/task names

### 8. Additional Features

- [ ] Support for glob patterns in outputs (e.g., `*.txt`)
- [ ] Support for output directories (not just files)
- [ ] Add `--dry-run` flag to show execution plan
- [ ] Add progress indicators for long-running cross-package tasks

## 📋 Testing Checklist

Before considering this feature complete:

- [ ] All integration tests pass
- [ ] Manual testing of example monorepo works
- [ ] Cross-platform testing (Linux, macOS, Windows)
- [ ] Performance testing with large monorepos
- [ ] Documentation review and approval

## 🐛 Known Issues

1. **Environment Variable Propagation**: Staged inputs may not be accessible in scripts due to environment variable issues
2. **Subdirectory Execution**: Running tasks from subdirectories with cross-package deps may fail
3. **Windows Compatibility**: Symlink creation may require admin privileges on Windows

## 📝 Notes

- The `#` separator syntax is now implemented but needs thorough testing
- Discovery module is working but could be optimized
- Consider whether to support backward compatibility with `:` separator

## 🚀 Release Criteria

- [ ] All critical issues resolved
- [ ] Test coverage > 80% for new code
- [ ] Documentation complete
- [ ] Examples working
- [ ] CI/CD pipeline green
- [ ] Performance benchmarks acceptable
