//! # Artifact Management
//!
//! Handles downloading and extracting FluxCD and ArgoCD artifacts.

mod argocd;
mod download;
mod flux;

// Re-export public API
pub use argocd::get_argocd_artifact_path;
pub use flux::{get_flux_artifact_path, get_flux_git_repository};
