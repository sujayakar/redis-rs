// This test requires the `hickory-dns` and `tokio-comp` features to be enabled.
// It validates that the Hickory-based async DNS resolver can successfully
// resolve the "localhost" hostname.

#![cfg(all(feature = "hickory-dns", feature = "tokio-comp"))]

use redis::io::HickoryAsyncDNSResolver;

#[tokio::test]
async fn test_hickory_resolver_localhost() {
    let resolver = HickoryAsyncDNSResolver;
    let result = resolver.resolve("localhost", 6379).await;
    let addresses = result.expect("resolution should succeed");
    let count = addresses.count();
    assert!(count > 0, "expected at least one address, got 0");
}