//! # Artifact Management
//!
//! Handles downloading and extracting FluxCD and ArgoCD artifacts.

use crate::controller::reconciler::types::Reconciler;
use crate::controller::reconciler::utils::{sanitize_path_component, SMC_BASE_PATH};
use crate::SourceRef;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tracing::{debug, error, info, info_span, warn, Instrument};

/// Get FluxCD GitRepository resource
#[allow(
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    reason = "Markdown formatting is intentional and error docs are in comments"
)]
pub async fn get_flux_git_repository(
    _reconciler: &Reconciler,
    source_ref: &SourceRef,
) -> Result<serde_json::Value> {
    // Use Kubernetes API to get GitRepository
    // GitRepository is a CRD from source.toolkit.fluxcd.io/v1beta2
    use kube::api::ApiResource;
    use kube::core::DynamicObject;

    let span = info_span!(
        "gitrepository.get_artifact",
        gitrepository.name = source_ref.name,
        namespace = source_ref.namespace
    );
    let span_clone = span.clone();
    let start = Instant::now();

    async move {
        let ar = ApiResource::from_gvk(&kube::core::GroupVersionKind {
            group: "source.toolkit.fluxcd.io".to_string(),
            version: "v1beta2".to_string(),
            kind: "GitRepository".to_string(),
        });

        let api: kube::Api<DynamicObject> =
            kube::Api::namespaced_with(_reconciler.client.clone(), &source_ref.namespace, &ar);

        let git_repo = api.get(&source_ref.name).await.context(format!(
            "Failed to get FluxCD GitRepository: {}/{}",
            source_ref.namespace, source_ref.name
        ))?;

        span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
        span_clone.record("operation.success", true);
        Ok(serde_json::to_value(git_repo)?)
    }
    .instrument(span)
    .await
}

