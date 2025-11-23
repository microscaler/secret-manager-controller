//! Controller integration tests with mock servers
//!
//! This module provides integration tests that exercise the full controller
//! reconciliation flow against mock servers. The tests are organized by provider.

pub mod aws;
pub mod azure;
pub mod common;
pub mod gcp;
