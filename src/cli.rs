//! # MSMCTL CLI
//!
//! Command-line interface for the Microscaler Secret Manager Controller.
//!
//! Similar to `fluxctl`, this CLI tool allows users to trigger reconciliations
//! and interact with the Secret Manager Controller running in Kubernetes.
//!
//! ## Usage
//!
//! ```bash
//! # Trigger reconciliation for a specific SecretManagerConfig
//! msmctl reconcile --namespace default --name my-secrets
//!
//! # List all SecretManagerConfig resources
//! msmctl list
//!
//! # Show status of a SecretManagerConfig
//! msmctl status --namespace default --name my-secrets
//! ```

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use kube::{
    api::{Api, Patch, PatchParams},
    Client,
};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

// Re-define types inline for CLI (avoids circular dependencies)
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[kube(
    kind = "SecretManagerConfig",
    group = "secret-management.microscaler.io",
    version = "v1",
    namespaced,
    status = "SecretManagerConfigStatus",
)]
#[serde(rename_all = "camelCase")]
struct SecretManagerConfigSpec {
    source_ref: SourceRef,
    gcp_project_id: String,
    environment: String,
    #[serde(default)]
    kustomize_path: Option<String>,
    #[serde(default)]
    base_path: Option<String>,
    #[serde(default)]
    secret_prefix: Option<String>,
    #[serde(default)]
    secret_suffix: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct SourceRef {
    #[serde(default = "default_source_kind")]
    kind: String,
    name: String,
    namespace: String,
}

fn default_source_kind() -> String {
    "GitRepository".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct SecretManagerConfigStatus {
    #[serde(default)]
    conditions: Vec<Condition>,
    #[serde(default)]
    observed_generation: Option<i64>,
    #[serde(default)]
    last_reconcile_time: Option<String>,
    #[serde(default)]
    secrets_synced: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Condition {
    r#type: String,
    status: String,
    #[serde(default)]
    last_transition_time: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

/// Microscaler Secret Manager Controller CLI
#[derive(Parser)]
#[command(name = "msmctl")]
#[command(about = "Microscaler Secret Manager Controller CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Kubernetes namespace (defaults to current context namespace)
    #[arg(short, long, global = true)]
    namespace: Option<String>,

    /// Kubernetes context to use
    #[arg(short, long, global = true)]
    context: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Trigger reconciliation for a SecretManagerConfig resource
    Reconcile {
        /// Name of the SecretManagerConfig resource
        #[arg(short, long)]
        name: String,

        /// Namespace of the SecretManagerConfig resource
        #[arg(short, long)]
        namespace: Option<String>,
    },
    /// List all SecretManagerConfig resources
    List {
        /// Namespace to list resources in (defaults to all namespaces)
        #[arg(short, long)]
        namespace: Option<String>,
    },
    /// Show status of a SecretManagerConfig resource
    Status {
        /// Name of the SecretManagerConfig resource
        #[arg(short, long)]
        name: String,

        /// Namespace of the SecretManagerConfig resource
        #[arg(short, long)]
        namespace: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "msmctl=info".into()),
        )
        .init();

    let cli = Cli::parse();

    // Create Kubernetes client
    let client = Client::try_default()
        .await
        .context("Failed to create Kubernetes client. Ensure kubeconfig is configured.")?;

    match cli.command {
        Commands::Reconcile { name, namespace } => {
            reconcile_command(client, name, namespace.or(cli.namespace)).await
        }
        Commands::List { namespace } => list_command(client, namespace.or(cli.namespace)).await,
        Commands::Status { name, namespace } => {
            status_command(client, name, namespace.or(cli.namespace)).await
        }
    }
}

/// Trigger reconciliation by adding/updating an annotation
/// This is the Kubernetes-native approach - the controller watches for annotation changes
async fn reconcile_command(
    client: Client,
    name: String,
    namespace: Option<String>,
) -> Result<()> {
    let ns = namespace.as_deref().unwrap_or("default");
    
    println!("Triggering reconciliation for SecretManagerConfig '{}/{}'...", ns, name);

    // Create API for SecretManagerConfig
    let api: Api<SecretManagerConfig> = Api::namespaced(client, ns);

    // Get current timestamp for annotation
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    // Patch the resource with a reconciliation annotation
    // The controller watches for changes to this annotation and triggers reconciliation
    let patch = json!({
        "metadata": {
            "annotations": {
                "secret-management.microscaler.io/reconcile": timestamp
            }
        }
    });

    let patch_params = PatchParams::apply("msmctl").force();
    
    api.patch(&name, &patch_params, &Patch::Merge(patch))
        .await
        .with_context(|| format!("Failed to trigger reconciliation for '{}/{}'", ns, name))?;

    println!("✅ Reconciliation triggered successfully");
    println!("   Resource: {}/{}", ns, name);
    println!("   Timestamp: {}", timestamp);
    println!("\nThe controller will reconcile this resource shortly.");

    Ok(())
}

/// List all SecretManagerConfig resources
async fn list_command(client: Client, namespace: Option<String>) -> Result<()> {
    let api: Api<SecretManagerConfig> = if let Some(ns) = namespace {
        println!("Listing SecretManagerConfig resources in namespace '{}'...", ns);
        Api::namespaced(client, &ns)
    } else {
        println!("Listing SecretManagerConfig resources in all namespaces...");
        Api::all(client)
    };

    let configs = api
        .list(&kube::api::ListParams::default())
        .await
        .context("Failed to list SecretManagerConfig resources")?;

    if configs.items.is_empty() {
        println!("No SecretManagerConfig resources found.");
        return Ok(());
    }

    println!("\n{:<30} {:<20} {:<15} {:<15}", "NAME", "NAMESPACE", "READY", "SECRETS SYNCED");
    println!("{}", "-".repeat(80));

    for config in configs.items {
        let name = config.metadata.name.as_deref().unwrap_or("<unknown>");
        let ns = config.metadata.namespace.as_deref().unwrap_or("<unknown>");
        
        // Get status
        let ready = config
            .status
            .as_ref()
            .and_then(|s| {
                s.conditions
                    .iter()
                    .find(|c| c.r#type == "Ready")
                    .map(|c| if c.status == "True" { "True" } else { "False" })
            })
            .unwrap_or("Unknown");

        let secrets_synced = config
            .status
            .as_ref()
            .and_then(|s| s.secrets_synced)
            .map(|n| n.to_string())
            .unwrap_or_else(|| "-".to_string());

        println!("{:<30} {:<20} {:<15} {:<15}", name, ns, ready, secrets_synced);
    }

    Ok(())
}

/// Show detailed status of a SecretManagerConfig resource
async fn status_command(
    client: Client,
    name: String,
    namespace: Option<String>,
) -> Result<()> {
    let ns = namespace.as_deref().unwrap_or("default");
    
    println!("Status for SecretManagerConfig '{}/{}':\n", ns, name);

    let api: Api<SecretManagerConfig> = Api::namespaced(client, ns);

    let config = api
        .get(&name)
        .await
        .with_context(|| format!("Failed to get SecretManagerConfig '{}/{}'", ns, name))?;

    // Print basic info
    println!("Metadata:");
    println!("  Name: {}", config.metadata.name.as_deref().unwrap_or("<unknown>"));
    println!("  Namespace: {}", config.metadata.namespace.as_deref().unwrap_or("<unknown>"));
    if let Some(gen) = config.metadata.generation {
        println!("  Generation: {}", gen);
    }

    // Print spec
    println!("\nSpec:");
    println!("  GCP Project ID: {}", config.spec.gcp_project_id);
    println!("  Environment: {}", config.spec.environment);
    println!("  Source: {}/{}", config.spec.source_ref.kind, config.spec.source_ref.name);
    if let Some(ref kustomize_path) = config.spec.kustomize_path {
        println!("  Kustomize Path: {}", kustomize_path);
    }
    if let Some(ref base_path) = config.spec.base_path {
        println!("  Base Path: {}", base_path);
    }
    if let Some(ref prefix) = config.spec.secret_prefix {
        println!("  Secret Prefix: {}", prefix);
    }
    if let Some(ref suffix) = config.spec.secret_suffix {
        println!("  Secret Suffix: {}", suffix);
    }

    // Print status
    if let Some(ref status) = config.status {
        println!("\nStatus:");
        
        if let Some(gen) = status.observed_generation {
            println!("  Observed Generation: {}", gen);
        }
        
        if let Some(ref time) = status.last_reconcile_time {
            println!("  Last Reconcile Time: {}", time);
        }
        
        if let Some(count) = status.secrets_synced {
            println!("  Secrets Synced: {}", count);
        }

        if !status.conditions.is_empty() {
            println!("\nConditions:");
            for condition in &status.conditions {
                println!("  {}: {}", condition.r#type, condition.status);
                if let Some(ref reason) = condition.reason {
                    println!("    Reason: {}", reason);
                }
                if let Some(ref message) = condition.message {
                    println!("    Message: {}", message);
                }
                if let Some(ref time) = condition.last_transition_time {
                    println!("    Last Transition: {}", time);
                }
            }
        }
    } else {
        println!("\nStatus: No status available (resource may not have been reconciled yet)");
    }

    Ok(())
}