/// Get artifact path from FluxCD GitRepository status
/// Downloads and extracts the tar.gz artifact from FluxCD source-controller HTTP service
/// Returns the path to the extracted directory
#[allow(
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    reason = "Markdown formatting is intentional, error docs in comments"
)]
pub async fn get_flux_artifact_path(
    reconciler: &Reconciler,
    git_repo: &serde_json::Value,
) -> Result<PathBuf> {
    // Extract artifact information from GitRepository status
    // FluxCD stores artifacts as tar.gz files accessible via HTTP from source-controller
    let status = git_repo
        .get("status")
        .and_then(|s| s.get("artifact"))
        .context("FluxCD GitRepository has no artifact in status")?;

    // Get artifact URL - this is the HTTP endpoint to download the tar.gz
    // FluxCD sometimes includes a dot before the path (e.g., cluster.local./path)
    // which causes HTTP requests to fail, so we normalize it
    let artifact_url_raw = status
        .get("url")
        .and_then(|u| u.as_str())
        .context("FluxCD GitRepository artifact has no URL")?;

    // Normalize URL: remove dots before path separators (e.g., cluster.local./path -> cluster.local/path)
    // This handles cases where Kubernetes DNS FQDNs include trailing dots before paths
    let artifact_url = artifact_url_raw
        .replace("./", "/")
        .trim_end_matches('.')
        .to_string();

    // Get revision for caching - use revision to determine if we need to re-download
    let revision = status
        .get("revision")
        .and_then(|r| r.as_str())
        .unwrap_or("unknown");

    // Extract branch name and short SHA from revision
    // FluxCD revision format: "main@sha1:7680da431ea59ae7d3f4fdbb903a0f4509da9078"
    // We need both branch and SHA to avoid conflicts when same SHA exists on different branches
    let (branch_name, short_sha) = if let Some(at_pos) = revision.find('@') {
        // Extract branch name (before @)
        let branch = &revision[..at_pos];
        let sanitized_branch = sanitize_path_component(branch);

        // Extract SHA (after @sha1: or @sha256:)
        let sha = if let Some(sha_start) = revision.find("sha1:") {
            &revision[sha_start + 5..]
        } else if let Some(sha_start) = revision.find("sha256:") {
            &revision[sha_start + 7..]
        } else {
            // No SHA found, use full revision after @
            &revision[at_pos + 1..]
        };

        let short_sha = if sha.len() >= 7 { &sha[..7] } else { sha };

        (sanitized_branch, short_sha.to_string())
    } else {
        // No @ separator found, treat entire revision as branch
        (sanitize_path_component(revision), "unknown".to_string())
    };

    // Create revision directory name: {branch}-sha-{short_sha}
    // Example: "main-sha-7680da4" or "old-branch-sha-7680da4"
    let revision_dir = format!("{}-sha-{}", branch_name, short_sha);

    // Get metadata for constructing cache path
    let metadata = git_repo
        .get("metadata")
        .context("FluxCD GitRepository has no metadata")?;

    let name = metadata
        .get("name")
        .and_then(|n| n.as_str())
        .context("FluxCD GitRepository has no name")?;

    let namespace = metadata
        .get("namespace")
        .and_then(|n| n.as_str())
        .context("FluxCD GitRepository has no namespace")?;

    // Create hierarchical cache directory path: /tmp/smc/flux-artifact/{namespace}/{name}/{branch}-sha-{short_sha}/
    // This structure:
    // 1. Avoids performance issues with many files in a single directory
    // 2. Allows cluster owners to mount a PVC at /tmp/smc for persistent storage
    // 3. Provides clear organization by namespace, name, branch, and SHA
    // 4. Uses branch name + short SHA (7 chars) to avoid conflicts when same SHA exists on different branches
    // 5. Cleanup uses mtime (filesystem modification time) to determine oldest revisions per branch
    let sanitized_namespace = sanitize_path_component(namespace);
    let sanitized_name = sanitize_path_component(name);

    let cache_path = PathBuf::from(SMC_BASE_PATH)
        .join("flux-artifact")
        .join(&sanitized_namespace)
        .join(&sanitized_name)
        .join(&revision_dir);

    // Check if artifact is already cached (directory exists and is not empty)
    if cache_path.exists() && cache_path.is_dir() {
        // Verify cache is valid by checking if it contains files
        if let Ok(mut entries) = std::fs::read_dir(&cache_path) {
            if entries.next().is_some() {
                info!(
                    "Using cached FluxCD artifact at {} (revision: {}, dir: {})",
                    cache_path.display(),
                    revision,
                    revision_dir
                );
                return Ok(cache_path);
            }
        }
    }

    // Download and extract artifact with OTEL spans and metrics
    let download_span = info_span!(
        "artifact.download",
        artifact.url = artifact_url.as_str(),
        artifact.revision = revision,
        artifact.cache_path = cache_path.display().to_string()
    );
    let download_start = Instant::now();
    crate::observability::metrics::increment_artifact_downloads_total();

    info!(
        "Downloading FluxCD artifact from {} (revision: {}, dir: {})",
        artifact_url, revision, revision_dir
    );

    // Create cache directory
    tokio::fs::create_dir_all(&cache_path)
        .await
        .context(format!(
            "Failed to create cache directory: {}",
            cache_path.display()
        ))?;

    // Download tar.gz file to temporary location
    let temp_tar = cache_path.join("artifact.tar.gz");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .context("Failed to create HTTP client")?;

    let response = match client.get(&artifact_url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            // Provide detailed error information for debugging network issues
            let error_msg = format!("{:?}", e);
            let error_str = format!("{}", e);

            // Check error type and provide specific guidance
            let is_timeout = error_msg.contains("timeout")
                || error_msg.contains("timed out")
                || error_str.contains("timeout")
                || error_str.contains("timed out");
            let is_dns = error_msg.contains("dns")
                || error_msg.contains("resolve")
                || error_msg.contains("Dns")
                || error_str.contains("dns")
                || error_str.contains("resolve");
            let is_connection = error_msg.contains("connection")
                || error_msg.contains("connect")
                || error_msg.contains("Connection")
                || error_str.contains("connection")
                || error_str.contains("connect");
            let is_builder = error_msg.contains("builder") || error_msg.contains("Builder");

            error!("Failed to download artifact from {}: {}", artifact_url, e);
            error!("Error details: {:?}", e);

            if is_timeout {
                error!("Network timeout detected - source-controller may be unreachable or slow to respond");
                error!("Troubleshooting:");
                error!("  1. Check service: kubectl get svc source-controller -n flux-system");
                error!("  2. Check pods: kubectl get pods -n flux-system -l app=source-controller");
                error!(
                    "  3. Check endpoints: kubectl get endpoints source-controller -n flux-system"
                );
                error!("  4. Test connectivity from controller pod");
            } else if is_dns {
                error!("DNS resolution failed - check if source-controller.flux-system.svc.cluster.local resolves");
                error!("Troubleshooting:");
                error!("  1. Check DNS: kubectl exec -n microscaler-system <pod> -- nslookup source-controller.flux-system.svc.cluster.local");
                error!(
                    "  2. Verify service exists: kubectl get svc source-controller -n flux-system"
                );
            } else if is_connection {
                error!("Connection failed - check network policies and service endpoints");
                error!("Troubleshooting:");
                error!(
                    "  1. Check endpoints: kubectl get endpoints source-controller -n flux-system"
                );
                error!("  2. Check network policies: kubectl get networkpolicies -A");
                error!("  3. Verify service targetPort matches pod containerPort");
            } else if is_builder {
                error!("HTTP client builder error - check reqwest configuration");
            } else {
                error!("Unknown network error - full error: {:?}", e);
                error!("Troubleshooting:");
                error!("  1. Verify source-controller is running: kubectl get pods -n flux-system -l app=source-controller");
                error!("  2. Check service: kubectl get svc source-controller -n flux-system");
                error!("  3. Test from controller pod: kubectl exec -n microscaler-system <pod> -- curl -v <url>");
            }

            crate::observability::metrics::increment_artifact_download_errors_total();
            download_span.record("operation.success", false);
            download_span.record("error.message", format!("{}", e));
            return Err(anyhow::anyhow!(
                "Failed to download artifact from {}: {} (details: {:?})",
                artifact_url,
                e,
                e
            ));
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let status_text = response.status().canonical_reason().unwrap_or("Unknown");
        crate::observability::metrics::increment_artifact_download_errors_total();
        download_span.record("operation.success", false);
        download_span.record("error.status_code", status.as_u16() as u64);
        error!(
            "Artifact download returned HTTP {} {} from {}",
            status.as_u16(),
            status_text,
            artifact_url
        );
        return Err(anyhow::anyhow!(
            "Failed to download artifact: HTTP {} {}",
            status.as_u16(),
            status_text
        ));
    }

    // Verify Content-Length matches actual download size (detect partial downloads)
    let expected_size = response.content_length();
    let mut file = tokio::fs::File::create(&temp_tar).await.context(format!(
        "Failed to create temp file: {}",
        temp_tar.display()
    ))?;

    // Stream download to detect partial downloads and verify size
    let mut downloaded_size: u64 = 0;
    let mut stream = response.bytes_stream();
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.context("Failed to read chunk from download stream")?;
        downloaded_size += chunk.len() as u64;
        file.write_all(&chunk)
            .await
            .context("Failed to write chunk to file")?;
    }

    drop(file); // Close file before verification

    // Verify download size matches Content-Length (if provided)
    if let Some(expected) = expected_size {
        if downloaded_size != expected {
            // Clean up partial download
            let _ = tokio::fs::remove_file(&temp_tar).await;
            return Err(anyhow::anyhow!(
                "Partial download detected: expected {} bytes, got {} bytes",
                expected,
                downloaded_size
            ));
        }
    }

    // Verify file is not empty
    if downloaded_size == 0 {
        crate::observability::metrics::increment_artifact_download_errors_total();
        download_span.record("operation.success", false);
        download_span.record("error.message", "Downloaded artifact is empty");
        let _ = tokio::fs::remove_file(&temp_tar).await;
        return Err(anyhow::anyhow!("Downloaded artifact is empty"));
    }

    // Record successful download metrics and span
    let download_duration = download_start.elapsed().as_secs_f64();
    crate::observability::metrics::observe_artifact_download_duration(download_duration);
    download_span.record(
        "operation.duration_ms",
        download_start.elapsed().as_millis() as u64,
    );
    download_span.record("operation.success", true);
    download_span.record("artifact.size_bytes", downloaded_size);

    // Verify checksum if provided by FluxCD
    // FluxCD provides digest in artifact status (e.g., "sha256:...")
    if let Some(digest_str) = status.get("digest").and_then(|d| d.as_str()) {
        use sha2::{Digest, Sha256};
        use std::io::Read;

        // Read file and compute SHA256
        let mut file = std::fs::File::open(&temp_tar)
            .context("Failed to open downloaded file for checksum verification")?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 8192];
        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }
        let computed_hash = format!("sha256:{:x}", hasher.finalize());

        // Extract hash from digest (format: "sha256:...")
        if digest_str != computed_hash {
            // Clean up invalid artifact
            let _ = tokio::fs::remove_file(&temp_tar).await;
            return Err(anyhow::anyhow!(
                "Checksum mismatch: expected {}, got {}. Artifact may be corrupt or tampered.",
                digest_str,
                computed_hash
            ));
        }
        debug!("Checksum verified: {}", digest_str);
    }

    // Verify file is a valid tar.gz by checking magic bytes
    // tar.gz files start with gzip magic bytes: 1f 8b
    // This prevents processing non-tar.gz files that could cause extraction errors
    let mut magic_buffer = [0u8; 2];
    if let Ok(mut file) = std::fs::File::open(&temp_tar) {
        use std::io::Read;
        if file.read_exact(&mut magic_buffer).is_ok() {
            if magic_buffer != [0x1f, 0x8b] {
                // Clean up invalid file
                let _ = tokio::fs::remove_file(&temp_tar).await;
                return Err(anyhow::anyhow!(
                    "Invalid file format: expected tar.gz (gzip), got magic bytes {:02x}{:02x}. File may be corrupt or wrong format.",
                    magic_buffer[0],
                    magic_buffer[1]
                ));
            }
            debug!("File format verified: valid gzip magic bytes");
        }
    }

    // Extract tar.gz file with security protections and OTEL spans
    let extract_span = info_span!(
        "artifact.extract",
        artifact.cache_path = cache_path.display().to_string(),
        artifact.size_bytes = downloaded_size
    );
    let extract_start = Instant::now();
    crate::observability::metrics::increment_artifact_extractions_total();

    info!(
        "Extracting artifact to {} (size: {} bytes)",
        cache_path.display(),
        downloaded_size
    );

    // Use tar command to extract with security flags:
    // - --strip-components=0: Preserve directory structure
    // - --warning=no-unknown-keyword: Suppress warnings for unknown keywords
    // - -C: Extract to specific directory (prevents path traversal)
    // Note: tar automatically prevents extraction outside -C directory on most systems
    let extract_output = tokio::process::Command::new("tar")
        .arg("-xzf")
        .arg(&temp_tar)
        .arg("-C")
        .arg(&cache_path)
        .arg("--strip-components=0") // Preserve directory structure
        .arg("--warning=no-unknown-keyword") // Suppress warnings
        .output()
        .await
        .context("Failed to execute tar command")?;

    if !extract_output.status.success() {
        let stderr = String::from_utf8_lossy(&extract_output.stderr);
        crate::observability::metrics::increment_artifact_extraction_errors_total();
        extract_span.record("operation.success", false);
        extract_span.record("error.message", stderr.to_string());
        // Clean up on extraction failure
        let _ = tokio::fs::remove_file(&temp_tar).await;
        // Also clean up partial extraction directory
        let _ = tokio::fs::remove_dir_all(&cache_path).await;
        return Err(anyhow::anyhow!(
            "Failed to extract artifact (corrupt or invalid tar.gz): {}",
            stderr
        ));
    }

    // Verify extraction succeeded by checking if directory contains files
    let mut entries = tokio::fs::read_dir(&cache_path)
        .await
        .context("Failed to read extracted directory")?;
    let has_files = entries.next_entry().await?.is_some();
    if !has_files {
        crate::observability::metrics::increment_artifact_extraction_errors_total();
        extract_span.record("operation.success", false);
        extract_span.record("error.message", "Extraction produced empty directory");
        // Clean up empty extraction
        let _ = tokio::fs::remove_file(&temp_tar).await;
        let _ = tokio::fs::remove_dir_all(&cache_path).await;
        return Err(anyhow::anyhow!(
            "Artifact extraction produced empty directory - artifact may be corrupt"
        ));
    }

    // Record successful extraction metrics and span
    let extract_duration = extract_start.elapsed().as_secs_f64();
    crate::observability::metrics::observe_artifact_extraction_duration(extract_duration);
    extract_span.record(
        "operation.duration_ms",
        extract_start.elapsed().as_millis() as u64,
    );
    extract_span.record("operation.success", true);

    // Clean up temporary tar file after successful extraction
    if let Err(e) = tokio::fs::remove_file(&temp_tar).await {
        warn!(
            "Failed to remove temporary tar file {}: {}",
            temp_tar.display(),
            e
        );
        // Don't fail reconciliation if cleanup fails
    }

    // Clean up old revisions - keep only the 3 newest revisions per namespace/name
    // This prevents disk space from growing unbounded
    if let Err(e) = cleanup_old_revisions(&cache_path.parent().unwrap()).await {
        warn!("Failed to cleanup old revisions: {}", e);
        // Don't fail reconciliation if cleanup fails
    }

    info!(
        "Successfully downloaded and extracted FluxCD artifact to {} (revision: {}, dir: {})",
        cache_path.display(),
        revision,
        revision_dir
    );

    Ok(cache_path)
}

