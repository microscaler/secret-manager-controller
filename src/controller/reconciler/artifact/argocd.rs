//! # ArgoCD Artifact Handling
//!
//! Handles ArgoCD Application artifacts.
//! Clones Git repositories directly from ArgoCD Application specs.

use crate::controller::reconciler::types::Reconciler;
use crate::controller::reconciler::utils::{sanitize_path_component, SMC_BASE_PATH};
use crate::crd::SourceRef;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::Instant;
use tracing::{info, info_span, warn, Instrument};

use super::download::cleanup_old_revisions;

/// Get artifact path from ArgoCD Application
/// Clones the Git repository directly from the Application spec
#[allow(
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::too_many_lines,
    reason = "Markdown formatting is intentional, error docs in comments, complex logic"
)]
pub async fn get_argocd_artifact_path(
    reconciler: &Reconciler,
    source_ref: &SourceRef,
) -> Result<PathBuf> {
    use kube::api::ApiResource;
    use kube::core::DynamicObject;

    // Get ArgoCD Application CRD
    // Application is from argoproj.io/v1alpha1
    let ar = ApiResource::from_gvk(&kube::core::GroupVersionKind {
        group: "argoproj.io".to_string(),
        version: "v1alpha1".to_string(),
        kind: "Application".to_string(),
    });

    let api: kube::Api<DynamicObject> =
        kube::Api::namespaced_with(reconciler.client.clone(), &source_ref.namespace, &ar);

    let application = api.get(&source_ref.name).await.context(format!(
        "Failed to get ArgoCD Application: {}/{}",
        source_ref.namespace, source_ref.name
    ))?;

    // Extract Git source from Application spec
    let spec = application
        .data
        .get("spec")
        .context("ArgoCD Application has no spec")?;

    let source = spec
        .get("source")
        .context("ArgoCD Application has no source in spec")?;

    let repo_url = source
        .get("repoURL")
        .and_then(|u| u.as_str())
        .context("ArgoCD Application source has no repoURL")?;

    let target_revision = source
        .get("targetRevision")
        .and_then(|r| r.as_str())
        .unwrap_or("HEAD");

    info!(
        "ArgoCD Application source: repo={}, revision={}",
        repo_url, target_revision
    );

    // Clone repository to hierarchical cache directory: /tmp/smc/argocd-repo/{namespace}/{name}/{hash}/
    // This structure:
    // 1. Avoids performance issues with many files in a single directory
    // 2. Allows cluster owners to mount a PVC at /tmp/smc for persistent storage
    // 3. Uses hash for revision to handle long/branch names safely
    let sanitized_namespace = sanitize_path_component(&source_ref.namespace);
    let sanitized_name = sanitize_path_component(&source_ref.name);
    let repo_hash = format!(
        "{:x}",
        md5::compute(format!(
            "{}-{}-{}",
            source_ref.namespace, source_ref.name, target_revision
        ))
    );

    let path_buf = PathBuf::from(SMC_BASE_PATH)
        .join("argocd-repo")
        .join(&sanitized_namespace)
        .join(&sanitized_name)
        .join(&repo_hash);

    let clone_path = path_buf.to_string_lossy().to_string();

    // Check if repository already exists and is at the correct revision
    if path_buf.exists() {
        // Verify the revision matches by checking HEAD
        let git_dir = path_buf.join(".git");
        if git_dir.exists() || path_buf.join("HEAD").exists() {
            // Check current HEAD revision
            let output = tokio::process::Command::new("git")
                .arg("-C")
                .arg(&path_buf)
                .arg("rev-parse")
                .arg("HEAD")
                .output()
                .await;

            if let Ok(output) = output {
                if output.status.success() {
                    let current_rev = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    // Try to resolve target revision
                    let target_output = tokio::process::Command::new("git")
                        .arg("-C")
                        .arg(&path_buf)
                        .arg("rev-parse")
                        .arg(target_revision)
                        .output()
                        .await;

                    if let Ok(target_output) = target_output {
                        if target_output.status.success() {
                            let target_rev = String::from_utf8_lossy(&target_output.stdout)
                                .trim()
                                .to_string();
                            if current_rev == target_rev {
                                info!(
                                    "Using cached ArgoCD repository at {} (revision: {})",
                                    clone_path, target_revision
                                );
                                return Ok(path_buf);
                            }
                        }
                    }
                }
            }
        }
        // Remove stale repository
        if let Err(e) = tokio::fs::remove_dir_all(&path_buf).await {
            warn!("Failed to remove stale repository at {}: {}", clone_path, e);
        }
    }

    // Clone the repository using git command
    let clone_path_for_match = clone_path.clone();
    let path_buf_for_match = path_buf.clone();
    let span = info_span!(
        "git.clone",
        repository.url = repo_url,
        clone.path = clone_path,
        revision = target_revision
    );
    let span_clone_for_match = span.clone();
    let span_clone = span.clone();
    let start = Instant::now();

    let clone_result = async move {
        info!(
            "Cloning ArgoCD repository: {} (revision: {})",
            repo_url, target_revision
        );

        // Create parent directory
        let parent_dir = path_buf.parent().ok_or_else(|| {
            anyhow::anyhow!("Cannot determine parent directory for path: {clone_path}")
        })?;
        tokio::fs::create_dir_all(parent_dir)
            .await
            .context(format!(
                "Failed to create parent directory for {clone_path}"
            ))?;

        // Clone repository (shallow clone for efficiency)
        // First try shallow clone with branch (works for branch/tag names)
        let clone_output = tokio::process::Command::new("git")
            .arg("clone")
            .arg("--depth")
            .arg("1")
            .arg("--branch")
            .arg(target_revision)
            .arg(repo_url)
            .arg(&clone_path)
            .output()
            .await
            .context(format!("Failed to execute git clone for {repo_url}"))?;

        if !clone_output.status.success() {
            // If branch clone fails, clone default branch and checkout specific revision
            // This handles commit SHAs and other revision types
            let clone_output = tokio::process::Command::new("git")
                .arg("clone")
                .arg("--depth")
                .arg("50") // Deeper clone to ensure revision is available
                .arg(repo_url)
                .arg(&clone_path)
                .output()
                .await
                .context(format!("Failed to execute git clone for {repo_url}"))?;

            if !clone_output.status.success() {
                let error_msg = String::from_utf8_lossy(&clone_output.stderr);
                span_clone.record("operation.success", false);
                span_clone.record("error.message", error_msg.to_string());
                crate::observability::metrics::increment_git_clone_errors_total();
                return Err(anyhow::anyhow!(
                    "Failed to clone repository {repo_url}: {error_msg}"
                ));
            }

            // Fetch the specific revision if needed
            let _fetch_output = tokio::process::Command::new("git")
                .arg("-C")
                .arg(&clone_path)
                .arg("fetch")
                .arg("--depth")
                .arg("50")
                .arg("origin")
                .arg(target_revision)
                .output()
                .await;

            // Checkout specific revision
            let checkout_output = tokio::process::Command::new("git")
                .arg("-C")
                .arg(&clone_path)
                .arg("checkout")
                .arg(target_revision)
                .output()
                .await
                .context(format!(
                    "Failed to checkout revision {target_revision} in repository {repo_url}"
                ))?;

            if !checkout_output.status.success() {
                let error_msg = String::from_utf8_lossy(&checkout_output.stderr);
                span_clone.record("operation.success", false);
                span_clone.record("error.message", error_msg.to_string());
                crate::observability::metrics::increment_git_clone_errors_total();
                return Err(anyhow::anyhow!(
                    "Failed to checkout revision {target_revision} in repository {repo_url}: {error_msg}"
                ));
            }
        }

        Ok(())
    }
    .instrument(span)
    .await;

    match clone_result {
        Ok(_) => {
            span_clone_for_match
                .record("operation.duration_ms", start.elapsed().as_millis() as u64);
            span_clone_for_match.record("operation.success", true);
            crate::observability::metrics::increment_git_clone_total();
            crate::observability::metrics::observe_git_clone_duration(
                start.elapsed().as_secs_f64(),
            );
            info!(
                "Successfully cloned ArgoCD repository to {} (revision: {})",
                clone_path_for_match, target_revision
            );

            // Clean up old revisions - keep only the 3 newest revisions per namespace/name
            // This prevents disk space from growing unbounded
            if let Err(e) = cleanup_old_revisions(&path_buf_for_match.parent().unwrap()).await {
                warn!("Failed to cleanup old ArgoCD revisions: {}", e);
                // Don't fail reconciliation if cleanup fails
            }

            Ok(path_buf_for_match)
        }
        Err(e) => {
            span_clone_for_match
                .record("operation.duration_ms", start.elapsed().as_millis() as u64);
            span_clone_for_match.record("operation.success", false);
            Err(e)
        }
    }
}
