# Sync-IO Feature Flag Summary

## Overview
Successfully added a new `sync-io` feature flag to the redis crate to enable explicit control over synchronous IO functionality.

## Changes Made

### 1. Added Feature Flag to Cargo.toml
- Added `sync-io = []` to the `[features]` section in `redis/Cargo.toml`
- Added `sync-io` to the default features list: `default = ["acl", "streams", "geospatial", "script", "keep-alive", "num-bigint", "sync-io"]`

### 2. Updated Public API Exports (lib.rs)
- Added conditional compilation attributes to synchronous connection types and traits:
  - `Commands` and `TypedCommands` traits are now gated behind `#[cfg(feature = "sync-io")]`
  - `Connection`, `ConnectionLike`, `Msg`, `PubSub`, and `transaction` are gated behind `#[cfg(feature = "sync-io")]`
  - Added fallback exports for items that should be available regardless of sync-io flag (like `ConnectionAddr`, `ConnectionInfo`, etc.)

### 3. Updated Client Implementation (client.rs)
- Added `#[cfg(feature = "sync-io")]` to synchronous methods:
  - `get_connection()`
  - `get_connection_with_timeout()`
- Added `#[cfg(feature = "sync-io")]` to the `ConnectionLike` implementation for `Client`
- Updated imports to feature-gate sync-specific types:
  - `Connection`, `ConnectionLike`, `connect` - only imported with sync-io
  - `Duration`, `Value` - only imported with sync-io
- Updated documentation example to show it requires sync-io feature

### 4. Updated Tests
- Added conditional compilation to the `Connection` Send test to only run when `sync-io` is enabled

### 5. Updated Documentation
- Added documentation for the new `sync-io` feature in the "Optional Features" section of lib.rs
- Documented it as "enables synchronous IO support (enabled by default)"

## Feature Behavior

### With `sync-io` enabled (default):
- All synchronous connection functionality is available
- `Commands` and `TypedCommands` traits for synchronous operations
- `Connection` type for synchronous connections
- `PubSub` type for synchronous pubsub
- `transaction` function for synchronous transactions
- Client methods `get_connection()` and `get_connection_with_timeout()`

### With `sync-io` disabled:
- Synchronous connection types and traits are not available
- Only core types like `ConnectionAddr`, `ConnectionInfo`, and `RedisConnectionInfo` remain
- Client can still be created and used for async operations
- No synchronous I/O code is compiled, reducing binary size

## Testing Results
- ✅ Compiles successfully with `sync-io` disabled (shows expected "unused" warnings for sync functionality)
- ✅ Compiles successfully with `sync-io` enabled
- ✅ Default build works correctly (includes `sync-io` by default)
- ✅ Clean separation verified - sync code uses only standard library I/O

## Usage Examples

### Enable sync-io explicitly:
```toml
[dependencies]
redis = { version = "0.31.0", features = ["sync-io"] }
```

### Disable sync-io (async-only):
```toml
[dependencies]
redis = { version = "0.31.0", default-features = false, features = ["tokio-comp", "safe_iterators"] }
```

### Default usage (sync-io included):
```toml
[dependencies]
redis = "0.31.0"
```

### Minimal async-only build:
```toml
[dependencies]
redis = { version = "0.31.0", default-features = false, features = ["tokio-comp", "safe_iterators"] }
```

## Implementation Details

The synchronous APIs are **fully synchronous** and independent of async code:
- `Connection` uses standard blocking I/O (`TcpStream`, `UnixStream`)
- No futures, no async/await, no runtime dependencies
- Clean separation between sync (`ConnectionLike`) and async (`aio::ConnectionLike`) traits
- Synchronous commands return `RedisResult<T>` directly
- Async commands return `RedisFuture<'a, T>`

## Backwards Compatibility
- ✅ Fully backwards compatible
- ✅ Default behavior unchanged (sync-io is enabled by default)
- ✅ Existing code will continue to work without modification

The feature flag provides explicit control over synchronous IO functionality while maintaining full backwards compatibility.