# Redis-rs Iterator Bug Fix Documentation

## Summary

A critical bug was identified in the redis-rs library's `Iter` and `AsyncIter` implementations. When encountering values that cannot be converted to the target type, the iterator would **completely stop processing**, potentially causing severe data loss without any error indication.

## The Bug

The previous implementation used this pattern:

```rust
fn next(&mut self) -> Option<T> {
    self.iter.next()?.ok()  // BUG: .ok() converts errors to None, ? stops iteration
}
```

When a conversion error occurs:
1. The `next()` method returns an error wrapped in `Some(Err(...))`
2. The `?` operator converts this to `None` 
3. This signals the end of iteration, even if more valid data exists

### Example Impact

```rust
// Data from Redis: [42, "not_a_number", 123, 456]
let mut iter: Iter<i32> = cmd.iter(&mut con)?;
let results: Vec<i32> = iter.collect();

// BEFORE FIX: results = [42]  // Lost 123 and 456!
// AFTER FIX:  results = [Ok(42), Err(...), Ok(123), Ok(456)]
```

## The Fix

### Changes Made

1. **Default Safe Behavior**: The safe iterator behavior is now the default
   - Returns `Iterator<Item = RedisResult<T>>` instead of `Iterator<Item = T>`
   - All values are processed, errors are properly propagated

2. **Feature Flag Inversion**: 
   - Removed: `safe_iterators` feature
   - Added: `unsafe_iterators` feature (for backward compatibility only)

3. **Clear Deprecation Warnings**: Added comprehensive deprecation messages

### Migration Guide

#### For Most Users (Recommended)

Simply remove any `safe_iterators` feature flag from your `Cargo.toml`:

```toml
# Before
redis = { version = "0.31", features = ["safe_iterators"] }

# After  
redis = { version = "0.31" }
```

Update your code to handle potential errors:

```rust
// Before (unsafe)
let iter: Iter<String> = con.scan()?;
for key in iter {
    println!("Key: {}", key);
}

// After (safe)
let iter: Iter<String> = con.scan()?;
for result in iter {
    match result {
        Ok(key) => println!("Key: {}", key),
        Err(e) => eprintln!("Error converting key: {}", e),
    }
}
```

#### For Legacy Code (Not Recommended)

If you absolutely need the old behavior temporarily:

```toml
redis = { version = "0.31", features = ["unsafe_iterators"] }
```

**⚠️ WARNING**: This will restore the buggy behavior and you will see deprecation warnings. Plan to migrate as soon as possible.

## Testing

The fix includes comprehensive tests that verify:

1. **Error Handling**: Mixed valid/invalid values are all processed
2. **Cursor Pagination**: Multi-page results work correctly  
3. **Empty Results**: Empty result sets are handled properly

Run tests with:
```bash
# Test safe behavior (default)
cargo test -p redis test_iterator

# Test unsafe behavior (legacy)
cargo test -p redis --features unsafe_iterators test_iterator
```

## Best Practices

1. **Always handle errors** when iterating over Redis data
2. **Log conversion errors** to detect data format issues
3. **Consider using `filter_map`** to skip errors if appropriate:

```rust
let valid_keys: Vec<String> = iter
    .filter_map(|result| result.ok())
    .collect();
```

## Performance Impact

The fix has minimal performance impact:
- Same memory allocation patterns
- Same number of Redis round-trips
- Only adds error wrapping overhead (negligible)

## Conclusion

This fix ensures data integrity by properly propagating errors instead of silently losing data. While it requires updating code to handle `Result` types, it prevents the severe data loss that could occur with the previous implementation.