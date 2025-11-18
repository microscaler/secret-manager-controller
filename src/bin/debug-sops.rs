#!/usr/bin/env rust-script
//! Debug program to decrypt SOPS files locally
//!
//! Usage:
//!   cargo run --bin debug-sops -- deployment-configuration/profiles/tilt/application.secrets.env
//!
//! Or with GPG key:
//!   SOPS_PRIVATE_KEY="$(cat path/to/key.asc)" cargo run --bin debug-sops -- deployment-configuration/profiles/tilt/application.secrets.env
//!
//! # Optimal Output Type for Cloud Secret Stores
//!
//! The output type matches the input type to preserve format for parsing:
//!
//! ## .env / application.secrets.env files
//! - **Input type**: `dotenv`
//! - **Output type**: `dotenv` (matches input)
//! - **Why**: Decrypted content is parsed line-by-line as `KEY=VALUE` pairs
//! - **Storage**: Each key-value pair stored as individual secret in cloud provider
//!
//! ## .yaml / application.secrets.yaml files
//! - **Input type**: `yaml`
//! - **Output type**: `yaml` (matches input)
//! - **Why**: Decrypted content is parsed with `serde_yaml`, then flattened
//! - **Storage**: Flattened key-value pairs stored as individual secrets in cloud provider
//!
//! ## Cloud Provider Storage
//! - **GCP Secret Manager**: Stores secrets as strings
//! - **AWS Secrets Manager**: Stores secrets as strings (or JSON strings)
//! - **Azure Key Vault**: Stores secrets as strings
//!
//! **Key Insight**: Format preservation is only needed for parsing, not storage.
//! After parsing, all secrets are stored as strings in cloud providers.
//! The output type must match input type so the parser receives the expected format.

use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> Result<()> {
    // Get file path from command line args
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path-to-sops-file>", args[0]);
        eprintln!(
            "Example: {} deployment-configuration/profiles/tilt/application.secrets.env",
            args[0]
        );
        eprintln!("\nOptional: Set SOPS_PRIVATE_KEY environment variable to provide GPG key");
        std::process::exit(1);
    }

    let file_path = PathBuf::from(&args[1]);

    if !file_path.exists() {
        eprintln!("❌ File not found: {}", file_path.display());
        std::process::exit(1);
    }

    println!("🔍 Debug SOPS Decryption");
    println!("📄 File: {}", file_path.display());
    println!();

    // Read encrypted file
    println!("📖 Reading encrypted file...");
    let encrypted_content = fs::read_to_string(&file_path)
        .context(format!("Failed to read file: {}", file_path.display()))?;

    println!("   File size: {} bytes", encrypted_content.len());
    println!(
        "   First 100 chars: {}",
        &encrypted_content.chars().take(100).collect::<String>()
    );
    println!();

    // Get GPG key from environment variable (optional)
    let sops_private_key = env::var("SOPS_PRIVATE_KEY").ok();
    if sops_private_key.is_some() {
        println!("🔑 GPG key provided via SOPS_PRIVATE_KEY environment variable");
        println!(
            "   Key length: {} bytes",
            sops_private_key.as_ref().unwrap().len()
        );
    } else {
        println!("⚠️  No GPG key provided - will use system keyring");
        println!("   Set SOPS_PRIVATE_KEY environment variable to provide key");
    }
    println!();

    // Decrypt using the same logic as the controller
    println!("🔓 Decrypting with SOPS binary...");
    let decrypted =
        decrypt_with_sops_binary(&encrypted_content, &file_path, sops_private_key.as_deref())
            .await
            .context("SOPS decryption failed")?;

    println!("✅ Decryption successful!");
    println!("   Decrypted size: {} bytes", decrypted.len());
    println!();
    println!("📋 Decrypted content:");
    println!("{}", "─".repeat(80));
    println!("{}", decrypted);
    println!("{}", "─".repeat(80));

    Ok(())
}

