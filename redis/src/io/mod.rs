/// Module for defining the TCP settings and behavior.
pub mod tcp;

#[cfg(feature = "aio")]
mod dns;

#[cfg(feature = "aio")]
pub use dns::AsyncDNSResolver;

#[cfg(all(feature = "aio", feature = "hickory-dns"))]
mod hickory;

#[cfg(all(feature = "aio", feature = "hickory-dns"))]
pub use hickory::HickoryAsyncDNSResolver;
