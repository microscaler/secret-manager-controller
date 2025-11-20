//! # SOPS Decryption
//!
//! Handles SOPS-encrypted file decryption using the sops binary.
//!
//! ## Module Structure
//!
//! - `detection.rs` - SOPS encryption detection
//! - `decrypt.rs` - Main decryption logic
//! - `gpg.rs` - GPG key management
//! - `error.rs` - Error types and classification

pub mod decrypt;
pub mod detection;
pub mod error;
pub mod gpg;

// Re-export public API
pub use decrypt::decrypt_sops_content;
pub use detection::{is_sops_encrypted, is_sops_encrypted_impl};