/// Decrypt SOPS content using sops binary via temporary file
/// This matches the implementation in src/controller/parser/sops.rs
async fn decrypt_with_sops_binary(
    content: &str,
    file_path: &PathBuf,
    sops_private_key: Option<&str>,
) -> Result<String> {
    // Check if sops binary is available
    let sops_path = which::which("sops")
        .context("sops binary not found in PATH. Please install sops: brew install sops (macOS) or see https://github.com/mozilla/sops")?;

    println!("   Using sops binary: {:?}", sops_path);

    // Set up GPG keyring if private key is provided
    let gpg_home = if let Some(private_key) = sops_private_key {
        println!("   Importing GPG private key into temporary keyring...");
        import_gpg_key(private_key).await?
    } else {
        None
    };

    // SECURITY: Use stdin/stdout pipes - no disk writes
    // Writing secrets to disk (even temporarily) is a security breach
    // With --input-type and --output-type flags, SOPS can read from stdin reliably

    // Determine input/output type from file extension or content
    // Output type must match input type to preserve format for parsing:
    // - dotenv → KEY=VALUE format → parsed line-by-line → stored as individual secrets
    // - yaml → YAML format → parsed with serde_yaml → flattened → stored as individual secrets
    // - Cloud providers (GCP/AWS/Azure) store secrets as strings, so format preservation
    //   is only needed for parsing, not for storage
    let input_type = if let Some(ext) = file_path.extension().and_then(|s| s.to_str()) {
        match ext {
            "env" => "dotenv",
            "yaml" | "yml" => "yaml",
            "json" => "json",
            _ => {
                // Check filename for application.secrets.env pattern
                let filename = file_path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains("application.secrets.env") {
                    "dotenv"
                } else if filename.contains("application.secrets.yaml") {
                    "yaml"
                } else {
                    // Content-based detection fallback
                    if content.trim_start().starts_with('{') {
                        "json"
                    } else if content.trim_start().contains('=')
                        && !content.trim_start().starts_with("sops:")
                    {
                        "dotenv"
                    } else {
                        "yaml" // Default to YAML
                    }
                }
            }
        }
    } else {
        // No extension, check filename
        let filename = file_path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if filename.contains("application.secrets.env") {
            "dotenv"
        } else if filename.contains("application.secrets.yaml") {
            "yaml"
        } else {
            // Content-based detection
            if content.trim_start().starts_with('{') {
                "json"
            } else if content.trim_start().contains('=')
                && !content.trim_start().starts_with("sops:")
            {
                "dotenv"
            } else {
                "yaml"
            }
        }
    };

    println!(
        "   Detected input type: {} (output type matches for parsing)",
        input_type
    );
    println!("   → dotenv: parsed as KEY=VALUE lines → individual secrets");
    println!("   → yaml: parsed as YAML → flattened → individual secrets");
    println!("   → Cloud providers store secrets as strings (format only matters for parsing)");

    // Prepare sops command to read from stdin
    // SECURITY: Use stdin/stdout pipes - no disk writes
    // With --input-type and --output-type flags, SOPS can read from stdin reliably
    let mut cmd = tokio::process::Command::new(sops_path);
    cmd.arg("-d") // Decrypt
        .arg("--input-type") // Specify input type explicitly
        .arg(input_type) // dotenv, yaml, json, or binary
        .arg("--output-type") // Specify output type to match input type
        .arg(input_type) // Preserve format: dotenv→dotenv, yaml→yaml, json→json
        .arg("/dev/stdin") // Read encrypted content from stdin (POSIX standard)
        .stdin(std::process::Stdio::piped()) // Pipe encrypted content to stdin
        .stdout(std::process::Stdio::piped()) // Capture decrypted content from stdout
        .stderr(std::process::Stdio::piped());

    // Set GPG home directory if we created a temporary one
    if let Some(ref gpg_home_path) = gpg_home {
        cmd.env("GNUPGHOME", gpg_home_path);
        cmd.env("GNUPG_TRUST_MODEL", "always");
        println!("   Using temporary GPG home: {:?}", gpg_home_path);
    }

    println!(
        "   Executing: sops -d --input-type {} --output-type {} /dev/stdin",
        input_type, input_type
    );
    println!("   🔒 SECURITY: Using stdin/stdout pipes - no disk writes");

    // Spawn the process
    let mut child = cmd.spawn().context("Failed to spawn sops command")?;

    // Write encrypted content to stdin (never touches disk)
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(content.as_bytes())
            .await
            .context("Failed to write encrypted content to sops stdin")?;
        stdin
            .shutdown()
            .await
            .context("Failed to close sops stdin")?;
    }

    // Wait for process to complete and capture output
    let output = child
        .wait_with_output()
        .await
        .context("Failed to wait for sops command")?;

    // Clean up temporary GPG home directory
    if let Some(ref gpg_home_path) = gpg_home {
        let _ = tokio::fs::remove_dir_all(gpg_home_path).await;
    }

    if output.status.success() {
        // SECURITY: Decrypted content exists only in memory (from stdout pipe)
        // Never written to disk - only exists in this String
        let decrypted =
            String::from_utf8(output.stdout).context("sops output is not valid UTF-8")?;
        Ok(decrypted)
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        let stdout_msg = String::from_utf8_lossy(&output.stdout);

        eprintln!(
            "   ❌ SOPS decryption failed with exit code: {:?}",
            output.status.code()
        );
        eprintln!("   stderr: {}", error_msg);
        if !stdout_msg.trim().is_empty() {
            eprintln!("   stdout: {}", stdout_msg);
        }

        Err(anyhow::anyhow!(
            "sops decryption failed: {} (exit code: {})",
            error_msg,
            output.status.code().unwrap_or(-1)
        ))
    }
}

