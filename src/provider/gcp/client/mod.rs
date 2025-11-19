//! GCP Secret Manager Client Implementation
//!
//! Native REST implementation using reqwest with rustls.
//! This avoids SSL/OpenSSL issues present in the official gRPC SDK.

pub mod common;
pub mod rest;

pub use rest::SecretManagerREST;
