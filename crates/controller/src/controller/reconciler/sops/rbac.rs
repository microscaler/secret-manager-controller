//! # SOPS RBAC Verification
//!
//! Verifies RBAC permissions for SOPS key watching.

use anyhow::Result;
use k8s_openapi::api::core::v1::Secret;
use kube::{Api, Client};
use tracing::debug;

/// Verify RBAC is properly configured for SOPS key watch
/// Tests actual API access to verify RBAC permissions are active
/// We test the actual operations we need rather than checking RBAC resources exist
/// (which would require clusterrole read permissions we shouldn't have)
pub async fn verify_rbac_for_sops_watch(client: &Client) -> Result<()> {
    // Test actual API access to verify RBAC is propagated
    // This is the real test - can we actually list secrets across all namespaces?
    // This is what we need for SOPS key watching, so if this works, RBAC is correct
    let secrets: Api<Secret> = Api::all(client.clone());
    match secrets
        .list(&kube::api::ListParams::default().limit(1))
        .await
    {
        Ok(_) => {
            debug!("✅ RBAC permissions verified - can list secrets across all namespaces");
            Ok(())
        }
        Err(e) => {
            // RBAC permissions not active - this could be propagation delay or misconfiguration
            Err(anyhow::anyhow!(
                "Cannot list secrets across all namespaces: {}. Verify RBAC is installed and ServiceAccount is bound to ClusterRole. Restart the pod if RBAC was created after pod started.",
                e
            ))
        }
    }
}
