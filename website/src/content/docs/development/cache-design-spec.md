---
title: Cache Architecture Design Specification
description: Comprehensive redesign specification for cuenv's Bazel-style build cache system
---

# Improved Cache Architecture Design for cuenv

## Executive Summary

This document outlines a comprehensive redesign of the cuenv cache system to create a Bazel-style build cache that works by default, can be disabled per task, uses persistent disk-based storage, and properly integrates with the existing sophisticated infrastructure (ActionCache, ContentAddressedStore, CacheSigner).

## Current Issues Analysis

### 1. Cache is Only In-Memory, Not Persistent

- **Problem**: The current `CacheManager` uses a simple `HashMap<String, CachedTaskResult>` for in-memory storage
- **Impact**: Cache is lost between cuenv invocations, eliminating the primary benefit of build caching
- **Root Cause**: `CacheManager::memory_cache` field is a simple in-memory HashMap

### 2. Task Executor Doesn't Check Task Config Cache Setting

- **Problem**: In `TaskExecutor::execute_single_task_with_cache()`, the method never checks `task_config.cache`
- **Impact**: All tasks are cached regardless of configuration, violating the principle of per-task control
- **Root Cause**: Missing conditional logic to check `task_config.cache.unwrap_or(true)` before caching

### 3. Cache Key Generation Includes ALL Environment Variables

- **Problem**: `CacheManager::generate_cache_key()` includes `std::env::vars().collect()` without filtering
- **Impact**: Cache keys change unnecessarily when unrelated environment variables change, reducing cache effectiveness
- **Root Cause**: No selective environment variable inclusion mechanism

### 4. Task Executor Bypasses ActionCache and ContentAddressedStore

- **Problem**: TaskExecutor uses `CacheManager`'s simple in-memory cache instead of the sophisticated `ActionCache`
- **Impact**: Loses benefits of content-addressed storage, cryptographic signing, and concurrent execution coordination
- **Root Cause**: `CacheManager` has both sophisticated components and a simple legacy interface

### 5. No Default Caching Behavior

- **Problem**: No global cache configuration system with sensible defaults
- **Impact**: Users must explicitly configure caching for each task
- **Root Cause**: Missing global cache configuration structure

## Design Requirements

### 1. Cache Configuration System

- **Global cache configuration** (enabled/disabled, cache directory, size limits)
- **Per-task cache override** (cache: true/false in CUE configuration)
- **Environment variable support** (CUENV_CACHE=off/read/read-write/write)
- **Remote cache configuration** (when available)

### 2. Cache Key Generation

- **Selective environment variable inclusion** (allowlist/denylist)
- **Input file hashing with glob pattern support**
- **Working directory normalization**
- **Task configuration hashing**
- **Command/script content hashing**

### 3. Cache Storage Architecture

- **Primary**: Local disk-based ContentAddressedStore
- **Fallback**: In-memory cache for small/fast operations
- **Remote**: gRPC-based remote cache server (when configured)
- **Persistent across cuenv invocations**

### 4. Cache Execution Flow

- **Check if caching is enabled** (global + per-task)
- **Generate cache key with selective inputs**
- **Check local cache (disk + memory)**
- **If miss, execute task and store results**
- **If remote cache configured, check/fetch from remote**

### 5. Integration Points

- **TaskExecutor must use ActionCache** instead of simple CacheManager
- **CacheManager should delegate to ActionCache** for actual operations
- **Maintain backward compatibility** with existing APIs
- **Preserve all existing security features** (cryptographic signing)

## Architecture Design

This specification provides the foundation for implementing a production-grade caching system that scales to Google-level requirements while maintaining the simplicity and usability that makes cuenv effective for development teams.

For detailed implementation guidance, see the [Cache Implementation Plan](/development/cache-implementation-plan/) and [Cache Architecture Guide](/guides/cache-architecture/).