/// Clean up old revisions, keeping only the 3 newest per namespace/name combination
/// Removes the 4th oldest revision and any older ones to prevent unbounded disk growth
pub async fn cleanup_old_revisions(parent_dir: &Path) -> Result<()> {
    use std::time::SystemTime;

    // List all revision directories
    let mut entries = Vec::new();
    let mut dir_entries = tokio::fs::read_dir(parent_dir)
        .await
        .context("Failed to read parent directory for cleanup")?;

    while let Some(entry) = dir_entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            // Get modification time to determine age
            let metadata = tokio::fs::metadata(&path).await?;
            let modified = metadata
                .modified()
                .unwrap_or_else(|_| SystemTime::UNIX_EPOCH);

            entries.push((path, modified));
        }
    }

    // If we have 4 or more revisions, remove the oldest ones (keep 3 newest)
    if entries.len() >= 4 {
        // Sort by modification time (newest first)
        entries.sort_by(|a, b| b.1.cmp(&a.1));

        // Remove all but the 3 newest
        let to_remove = entries.split_off(3);

        for (path, _) in to_remove {
            info!("Removing old revision cache: {}", path.display());
            if let Err(e) = tokio::fs::remove_dir_all(&path).await {
                warn!("Failed to remove old revision {}: {}", path.display(), e);
                // Continue removing others even if one fails
            }
        }
    }

    Ok(())
}

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
