// New resolver based on hickory-resolver when the `hickory-dns` feature is enabled.
// This module is only compiled when the `hickory-dns` and `aio` features are both active.

#![cfg(all(feature = "hickory-dns", feature = "aio"))]

use crate::io::AsyncDNSResolver;
use crate::types::{ErrorKind, RedisError, RedisFuture};
use futures_util::future::FutureExt;
use std::net::SocketAddr;

/// An [`AsyncDNSResolver`] implementation backed by the `hickory-resolver` crate.
///
/// The resolver uses the asynchronous Tokio runtime that `hickory` provides
/// (`TokioAsyncResolver`). Each `resolve` call constructs the resolver from the
/// system configuration and performs an `lookup_ip` query. The overhead for
/// constructing the resolver is negligible for most applications, but if this
/// becomes a bottleneck callers can wrap this resolver in their own caching
/// layer and pass the cached instance to `AsyncConnectionConfig::set_dns_resolver`.
#[derive(Clone, Debug, Default)]
pub struct HickoryAsyncDNSResolver;

impl AsyncDNSResolver for HickoryAsyncDNSResolver {
    fn resolve<'a, 'b: 'a>(
        &'a self,
        host: &'b str,
        port: u16,
    ) -> RedisFuture<'a, Box<dyn Iterator<Item = SocketAddr> + Send + 'a>> {
        async move {
            use hickory_resolver::Resolver;
            use hickory_resolver::name_server::TokioConnectionProvider;

            // Build a resolver using the host system configuration (/etc/resolv.conf etc.).
            let resolver = Resolver::builder_with_config(
                hickory_resolver::config::ResolverConfig::default(),
                TokioConnectionProvider::default(),
            )
            .build();

            // Perform the lookup.
            let response = resolver
                .lookup_ip(host)
                .await
                .map_err(|e| RedisError::from((
                    ErrorKind::IoError,
                    "DNS resolution failed",
                    e.to_string(),
                )))?;

            // Convert the resulting IP addresses into SocketAddr values.
            let addrs: Vec<SocketAddr> = response
                .iter()
                .map(|ip| SocketAddr::new(ip, port))
                .collect();

            if addrs.is_empty() {
                Err(RedisError::from((
                    ErrorKind::InvalidClientConfig,
                    "No address found for host",
                )))
            } else {
                Ok(Box::new(addrs.into_iter())
                    as Box<dyn Iterator<Item = SocketAddr> + Send + 'a>)
            }
        }
        .boxed()
    }
}