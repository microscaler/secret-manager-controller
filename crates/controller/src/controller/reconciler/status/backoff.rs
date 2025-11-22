//! # Backoff Calculation
//!
//! Calculates progressive backoff durations for error retries.
//!
//! This module provides a unified interface to the Fibonacci backoff implementation
//! in `src/controller/backoff.rs`, ensuring a single source of truth for backoff calculations.

use crate::controller::backoff::FibonacciBackoff;
use std::time::Duration;

/// Calculate progressive backoff duration based on error count
///
/// Uses the shared Fibonacci sequence implementation to gradually increase retry intervals.
/// This prevents controller overload when parsing errors occur.
/// Each resource maintains its own error count independently.
///
/// The sequence follows: 1m, 1m, 2m, 3m, 5m, 8m, 13m, 21m, 34m, 55m, then cap at 60m.
///
/// # Arguments
///
/// * `error_count` - The number of consecutive errors (0-indexed)
///
/// # Returns
///
/// The backoff duration, capped at 60 minutes (3600 seconds).
pub fn calculate_progressive_backoff(error_count: u32) -> Duration {
    // Use the shared Fibonacci backoff implementation
    // min_minutes=1, max_minutes=60 (cap at 60 minutes)
    FibonacciBackoff::calculate_for_error_count(error_count, 1, 60)
}
