# Python Script Usage Analysis

## Summary

This document tracks which Python scripts in `scripts/` are currently used and which are deprecated.

**Last Updated**: 2025-11-20

## Currently Used Scripts

| Script | Location | Usage | Notes |
|--------|----------|-------|-------|
| `tilt/build_controller.py` | Tiltfile | ✅ Active | Builds controller binary |
| `tilt/cleanup_stopped_containers.py` | Tiltfile | ✅ Active | Cleans up stopped containers |
| `tilt/install_fluxcd.py` | Tiltfile | ✅ Active | Installs FluxCD |
| `tilt/install_argocd.py` | Tiltfile | ✅ Active | Installs ArgoCD |
| `tilt/setup_git_credentials.py` | Tiltfile | ✅ Active | Sets up git credentials |
| `setup_sops_key.py` | Tiltfile | ✅ Active | Sets up SOPS keys |
| `tilt/build_mock_server_binary.py` | Tiltfile | ✅ Active | Builds mock server binaries (4x) |
| `tilt/docker_build_mock_server.py` | Tiltfile | ✅ Active | Docker build for mock servers |
| `tilt/docker_build_webhook.py` | Tiltfile | ✅ Active | Docker build for webhook |
| `pact_publish.py` | Tiltfile | ✅ Active | Publishes Pact contracts |
| `tilt/reset_test_resource.py` | Tiltfile | ✅ Active | Resets test resources |
| `tilt/generate_crd.py` | Tiltfile | ✅ Active | Generates CRD |
| `host_aware_build.py` | GitHub Actions | ✅ Active | Cross-compilation build |
| `copy_binary.py` | Used by build scripts | ✅ Active | Copies binaries |
| `dev_up.py` | justfile | ✅ Active | Starts dev environment |
| `dev_down.py` | justfile | ✅ Active | Stops dev environment |
| `undeploy.py` | justfile | ✅ Active | Undeploys from K8s |
| `status.py` | justfile | ✅ Active | Shows cluster status |
| `check_deps.py` | justfile | ✅ Active | Checks dependencies |
| `setup_kind.py` | Docs/Manual | ✅ Active | Sets up Kind cluster |
| `fix_registry_config.py` | Docs/Manual | ✅ Active | Fixes registry config |
| `cleanup_kind_storage.py` | Docs/Manual | ✅ Active | Cleans Kind storage |
| `test-sops-complete.py` | README/Docs | ✅ Active | Complete SOPS testing |
| `test-sops-quick.py` | README/Docs | ✅ Active | Quick SOPS testing |
| `test_sops_decrypt_and_pact.py` | Exists | ✅ Active | SOPS decrypt + Pact test |
| `pre_commit_rust.py` | Exists | ✅ Active | Pre-commit hook for Rust |
| `pre_commit_sops.py` | Exists | ✅ Active | Pre-commit hook for SOPS |

## Deprecated Scripts (Moved to `_deprecated/`)

| Script | Reason | Replacement |
|--------|--------|-------------|
| `tilt/build_binaries.py` | Replaced by `build_controller.py` | `tilt/build_controller.py` |
| `tilt/build_and_copy_binaries.py` | Replaced by `build_controller.py` | `tilt/build_controller.py` |
| `tilt/copy_binaries.py` | No longer needed (direct build) | N/A |
| `tilt/build_mock_server_binaries.py` | Replaced by `build_mock_server_binary.py` | `tilt/build_mock_server_binary.py` |
| `tilt/docker_build.py` | Replaced by `docker_build_controller.py` | `tilt/docker_build_controller.py` |
| `tilt/docker_build_controller.py` | Replaced by inline `custom_build` | Tiltfile `custom_build` |
| `tilt/cleanup.py` | Replaced by `cleanup_stopped_containers.py` | `tilt/cleanup_stopped_containers.py` |
| `build_and_push.py` | Not used in CI/CD | N/A (manual if needed) |
| `build_base_image.py` | Replaced by GitHub Actions | `.github/workflows/base-images.yml` |
| `semver_bump.py` | Not used | N/A |
| `set_package_visibility.py` | Not used (removed from workflow) | N/A |
| `extract_crd.py` | Not used | `tilt/generate_crd.py` |
| `setup_contour.py` | Commented out in Tiltfile | N/A (manual setup) |

## Notes

- **Tiltfile**: Uses `build_controller.py` directly, not the old `build_binaries.py` or `build_and_copy_binaries.py`
- **GitHub Actions**: Uses `host_aware_build.py` for cross-compilation
- **Justfile**: Uses `dev_up.py`, `dev_down.py`, `undeploy.py`, `status.py`, `check_deps.py`
- **Base Images**: Built via GitHub Actions workflow, not `build_base_image.py`
- **Docker Builds**: Controller uses inline `custom_build` in Tiltfile, not `docker_build_controller.py`


## Summary Statistics

- **Total Active Scripts**: 27
- **Total Deprecated Scripts**: 13 (all moved to `scripts/_deprecated/`)
- **Tiltfile Scripts**: 10
- **Root Scripts**: 17

## Cleanup Status

✅ **All deprecated scripts have been moved to `scripts/_deprecated/`**  
✅ **No deprecated scripts remain in active directories**  
⚠️ **Ready for final deletion after verification period (30 days)**
