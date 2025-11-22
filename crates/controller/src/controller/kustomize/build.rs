//! # Kustomize Build Execution
//!
//! Handles execution of `kustomize build` command.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use tracing::{error, info};

/// Run kustomize build on the specified path
/// Returns the YAML output as a string
pub fn run_kustomize_build(artifact_path: &Path, kustomize_path: &str) -> Result<String> {
    let full_path = artifact_path.join(kustomize_path);

    // Validate path exists
    if !full_path.exists() {
        return Err(anyhow::anyhow!(
            "Kustomize path does not exist: {}",
            full_path.display()
        ));
    }

    // Check if kustomization.yaml exists
    let kustomization_file = full_path.join("kustomization.yaml");
    if !kustomization_file.exists() {
        return Err(anyhow::anyhow!(
            "kustomization.yaml not found at: {}",
            kustomization_file.display()
        ));
    }

    info!("Running kustomize build on path: {}", full_path.display());

    // Run kustomize build
    let output = Command::new("kustomize")
        .arg("build")
        .arg(&full_path)
        .current_dir(artifact_path)
        .output()
        .context("Failed to execute kustomize build")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("Kustomize build failed: {}", stderr);
        return Err(anyhow::anyhow!("Kustomize build failed: {stderr}"));
    }

    let yaml_output =
        String::from_utf8(output.stdout).context("Failed to decode kustomize output as UTF-8")?;

    Ok(yaml_output)
}