/// Import GPG private key into a temporary GPG home directory
async fn import_gpg_key(private_key: &str) -> Result<Option<std::path::PathBuf>> {
    use std::process::Stdio;

    // Check if gpg binary is available
    let gpg_path = match which::which("gpg") {
        Ok(path) => path,
        Err(_) => {
            eprintln!("   ⚠️  gpg binary not found - SOPS decryption may fail");
            return Ok(None);
        }
    };

    // Create temporary GPG home directory
    let temp_dir = std::env::temp_dir();
    let gpg_home = temp_dir.join(format!("gpg-home-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&gpg_home)
        .await
        .context("Failed to create temporary GPG home directory")?;

    // Import private key into temporary keyring
    let gpg_path_for_trust = gpg_path.clone();
    let mut cmd = tokio::process::Command::new(&gpg_path);
    cmd.env("GNUPGHOME", &gpg_home)
        .arg("--batch")
        .arg("--yes")
        .arg("--pinentry-mode")
        .arg("loopback")
        .arg("--import")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().context("Failed to spawn gpg import command")?;

    // Write private key to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(private_key.as_bytes())
            .await
            .context("Failed to write GPG private key to stdin")?;
        stdin.shutdown().await.context("Failed to close stdin")?;
    }

    let output = child
        .wait_with_output()
        .await
        .context("Failed to wait for gpg import command")?;

    if output.status.success() {
        // Trust the imported key by setting ownertrust to ultimate (6)
        let gpg_home_clone = gpg_home.clone();
        let trust_output = tokio::process::Command::new(&gpg_path_for_trust)
            .env("GNUPGHOME", &gpg_home_clone)
            .arg("--list-keys")
            .arg("--with-colons")
            .arg("--fingerprint")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        if let Ok(list_output) = trust_output {
            if list_output.status.success() {
                let output_str = String::from_utf8_lossy(&list_output.stdout);
                for line in output_str.lines() {
                    if line.starts_with("fpr:") {
                        if let Some(fpr_line) = line.split(':').last() {
                            if !fpr_line.is_empty() {
                                // Set ownertrust to ultimate (6) for this fingerprint
                                let trust_cmd = tokio::process::Command::new(&gpg_path_for_trust)
                                    .env("GNUPGHOME", &gpg_home_clone)
                                    .arg("--batch")
                                    .arg("--yes")
                                    .arg("--import-ownertrust")
                                    .stdin(Stdio::piped())
                                    .stdout(Stdio::piped())
                                    .stderr(Stdio::piped())
                                    .spawn();

                                if let Ok(mut trust_child) = trust_cmd {
                                    let trust_input = format!("{}:6:\n", fpr_line);
                                    if let Some(mut stdin) = trust_child.stdin.take() {
                                        let _ = stdin.write_all(trust_input.as_bytes()).await;
                                        let _ = stdin.shutdown().await;
                                    }
                                    let _ = trust_child.wait_with_output().await;
                                }
                                break; // Only trust the first key found
                            }
                        }
                    }
                }
            }
        }

        println!("   ✅ Successfully imported GPG private key");
        Ok(Some(gpg_home))
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        eprintln!("   ❌ Failed to import GPG private key");
        eprintln!("   stderr: {}", error_msg);
        eprintln!("   stdout: {}", stdout);
        // Clean up on failure
        let _ = tokio::fs::remove_dir_all(&gpg_home).await;
        Err(anyhow::anyhow!(
            "Failed to import GPG private key: {error_msg}"
        ))
    }
}
